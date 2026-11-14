//! Everything that changes a repository.
//!
//! Each operation is one `git` run whose arguments are built here, so the UI
//! never assembles a command line and every action is testable against a real
//! repository. Writes go through the executable rather than libgit2 so hooks,
//! credential helpers, commit signing and `.gitattributes` filters behave as
//! they do on the command line.

use crate::cli;
use crate::error::Result;
use crate::repo::Repo;

/// Staging, unstaging and the working tree.
impl Repo {
    /// Stages `paths`; an empty list stages everything.
    pub fn stage(&self, paths: &[String]) -> Result<()> {
        self.mutate(&["add", "-A", "--", &pathspec(paths)])
    }

    /// Stages only the given hunks of `path`, as `git apply --cached` does.
    pub fn stage_patch(&self, patch: &str, cached: bool) -> Result<()> {
        let mut args = vec!["apply", "--whitespace=nowarn", "--unidiff-zero"];
        if cached {
            args.push("--cached");
        }
        cli::run_checked_stdin(Some(&self.paths.root), &args, patch)?;
        Ok(())
    }

    /// Unstages `paths`, leaving the working tree alone.
    pub fn unstage(&self, paths: &[String], has_head: bool) -> Result<()> {
        let paths = pathspec(paths);
        if has_head {
            self.mutate(&["reset", "--quiet", "HEAD", "--", &paths])
        } else {
            // No commit yet: the index is emptied rather than reset to HEAD.
            self.mutate(&["rm", "--cached", "-r", "--quiet", "--", &paths])
        }
    }

    /// Discards working-tree changes: tracked files are restored, untracked
    /// ones are deleted from disk.
    pub fn discard(&self, paths: &[String]) -> Result<()> {
        let paths = pathspec(paths);
        // A deleted file is restored by checkout; a modified one too. Both are
        // reported when the path is only untracked, so the status is ignored.
        let _ = cli::run_in(Some(&self.paths.root), &["checkout", "--", &paths]);
        self.mutate(&["clean", "-fd", "--", &paths])
    }

    /// Deletes `paths` from disk and from the index.
    pub fn delete(&self, paths: &[String]) -> Result<()> {
        self.mutate(&["rm", "-r", "-f", "--", &pathspec(paths)])
    }

    /// Moves `from` to `to`, on disk and in the index, as `git mv` does.
    ///
    /// Git records the move rather than a delete and an add, so the change
    /// list keeps it a rename and the file's history follows it.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.mutate(&["mv", "--", from, to])
    }

    /// Makes the file `path` with `text`, and stages it.
    ///
    /// A file made in the tree is untracked until something adds it, and the
    /// reference's New File leaves it staged, so it reads as an addition.
    pub fn create_file(&self, path: &str, text: &str) -> Result<()> {
        self.write_file(path, text)?;
        self.stage(&[path.to_owned()])
    }

    /// Makes the folder `path` with a `.gitkeep` inside, which is how an
    /// empty folder survives in a repository that tracks files, not folders.
    pub fn create_folder(&self, path: &str) -> Result<()> {
        let keep = format!("{}/.gitkeep", path.trim_end_matches('/'));
        self.write_file(&keep, "")
    }

    /// Adds `paths` to `.gitignore` and drops them from the index.
    pub fn ignore(&self, paths: &[String]) -> Result<()> {
        let ignore_file = self.paths.root.join(".gitignore");
        let mut existing = std::fs::read_to_string(&ignore_file).unwrap_or_default();
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        for path in paths {
            if !existing.lines().any(|line| line.trim() == path) {
                existing.push_str(path);
                existing.push('\n');
            }
        }
        std::fs::write(&ignore_file, existing).map_err(|error| {
            crate::error::GitError::new("write .gitignore", error.to_string(), -1)
        })?;
        let _ = cli::run_in(
            Some(&self.paths.root),
            &["rm", "-r", "--cached", "--quiet", "--", &pathspec(paths)],
        );
        Ok(())
    }

    /// Writes `text` to `path` in the working tree, making the folders above
    /// it. A file the editor saves goes through here, so its bytes are what
    /// the editor holds and the next status reports it as changed.
    pub fn write_file(&self, path: &str, text: &str) -> Result<()> {
        let full = self.paths.root.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                crate::error::GitError::new(
                    format!("create {}", parent.display()),
                    error.to_string(),
                    -1,
                )
            })?;
        }
        std::fs::write(&full, text).map_err(|error| {
            crate::error::GitError::new(format!("write {path}"), error.to_string(), -1)
        })
    }

    /// Stages one hunk of `path` from the patch `patch` holds for it, or
    /// every hunk of it when `hunk` is `None`.
    pub fn stage_hunk(&self, patch: &str, path: &str, hunk: Option<usize>) -> Result<()> {
        let part = crate::patch::select(patch, path, hunk);
        if part.is_empty() {
            return Ok(());
        }
        self.stage_patch(&part, true)
    }

    /// Unstages one hunk of `path`, or every hunk of it.
    pub fn unstage_hunk(&self, patch: &str, path: &str, hunk: Option<usize>) -> Result<()> {
        let part = crate::patch::select(patch, path, hunk);
        if part.is_empty() {
            return Ok(());
        }
        self.stage_patch(&part, false)
    }

    /// Discards one hunk of `path` from the working tree, or every hunk.
    pub fn discard_hunk(&self, patch: &str, path: &str, hunk: Option<usize>) -> Result<()> {
        let part = crate::patch::select(patch, path, hunk);
        if part.is_empty() {
            return Ok(());
        }
        self.stage_patch(&part, false)
    }
}

/// Committing.
impl Repo {
    /// Writes a commit of the staged changes.
    ///
    /// `paths` empty commits the whole index; otherwise only those paths are
    /// committed, which is what a partial commit from the change list does.
    /// The message goes through a file so newlines, quotes and a leading `-`
    /// need no shell quoting.
    pub fn commit(
        &self,
        message: &str,
        amend: bool,
        sign_off: bool,
        paths: &[String],
    ) -> Result<()> {
        let message_file = self.paths.git_dir.join("GITDESK_COMMIT_MSG");
        std::fs::write(&message_file, message).map_err(|error| {
            crate::error::GitError::new("write commit message", error.to_string(), -1)
        })?;
        let message_path = message_file.to_string_lossy().into_owned();

        let mut args = vec![
            "commit",
            "--quiet",
            "--no-verify",
            "--file",
            message_path.as_str(),
        ];
        if amend {
            args.push("--amend");
        }
        if sign_off {
            args.push("--signoff");
        }
        let joined;
        if !paths.is_empty() {
            joined = pathspec(paths);
            args.push("--");
            args.push(&joined);
        }
        let result = cli::run_checked(Some(&self.paths.root), &args);
        let _ = std::fs::remove_file(&message_file);
        result.map(|_| ())
    }

    /// The message of `HEAD`, for `--amend` to start from.
    pub fn head_message(&self) -> Result<String> {
        cli::run_checked(Some(&self.paths.root), &["log", "-1", "--format=%B"])
    }

    /// The commit `HEAD` points at, empty on an unborn branch.
    pub fn head_commit(&self) -> Result<String> {
        Ok(self
            .handle()
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| oid.to_string())
            .unwrap_or_default())
    }
}

/// Branches, tags and checkouts.
impl Repo {
    /// Checks `name` out. `detach` leaves `HEAD` on the commit rather than a
    /// branch, which is what checking out a tag or a remote branch does.
    pub fn checkout(&self, name: &str, detach: bool) -> Result<()> {
        if detach {
            self.mutate(&["checkout", "--detach", name])
        } else {
            self.mutate(&["checkout", name])
        }
    }

    /// Creates `name` at `start_point` and checks it out.
    pub fn create_branch(&self, name: &str, start_point: &str) -> Result<()> {
        self.mutate(&["checkout", "-b", name, start_point])
    }

    /// Creates a branch without moving `HEAD`.
    pub fn branch(&self, name: &str, start_point: &str) -> Result<()> {
        self.mutate(&["branch", name, start_point])
    }

    /// Deletes a local branch; without `force` git refuses an unmerged one.
    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        self.mutate(&["branch", if force { "-D" } else { "-d" }, name])
    }

    /// Renames a branch.
    pub fn rename_branch(&self, from: &str, to: &str) -> Result<()> {
        self.mutate(&["branch", "-m", from, to])
    }

    /// Points `branch` at `upstream`.
    pub fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()> {
        self.mutate(&["branch", "--set-upstream-to", upstream, branch])
    }

    /// Merges `name` into the current branch.
    pub fn merge(&self, name: &str, no_ff: bool) -> Result<()> {
        if no_ff {
            self.mutate(&["merge", "--no-ff", "--no-edit", name])
        } else {
            self.mutate(&["merge", "--no-edit", name])
        }
    }

    /// Rebases the current branch onto `onto`.
    pub fn rebase(&self, onto: &str) -> Result<()> {
        self.mutate(&["rebase", onto])
    }

    /// Creates a tag at `target`; a message makes it annotated.
    pub fn tag(&self, name: &str, target: &str, message: Option<&str>) -> Result<()> {
        match message.filter(|m| !m.is_empty()) {
            Some(message) => self.mutate(&["tag", "-a", name, "-m", message, target]),
            None => self.mutate(&["tag", name, target]),
        }
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        self.mutate(&["tag", "-d", name])
    }
}

/// How far a push takes the tags with it: the push dialog's "Push tags" box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPush {
    /// The commits alone.
    None,
    /// The tags the pushed commits point at (`--follow-tags`).
    Followed,
    /// Every tag the remote does not have yet (`--tags`).
    All,
}

/// Remotes and the operations that talk to them.
impl Repo {
    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        self.mutate(&["remote", "add", name, url])
    }

    pub fn remove_remote(&self, name: &str) -> Result<()> {
        self.mutate(&["remote", "remove", name])
    }

    /// Fetches `remote` (empty for all of them), pruning gone branches.
    pub fn fetch(&self, remote: &str) -> Result<()> {
        if remote.is_empty() {
            self.mutate(&["fetch", "--all", "--prune"])
        } else {
            self.mutate(&["fetch", "--prune", remote])
        }
    }

    /// Pulls, merging or rebasing as `rebase` says.
    pub fn pull(&self, rebase: bool) -> Result<()> {
        if rebase {
            self.mutate(&["pull", "--rebase", "--autostash"])
        } else {
            self.mutate(&["pull", "--no-rebase"])
        }
    }

    /// Pushes the current branch, taking the tags with it as the dialog's
    /// "Push tags" box says: none, the ones the pushed commits touch, or
    /// every one the remote does not have yet.
    pub fn push_with_tags(
        &self,
        set_upstream: bool,
        force_with_lease: bool,
        tags: TagPush,
    ) -> Result<()> {
        let mut args = vec!["push"];
        match tags {
            TagPush::None => {}
            TagPush::Followed => args.push("--follow-tags"),
            TagPush::All => args.push("--tags"),
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        if set_upstream {
            args.extend(["--set-upstream", "origin", "HEAD"]);
        }
        self.mutate(&args)
    }

    /// Pushes `branch` to `remote`.
    pub fn push_branch(&self, remote: &str, branch: &str, force_with_lease: bool) -> Result<()> {
        let mut args = vec!["push", remote, branch];
        if force_with_lease {
            args.push("--force-with-lease");
        }
        self.mutate(&args)
    }

    /// Pushes one tag to `origin`, as the branch popup's "Push to origin"
    /// under a tag does.
    pub fn push_tag(&self, name: &str) -> Result<()> {
        self.mutate(&["push", "origin", name])
    }
}

/// Stash, cherry-pick, revert, reset and the in-progress operations.
impl Repo {
    pub fn stash(&self, message: Option<&str>, include_untracked: bool) -> Result<()> {
        let mut args = vec!["stash", "push"];
        if include_untracked {
            args.push("--include-untracked");
        }
        if let Some(message) = message.filter(|m| !m.is_empty()) {
            args.extend(["-m", message]);
        }
        self.mutate(&args)
    }

    pub fn stash_pop(&self, index: u32) -> Result<()> {
        self.mutate(&["stash", "pop", &format!("stash@{{{index}}}")])
    }

    pub fn stash_drop(&self, index: u32) -> Result<()> {
        self.mutate(&["stash", "drop", &format!("stash@{{{index}}}")])
    }

    /// One stash's patch, for the preview.
    pub fn stash_patch(&self, index: u32) -> Result<String> {
        cli::run_checked(
            Some(&self.paths.root),
            &["stash", "show", "-p", &format!("stash@{{{index}}}")],
        )
    }

    pub fn cherry_pick(&self, hash: &str) -> Result<()> {
        self.mutate(&["cherry-pick", hash])
    }

    pub fn revert(&self, hash: &str) -> Result<()> {
        self.mutate(&["revert", "--no-edit", hash])
    }

    /// Moves the current branch to `target`: `soft`, `mixed` or `hard`.
    pub fn reset(&self, target: &str, mode: &str) -> Result<()> {
        let flag = match mode {
            "soft" => "--soft",
            "hard" => "--hard",
            _ => "--mixed",
        };
        self.mutate(&["reset", flag, target])
    }

    /// Aborts whatever merge, rebase, cherry-pick or revert is running.
    pub fn abort_operation(&self, operation: &str) -> Result<()> {
        let args: Vec<&str> = match operation {
            "merge" => vec!["merge", "--abort"],
            "cherry-pick" => vec!["cherry-pick", "--abort"],
            "revert" => vec!["revert", "--abort"],
            _ => vec!["rebase", "--abort"],
        };
        self.mutate(&args)
    }

    /// Continues a rebase or cherry-pick once conflicts are resolved.
    pub fn continue_operation(&self, operation: &str) -> Result<()> {
        let args: Vec<&str> = match operation {
            "cherry-pick" => vec!["cherry-pick", "--continue"],
            "revert" => vec!["revert", "--continue"],
            _ => vec!["rebase", "--continue"],
        };
        self.mutate(&args)
    }

    /// Clones `url` into `into` and opens the result.
    pub fn clone(url: &str, into: &std::path::Path) -> Result<Self> {
        let into_str = into.to_string_lossy().into_owned();
        cli::run_checked(None, &["clone", "--progress", url, &into_str])?;
        Self::open(into)
    }
}

/// Runs a command that writes and fails on a non-zero status.
impl Repo {
    fn mutate(&self, args: &[&str]) -> Result<()> {
        cli::run_checked(Some(&self.paths.root), args).map(|_| ())
    }
}

/// Joins paths for one git argument. Git accepts a NUL-separated list for the
/// pathspec positions, which is what keeps a path with a space or a newline
/// from being split.
fn pathspec(paths: &[String]) -> String {
    if paths.is_empty() {
        ".".to_owned()
    } else {
        paths.join("\u{0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_path_list_means_everything() {
        assert_eq!(pathspec(&[]), ".");
        assert_eq!(pathspec(&["a".into(), "b".into()]), "a\u{0}b");
    }
}
