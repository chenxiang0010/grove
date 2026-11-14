//! What the window shows: one repository read into plain data.
//!
//! The UI never holds a `Repo`: libgit2's handle is not shareable across
//! threads, and every read happens on a worker. A [`Snapshot`] is `Send`,
//! `Clone` and cheap to compare, so a refresh can replace one whole value and
//! every binding that read it updates at once.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use git_core::{
    BranchState, Commit, CommitDetails, CommitFile, GraphLayout, RefEntry, Remote, RepoPaths,
    StashEntry, Status,
};

/// One repository as the window shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Where the repository is, for a worker to reopen.
    pub paths: Option<RepoPaths>,
    /// The working tree's directory name, for the title.
    pub name: String,
    pub branch: BranchState,
    /// The change list. Shared, so a row reading the snapshot clones a
    /// pointer rather than every path.
    pub status: Arc<Status>,
    pub commits: Arc<Vec<Commit>>,
    /// The graph for `commits`, laid out in the same order.
    pub graph: Arc<GraphLayout>,
    pub refs: Arc<Vec<RefEntry>>,
    pub stashes: Arc<Vec<StashEntry>>,
    pub remotes: Arc<Vec<Remote>>,
    /// The working tree's top level, as the Project pane lists it: a folder
    /// or a file, by name.
    pub project: Arc<Vec<(String, bool)>>,
    /// The first thing that went wrong, when the read could not finish.
    pub error: Option<String>,
}

impl Snapshot {
    pub fn is_open(&self) -> bool {
        self.paths.is_some()
    }
}

/// What the log is narrowed to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Matched against the subject, the author and the hash.
    pub text: String,
    pub author: String,
    /// Matched against the paths a commit touched, loaded lazily.
    pub path: String,
    /// A branch or tag the log walks; empty walks everything.
    pub reference: Option<String>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.author.is_empty() && self.path.is_empty()
    }

    /// Whether `commit` passes the text and author filters.
    ///
    /// The path filter needs the commit's file list, which a listing does not
    /// carry; it is applied when the details of a commit are read.
    pub fn matches(&self, commit: &Commit) -> bool {
        if !self.text.is_empty() {
            let needle = self.text.to_lowercase();
            let hit = commit.subject.to_lowercase().contains(&needle)
                || commit.author_name.to_lowercase().contains(&needle)
                || commit.hash.starts_with(&needle)
                || commit.short_hash.starts_with(&needle);
            if !hit {
                return false;
            }
        }
        if !self.author.is_empty() {
            let needle = self.author.to_lowercase();
            if !commit.author_name.to_lowercase().contains(&needle) {
                return false;
            }
        }
        true
    }
}

/// Reads everything the window needs, on the calling thread.
///
/// Runs on a worker: opening costs milliseconds and libgit2 keeps no state a
/// second handle would miss, so a refresh needs nothing from the engine.
pub fn load(
    path: &Path,
    limit: usize,
    reference: Option<&str>,
    filter_path: Option<&str>,
) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let repo = match git_core::Repo::open(path) {
        Ok(repo) => repo,
        Err(error) => {
            snapshot.error = Some(error.summary());
            return snapshot;
        }
    };
    snapshot.name = repo.name();
    snapshot.paths = Some(repo.paths.clone());
    if repo.is_bare() {
        snapshot.error = Some("This repository has no working tree.".to_owned());
    }

    match repo.status() {
        Ok(status) => snapshot.branch = status.branch.clone(),
        Err(error) => snapshot.error = Some(error.summary()),
    }
    if snapshot.error.is_none() {
        match repo.status() {
            Ok(status) => snapshot.status = Arc::new(status),
            Err(error) => snapshot.error = Some(error.summary()),
        }
    }
    match repo.branch_state() {
        Ok(branch) => snapshot.branch = branch,
        Err(error) => {
            snapshot.error.get_or_insert(error.summary());
        }
    }

    let refs: Vec<String> = reference
        .map(|name| vec![name.to_owned()])
        .unwrap_or_default();
    match repo.log(limit, 0, &refs, filter_path) {
        Ok(commits) => {
            let layout = GraphLayout::build(
                &commits
                    .iter()
                    .map(|commit| (commit.hash.clone(), commit.parents.clone()))
                    .collect::<Vec<_>>(),
            );
            snapshot.commits = Arc::new(commits);
            snapshot.graph = Arc::new(layout);
        }
        Err(error) => {
            snapshot.error.get_or_insert(error.summary());
        }
    }

    if let Ok(refs) = repo.refs() {
        snapshot.refs = Arc::new(refs);
    }
    if let Ok(stashes) = repo.stashes() {
        snapshot.stashes = Arc::new(stashes);
    }
    if let Ok(remotes) = repo.remotes() {
        snapshot.remotes = Arc::new(remotes);
    }
    snapshot.project = Arc::new(read_project(&repo.paths.root));
    snapshot
}

/// The working tree's top level: folders first, then files, each by name.
///
/// `.git` is left out: the Project pane lists what the repository holds, not
/// the store it keeps it in.
fn read_project(root: &Path) -> Vec<(String, bool)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if is_dir {
            folders.push((name, true));
        } else {
            files.push((name, false));
        }
    }
    folders.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    folders.extend(files);
    folders
}

/// Opens a folder, returning the snapshot of the repository inside it.
pub fn open_folder(folder: &Path, limit: usize) -> Snapshot {
    load(folder, limit, None, None)
}

/// What the push dialog shows: the commits the branch has that its upstream
/// does not, and the files each of them touches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PushData {
    pub commits: Vec<Commit>,
    /// The files of each commit, by its hash.
    pub files: std::collections::HashMap<String, Arc<Vec<CommitFile>>>,
    /// The full message of each commit, by its hash: the push dialog shows it
    /// under the file tree of the commit being looked at.
    pub bodies: std::collections::HashMap<String, String>,
    /// Every touched path once, in the order the commits name them: what
    /// the dialog lists while no commit is picked.
    pub all_files: Arc<Vec<CommitFile>>,
}

/// Reads what a push would send, on the calling thread (a worker).
///
/// A commit whose details cannot be read contributes nothing rather than
/// failing the whole preview: one odd commit should not empty the dialog.
pub fn load_push(paths: &RepoPaths, limit: usize) -> PushData {
    let mut data = PushData::default();
    let Ok(repo) = paths.open() else {
        return data;
    };
    let Ok(commits) = repo.unpushed_commits(limit) else {
        return data;
    };
    let mut all: Vec<CommitFile> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for commit in &commits {
        let details = repo.commit_details(&commit.hash);
        let files = Arc::new(
            details
                .as_ref()
                .map(|d| d.files.clone())
                .unwrap_or_default(),
        );
        for file in files.iter() {
            if seen.insert(file.path.clone()) {
                all.push(file.clone());
            }
        }
        if let Ok(details) = details {
            // The details' body is the whole message, whose first line is
            // the subject; the pane shows what comes after it.
            let body = details.body.lines().skip(1).collect::<Vec<_>>().join("\n");
            data.bodies
                .insert(commit.hash.clone(), body.trim().to_owned());
        }
        data.files.insert(commit.hash.clone(), files);
    }
    data.commits = commits;
    data.all_files = Arc::new(all);
    data
}

/// The patch one commit introduced to one path, as the push dialog shows it.
///
/// A root commit has no parent to diff against; the empty tree stands in,
/// which is what `git diff --root` compares it with.
pub fn load_commit_file_patch(
    paths: &RepoPaths,
    hash: &str,
    parent: Option<&str>,
    path: &str,
) -> String {
    const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
    let from = parent
        .map(str::to_owned)
        .unwrap_or_else(|| EMPTY_TREE.to_owned());
    paths
        .open()
        .ok()
        .and_then(|repo| repo.file_diff_between(path, &from, hash).ok())
        .unwrap_or_default()
}

/// One commit with its message and files, read on a worker.
pub fn load_details(paths: &RepoPaths, hash: &str) -> Option<CommitDetails> {
    let repo = paths.open().ok()?;
    repo.commit_details(hash).ok()
}

/// The patch one commit introduced.
pub fn load_commit_patch(paths: &RepoPaths, hash: &str) -> String {
    paths
        .open()
        .ok()
        .and_then(|repo| repo.commit_diff(hash).ok())
        .unwrap_or_default()
}

/// The patch of one path against `HEAD` (staged) or the index (unstaged).
pub fn load_file_patch(paths: &RepoPaths, path: &str, staged: bool) -> String {
    paths
        .open()
        .ok()
        .and_then(|repo| repo.file_diff(path, staged).ok())
        .unwrap_or_default()
}

/// A stash's patch.
pub fn load_stash_patch(paths: &RepoPaths, index: u32) -> String {
    paths
        .open()
        .ok()
        .and_then(|repo| repo.stash_patch(index).ok())
        .unwrap_or_default()
}

/// The folders the window remembers, newest first.
pub fn recent_paths() -> Vec<PathBuf> {
    let Some(dir) = nsh_core::element::data_dir() else {
        return Vec::new();
    };
    let file = dir.join("recent.txt");

    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Adds `path` to the front of the recent list, keeping it short.
pub fn remember_path(path: &Path) {
    let Some(dir) = nsh_core::element::data_dir() else {
        return;
    };
    let file = dir.join("recent.txt");
    let mut entries = recent_paths();
    entries.retain(|entry| entry != path);
    entries.insert(0, path.to_path_buf());
    entries.truncate(12);
    let text = entries
        .iter()
        .map(|entry| entry.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(file, text);
}

/// The branch name a repository at `path` has checked out, read off disk.
///
/// The project menu shows one line under every recent project, and the read
/// is a text file — `.git/HEAD`, or the worktree's pointed-at `HEAD` — not a
/// repository open, so a menu of twelve projects costs twelve file reads.
pub fn head_branch_of(path: &Path) -> Option<String> {
    let dot_git = path.join(".git");
    let head = if dot_git.is_dir() {
        dot_git.join("HEAD")
    } else {
        // A worktree's `.git` is a file naming the real git dir.
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let gitdir = text.strip_prefix("gitdir:")?.trim();
        let gitdir = if Path::new(gitdir).is_absolute() {
            PathBuf::from(gitdir)
        } else {
            path.join(gitdir)
        };
        gitdir.join("HEAD")
    };
    let text = std::fs::read_to_string(head).ok()?;
    let text = text.trim();
    if let Some(branch) = text.strip_prefix("ref: refs/heads/") {
        Some(branch.to_owned())
    } else {
        // Detached: the commit is the answer there is.
        Some(text.chars().take(8).collect())
    }
}

/// The directory the last clone went into, or the default beside the home.
pub fn clone_dir_default() -> PathBuf {
    let Some(dir) = nsh_core::element::data_dir() else {
        return home_dir().join("GroveProjects");
    };
    std::fs::read_to_string(dir.join("clone_dir.txt"))
        .ok()
        .map(|text| PathBuf::from(text.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join("GroveProjects"))
}

/// Remembers the directory `path` clones go into.
pub fn remember_clone_dir(path: &Path) {
    let Some(dir) = nsh_core::element::data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("clone_dir.txt"), path.display().to_string());
}

/// The folder name a clone target takes from its URL: the last path segment,
/// without its `.git` or query, reduced to a file name (`https://h/o/repo.git`
/// clones into `repo`). What the dialog derives the "Clone into" field's tail
/// from, until the field is edited by hand.
pub fn clone_dir_name(url: &str) -> String {
    let mut text = url.trim().to_owned();
    if let Some(cut) = text.find(['?', '#']) {
        text.truncate(cut);
    }
    let text = text.trim_end_matches(['/', '\\']);
    let text = if text.to_lowercase().ends_with(".git") {
        &text[..text.len() - 4]
    } else {
        text
    };
    let name = text.rsplit(['/', '\\', ':']).next().unwrap_or(text);
    // Only what a folder name may hold, and nothing that would hide it.
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_start_matches(['.', '-'])
        .to_owned()
}

/// Joins the directory clones go into with the clone's folder name: the
/// whole path the "Clone into" field shows.
pub fn join_clone_path(parent: &str, name: &str) -> String {
    let parent = parent.trim().trim_end_matches(['/', '\\']);
    if parent.is_empty() {
        name.to_owned()
    } else if name.is_empty() {
        parent.to_owned()
    } else {
        format!("{parent}\\{name}")
    }
}

/// What stops the Clone button: the first problem with the form, or none.
pub fn validate_clone(url: &str, into: &str, shallow: bool, depth: &str) -> Option<&'static str> {
    let url = url.trim();
    if url.is_empty() {
        return Some("Enter a repository URL");
    }
    if url.contains([' ', '\t']) {
        return Some("The URL must not contain spaces");
    }
    if url.starts_with('-') {
        return Some("Invalid repository URL");
    }
    let into = into.trim();
    if into.is_empty() {
        return Some("Choose a directory");
    }
    if !is_absolute_path(into) {
        return Some("The directory must be an absolute path");
    }
    if shallow && depth.trim().parse::<u32>().map(|depth| depth > 0) != Ok(true) {
        return Some("Enter a positive number of commits");
    }
    None
}

/// Whether `path` is absolute, on either platform's spelling.
fn is_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.first() == Some(&b'/')
        || bytes.first() == Some(&b'\\')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/'))
}

/// The repository URLs the window remembers, newest first: what the clone
/// dialog's URL field offers behind its chevron.
pub fn recent_clone_urls() -> Vec<String> {
    let Some(dir) = nsh_core::element::data_dir() else {
        return Vec::new();
    };
    std::fs::read_to_string(dir.join("clone_urls.txt"))
        .ok()
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Adds `url` to the front of the recent list, keeping it short.
pub fn remember_clone_url(url: &str) {
    let Some(dir) = nsh_core::element::data_dir() else {
        return;
    };
    let mut urls = recent_clone_urls();
    urls.retain(|entry| entry != url);
    urls.insert(0, url.to_owned());
    urls.truncate(10);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("clone_urls.txt"), urls.join("\n"));
}

/// The update choice the window stopped asking about (`true` is rebase), as
/// the update dialog's "Don't show again" left it.
pub fn update_mode() -> Option<bool> {
    let dir = nsh_core::element::data_dir()?;
    match std::fs::read_to_string(dir.join("update_mode.txt"))
        .ok()?
        .trim()
    {
        "rebase" => Some(true),
        "merge" => Some(false),
        _ => None,
    }
}

/// Remembers the update choice that shall not be asked about again.
pub fn remember_update_mode(rebase: bool) {
    let Some(dir) = nsh_core::element::data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("update_mode.txt"),
        if rebase { "rebase" } else { "merge" },
    );
}

/// The user's home directory, or the current directory when it cannot be read.
fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(subject: &str, author: &str, hash: &str) -> Commit {
        Commit {
            hash: hash.to_owned(),
            short_hash: hash.chars().take(8).collect(),
            parents: Vec::new(),
            author_name: author.to_owned(),
            author_email: String::new(),
            author_time: 0,
            committer_name: author.to_owned(),
            commit_time: 0,
            subject: subject.to_owned(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn the_text_filter_matches_subject_author_and_hash() {
        let entry = commit(
            "fix(web): read checked mode",
            "ZiChuanShanFeng",
            "a1b2c3d4e5",
        );
        let by_subject = Filter {
            text: "checked".into(),
            ..Filter::default()
        };
        assert!(by_subject.matches(&entry));
        let by_author = Filter {
            text: "zichuan".into(),
            ..Filter::default()
        };
        assert!(by_author.matches(&entry));
        let by_hash = Filter {
            text: "a1b2c3".into(),
            ..Filter::default()
        };
        assert!(by_hash.matches(&entry));
        let miss = Filter {
            text: "nothing".into(),
            ..Filter::default()
        };
        assert!(!miss.matches(&entry));
    }

    #[test]
    fn an_empty_filter_takes_everything() {
        let filter = Filter::default();
        assert!(filter.is_empty());
        assert!(filter.matches(&commit("anything", "someone", "abc")));
    }

    #[test]
    fn the_author_filter_is_narrower_than_the_text_filter() {
        let entry = commit("release v0.2.31", "ada", "abc123");
        let author = Filter {
            author: "ada".into(),
            ..Filter::default()
        };
        assert!(author.matches(&entry));
        let wrong = Filter {
            author: "bob".into(),
            ..Filter::default()
        };
        assert!(!wrong.matches(&entry));
    }

    #[test]
    fn the_clone_folder_name_is_the_urls_last_segment_without_git() {
        assert_eq!(clone_dir_name("https://github.com/user/repo.git"), "repo");
        assert_eq!(clone_dir_name("https://github.com/user/repo"), "repo");
        assert_eq!(clone_dir_name("git@host:user/repo.git"), "repo");
        assert_eq!(clone_dir_name("https://host.io/x/y.git?tab=readme"), "y");
        assert_eq!(clone_dir_name("https://host.io/x/y.GIT/"), "y");
        assert_eq!(clone_dir_name("https://host.io/we ird repo"), "we-ird-repo");
        assert_eq!(clone_dir_name(""), "");
    }

    #[test]
    fn joining_the_clone_path_keeps_the_parent_without_the_name() {
        assert_eq!(
            join_clone_path("C:\\Users\\me\\GroveProjects", "repo"),
            "C:\\Users\\me\\GroveProjects\\repo"
        );
        assert_eq!(
            join_clone_path("C:\\Users\\me\\", "repo"),
            "C:\\Users\\me\\repo"
        );
        assert_eq!(join_clone_path("C:\\Users\\me", ""), "C:\\Users\\me");
    }

    #[test]
    fn the_clone_form_is_validated_in_the_order_the_fields_read() {
        assert_eq!(
            validate_clone("", "C:\\tmp", false, "1"),
            Some("Enter a repository URL")
        );
        assert_eq!(
            validate_clone("https://a b", "", false, "1"),
            Some("The URL must not contain spaces")
        );
        assert_eq!(
            validate_clone("-bad", "C:\\tmp", false, "1"),
            Some("Invalid repository URL")
        );
        assert_eq!(
            validate_clone("https://a", "", false, "1"),
            Some("Choose a directory")
        );
        assert_eq!(
            validate_clone("https://a", "tmp", false, "1"),
            Some("The directory must be an absolute path")
        );
        assert_eq!(
            validate_clone("https://a", "C:\\tmp", true, "0"),
            Some("Enter a positive number of commits")
        );
        assert_eq!(validate_clone("https://a", "C:\\tmp", true, "3"), None);
        assert_eq!(validate_clone("https://a", "C:\\tmp", false, "x"), None);
    }
}
