//! Opening a repository and reading it through libgit2.
//!
//! Everything here is a read: status, refs, the log, diffs, blame. Reads run
//! in-process so a refresh costs milliseconds on a normal repository, which is
//! what makes the change list and the log feel immediate. Writes live in
//! [`crate::ops`] and go through the `git` executable.

use crate::error::{GitError, Result};
use std::path::{Path, PathBuf};

/// The kind of change a path carries, as both sides of the index can have one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Ignored,
    Conflicted,
}

impl ChangeKind {
    /// The letter IntelliJ shows in the change list.
    pub fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::TypeChanged => "T",
            Self::Untracked => "?",
            Self::Ignored => "!",
            Self::Conflicted => "U",
        }
    }

    /// The word a tooltip and a screen reader use.
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::TypeChanged => "Type changed",
            Self::Untracked => "Untracked",
            Self::Ignored => "Ignored",
            Self::Conflicted => "Conflicted",
        }
    }
}

/// One row of the change list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// Repository-relative, `/`-separated, never quoted.
    pub path: String,
    /// Where a rename or copy came from.
    pub orig_path: Option<String>,
    /// The staged change.
    pub staged: Option<ChangeKind>,
    /// The unstaged change.
    pub unstaged: Option<ChangeKind>,
}

impl StatusEntry {
    /// The letter a single-column list shows.
    pub fn display_kind(&self) -> ChangeKind {
        self.unstaged
            .or(self.staged)
            .unwrap_or(ChangeKind::Modified)
    }

    /// The file name without its directories.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The directories above the file, with a trailing `/`; empty at the top.
    pub fn dir(&self) -> &str {
        match self.path.rfind('/') {
            Some(at) => &self.path[..=at],
            None => "",
        }
    }

    /// True when only the index has a change.
    pub fn is_staged_only(&self) -> bool {
        self.staged.is_some() && self.unstaged.is_none()
    }
}

/// What `HEAD` points at, and how far it is from its upstream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchState {
    /// The commit `HEAD` resolves to; `None` on an unborn branch.
    pub head_commit: Option<String>,
    /// The branch name, or `None` when `HEAD` is detached.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// `merge`, `rebase`, `cherry-pick`, `revert` or `bisect` while one runs.
    pub operation: Option<String>,
}

impl BranchState {
    /// What the toolbar's branch button says.
    pub fn label(&self) -> String {
        match (&self.branch, &self.head_commit) {
            (Some(branch), _) => branch.clone(),
            (None, Some(commit)) => format!("Detached at {}", &commit[..commit.len().min(8)]),
            (None, None) => "No commits yet".to_owned(),
        }
    }
}

/// A whole status: the branch and the paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: BranchState,
    pub entries: Vec<StatusEntry>,
}

impl Status {
    pub fn staged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.staged.is_some())
    }

    pub fn unstaged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.unstaged.is_some())
    }

    pub fn conflicted(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| {
            e.staged == Some(ChangeKind::Conflicted) || e.unstaged == Some(ChangeKind::Conflicted)
        })
    }

    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn staged_count(&self) -> usize {
        self.staged().count()
    }

    pub fn unstaged_count(&self) -> usize {
        self.unstaged().count()
    }
}

/// Which kind of ref a name is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

impl RefKind {
    /// The short name: `main`, `origin/main`, `v1.2`.
    pub fn short(self, full: &str) -> String {
        match self {
            Self::LocalBranch => full.strip_prefix("refs/heads/").unwrap_or(full).to_owned(),
            Self::RemoteBranch => full
                .strip_prefix("refs/remotes/")
                .unwrap_or(full)
                .to_owned(),
            Self::Tag => full.strip_prefix("refs/tags/").unwrap_or(full).to_owned(),
        }
    }
}

/// One entry of the Branches tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefEntry {
    pub full_name: String,
    pub short_name: String,
    pub kind: RefKind,
    pub target: String,
    /// The branch this one tracks, for a local branch.
    pub upstream: Option<String>,
    /// The subject the ref points at.
    pub subject: String,
    /// The ref `HEAD` is on.
    pub is_head: bool,
}

/// A commit as the log lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Seconds since the epoch, author date.
    pub author_time: i64,
    pub committer_name: String,
    /// Seconds since the epoch, commit date.
    pub commit_time: i64,
    pub subject: String,
    /// Refs pointing at this commit, short: `HEAD -> main`, `tag: v1.0`.
    pub refs: Vec<String>,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// `author_time` as `2026-11-13 17:06`, in UTC.
    pub fn date_label(&self) -> String {
        format_timestamp(self.author_time)
    }

    /// `author_time` the way the log column reads it: `Today 17:06`,
    /// `Yesterday 09:12`, then `11-13 17:06` within the year, and
    /// `2025-11-13` beyond it.
    pub fn relative_label(&self, now: i64) -> String {
        relative_timestamp(self.author_time, now)
    }
}

/// The wall clock now, in seconds since the epoch: what the relative dates
/// are measured from.
pub fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// [`Commit::relative_label`] for a raw timestamp.
pub fn relative_timestamp(seconds: i64, now: i64) -> String {
    let rest = seconds.rem_euclid(86_400);
    let clock = format!("{:02}:{:02}", rest / 3600, (rest % 3600) / 60);
    // Days are counted in UTC, which is what both timestamps are: the label
    // says how far apart they are, not which wall clock showed them.
    let days = (now.div_euclid(86_400) - seconds.div_euclid(86_400)) as i64;
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let (today_year, _, _) = civil_from_days(now.div_euclid(86_400));
    match days {
        0 => format!("Today {clock}"),
        1 => format!("Yesterday {clock}"),
        _ if year == today_year => format!("{month:02}-{day:02} {clock}"),
        _ => format!("{year:04}-{month:02}-{day:02}"),
    }
}

/// Formats a Unix timestamp as `YYYY-MM-DD HH:MM` in UTC, without pulling in
/// a date library for one function.
pub fn format_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60
    )
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to a date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Where a repository is, without the handle: `Send`, so a worker thread can
/// reopen it.
///
/// `git2::Repository` is `Send` but not `Sync`, so a handle cannot be shared
/// between threads and a `Repo` holding one is not `Send`. Work that has to
/// leave the engine thread therefore takes a `RepoPaths` and opens its own
/// handle — opening costs milliseconds and libgit2 keeps no state a second
/// handle would miss.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoPaths {
    /// The working tree root, or the `.git` directory for a bare repository.
    pub root: PathBuf,
    /// `.git`, or the git directory a worktree or submodule points at.
    pub git_dir: PathBuf,
    /// False for a bare repository: no change list, nothing to check out.
    pub is_bare: bool,
}

impl RepoPaths {
    /// Reopens the repository, for a worker thread.
    pub fn open(&self) -> Result<Repo> {
        Repo::open(&self.root)
    }
}

/// An opened repository.
pub struct Repo {
    /// Where it is; what a worker thread needs to reopen it.
    pub paths: RepoPaths,
    inner: git2::Repository,
}

impl Repo {
    /// The working tree root, or the `.git` directory for a bare repository.
    pub fn root(&self) -> &Path {
        &self.paths.root
    }

    /// `.git`, or the git directory a worktree or submodule points at.
    pub fn git_dir(&self) -> &Path {
        &self.paths.git_dir
    }

    /// False for a bare repository: no change list, nothing to check out.
    pub fn is_bare(&self) -> bool {
        self.paths.is_bare
    }
}

impl Clone for Repo {
    /// A second handle on the same repository. Both read independently, which
    /// is what libgit2 expects; nothing writes through this type.
    fn clone(&self) -> Self {
        Self {
            paths: self.paths.clone(),
            inner: git2::Repository::open(&self.paths.git_dir).unwrap_or_else(|_| {
                git2::Repository::open(&self.paths.root)
                    .expect("the repository was open a moment ago")
            }),
        }
    }
}

impl std::fmt::Debug for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repo")
            .field("root", &self.paths.root)
            .field("is_bare", &self.paths.is_bare)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.paths.root == other.paths.root
    }
}

impl Repo {
    /// Opens the repository containing `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(GitError::new(
                format!("open {}", path.display()),
                "not a directory",
                -1,
            ));
        }
        let inner = git2::Repository::discover(path)
            .map_err(|error| GitError::from_git2(format!("open {}", path.display()), error))?;
        let root = if inner.is_bare() {
            inner.path().to_path_buf()
        } else {
            inner
                .workdir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| inner.path().to_path_buf())
        };
        let root = root.canonicalize().unwrap_or(root);
        Ok(Self {
            paths: RepoPaths {
                git_dir: inner.path().to_path_buf(),
                root,
                is_bare: inner.is_bare(),
            },
            inner,
        })
    }

    /// Creates a repository at `path` and opens it.
    pub fn init(path: impl AsRef<Path>, bare: bool) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|error| {
            GitError::new(format!("create {}", path.display()), error.to_string(), -1)
        })?;
        let mut options = git2::RepositoryInitOptions::new();
        options.bare(bare).no_reinit(false);
        git2::Repository::init_opts(path, &options)
            .map_err(|error| GitError::from_git2(format!("init {}", path.display()), error))?;
        Self::open(path)
    }

    /// The repository name shown in a tab.
    pub fn name(&self) -> String {
        self.paths
            .root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.paths.root.display().to_string())
    }

    /// The libgit2 handle.
    pub fn handle(&self) -> &git2::Repository {
        &self.inner
    }

    /// The full name of the ref `HEAD` is on (`refs/heads/main`), if any.
    fn head_name(&self) -> Option<String> {
        self.inner
            .head()
            .ok()
            .and_then(|head| head.name().ok().map(str::to_owned))
    }

    /// The change list, read in-process.
    ///
    /// A working tree holding an enormous number of files costs more here than
    /// in the executable, which keeps a cached index extension;
    /// [`Repo::status_via_cli`] is the fallback for those.
    pub fn status(&self) -> Result<Status> {
        if self.paths.is_bare {
            return Ok(Status::default());
        }
        let mut options = git2::StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .update_index(false);
        let statuses = self
            .inner
            .statuses(Some(&mut options))
            .map_err(|error| GitError::from_git2("status", error))?;

        let mut entries = Vec::with_capacity(statuses.len());
        for entry in statuses.iter() {
            let path = match entry.path() {
                Ok(path) => path.to_owned(),
                Err(_) => entry.path_bytes().iter().map(|b| *b as char).collect(),
            };
            let flags = entry.status();
            let index = change_from_git2(
                flags.intersects(git2::Status::INDEX_NEW),
                flags.intersects(git2::Status::INDEX_MODIFIED),
                flags.intersects(git2::Status::INDEX_DELETED),
                flags.intersects(git2::Status::INDEX_RENAMED),
                flags.intersects(git2::Status::INDEX_TYPECHANGE),
            );
            let worktree = change_from_git2(
                flags.intersects(git2::Status::WT_NEW),
                flags.intersects(git2::Status::WT_MODIFIED),
                flags.intersects(git2::Status::WT_DELETED),
                flags.intersects(git2::Status::WT_RENAMED),
                flags.intersects(git2::Status::WT_TYPECHANGE),
            );
            let conflicted = flags.contains(git2::Status::CONFLICTED);
            let orig_path = entry
                .head_to_index()
                .and_then(|delta| delta.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            entries.push(StatusEntry {
                path: path.replace('\\', "/"),
                orig_path,
                staged: if conflicted {
                    Some(ChangeKind::Conflicted)
                } else {
                    index
                },
                unstaged: if conflicted {
                    Some(ChangeKind::Conflicted)
                } else {
                    worktree
                },
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Status {
            branch: self.branch_state()?,
            entries,
        })
    }

    /// The change list read through `git status --porcelain=v2`, for a working
    /// tree too large for libgit2's uncached index walk.
    pub fn status_via_cli(&self) -> Result<Status> {
        let raw = crate::cli::run_checked(
            Some(&self.paths.root),
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=normal",
            ],
        )?;
        let mut status = parse_porcelain_v2(&raw);
        status.branch.operation = self.operation();
        Ok(status)
    }

    /// The branch, its upstream and the distance between them.
    pub fn branch_state(&self) -> Result<BranchState> {
        let mut state = BranchState {
            operation: self.operation(),
            ..BranchState::default()
        };
        if self.inner.head_detached().unwrap_or(false) {
            state.head_commit = self
                .inner
                .head()
                .ok()
                .and_then(|head| head.target())
                .map(|oid| oid.to_string());
            return Ok(state);
        }
        match self.inner.head() {
            Ok(head) => {
                state.head_commit = head.target().map(|oid| oid.to_string());
                if let Ok(shorthand) = head.shorthand() {
                    if shorthand != "HEAD" {
                        state.branch = Some(shorthand.to_owned());
                    }
                }
            }
            Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {}
            Err(error) => return Err(GitError::from_git2("head", error)),
        }
        if let Some(branch_name) = state.branch.clone() {
            if let Ok(branch) = self
                .inner
                .find_branch(&branch_name, git2::BranchType::Local)
            {
                if let Ok(upstream) = branch.upstream() {
                    state.upstream = upstream.name().ok().flatten().map(str::to_owned);
                    if let (Some(local), Some(remote)) =
                        (branch.get().target(), upstream.get().target())
                    {
                        if let Ok((ahead, behind)) = self.inner.graph_ahead_behind(local, remote) {
                            state.ahead = ahead as u32;
                            state.behind = behind as u32;
                        }
                    }
                }
            }
        }
        Ok(state)
    }

    /// The operation in progress, from the marker files git leaves.
    fn operation(&self) -> Option<String> {
        let dir = &self.paths.git_dir;
        [
            ("rebase-merge", "rebase"),
            ("rebase-apply", "rebase"),
            ("MERGE_HEAD", "merge"),
            ("CHERRY_PICK_HEAD", "cherry-pick"),
            ("REVERT_HEAD", "revert"),
            ("BISECT_LOG", "bisect"),
        ]
        .into_iter()
        .find(|(marker, _)| dir.join(marker).exists())
        .map(|(_, name)| name.to_owned())
    }

    /// Every branch and tag, ordered as the Branches tree shows them.
    pub fn refs(&self) -> Result<Vec<RefEntry>> {
        let head_name = self.head_name();
        let mut refs = Vec::new();
        for kind in [RefKind::LocalBranch, RefKind::RemoteBranch, RefKind::Tag] {
            let pattern = match kind {
                RefKind::LocalBranch => "refs/heads/*",
                RefKind::RemoteBranch => "refs/remotes/*",
                RefKind::Tag => "refs/tags/*",
            };
            let references = match self.inner.references_glob(pattern) {
                Ok(references) => references,
                Err(_) => continue,
            };
            for reference in references.flatten() {
                let Ok(full_name) = reference.name() else {
                    continue;
                };
                let full_name = full_name.to_owned();
                let target = reference
                    .target()
                    .map(|oid| oid.to_string())
                    .or_else(|| {
                        reference
                            .symbolic_target()
                            .ok()
                            .flatten()
                            .and_then(|name| self.inner.refname_to_id(name).ok())
                            .map(|oid| oid.to_string())
                    })
                    .unwrap_or_default();
                let is_head = head_name.as_deref() == Some(full_name.as_str());
                let upstream = if kind == RefKind::LocalBranch {
                    self.inner
                        .find_branch(&kind.short(&full_name), git2::BranchType::Local)
                        .ok()
                        .and_then(|branch| {
                            branch
                                .upstream()
                                .ok()
                                .and_then(|up| up.name().ok().flatten().map(str::to_owned))
                        })
                } else {
                    None
                };
                let subject = target
                    .parse()
                    .ok()
                    .and_then(|oid| self.inner.find_commit(oid).ok())
                    .and_then(|commit| commit.summary().ok().flatten().map(str::to_owned))
                    .unwrap_or_default();
                refs.push(RefEntry {
                    short_name: kind.short(&full_name),
                    full_name,
                    kind,
                    target,
                    upstream,
                    subject,
                    is_head,
                });
            }
        }
        refs.sort_by(|a, b| {
            a.kind.cmp(&b.kind).then_with(|| {
                a.short_name
                    .to_lowercase()
                    .cmp(&b.short_name.to_lowercase())
            })
        });
        Ok(refs)
    }

    /// The commits the log shows, newest first.
    ///
    /// `refs` names what to walk; empty means every ref, which is the Log tool
    /// window's default. `skip` pages through a long history. `path` narrows
    /// the walk to the commits that touched one path — a file's history, or a
    /// folder's — which libgit2's revwalk cannot do, so that walk runs the
    /// executable and its history simplification decides what belongs.
    pub fn log(
        &self,
        limit: usize,
        skip: usize,
        refs: &[String],
        path: Option<&str>,
    ) -> Result<Vec<Commit>> {
        if let Some(path) = path.filter(|path| !path.is_empty()) {
            return self.log_for_path(limit, skip, refs, path);
        }
        let mut walk = self
            .inner
            .revwalk()
            .map_err(|error| GitError::from_git2("revwalk", error))?;
        walk.set_sorting(git2::Sort::TIME)
            .map_err(|error| GitError::from_git2("revwalk sorting", error))?;
        if refs.is_empty() {
            walk.push_glob("refs/heads/*")
                .map_err(|error| GitError::from_git2("revwalk heads", error))?;
            // Remotes and tags too, so a fetched branch with no local checkout
            // still shows, and a tag on an older commit joins the history.
            let _ = walk.push_glob("refs/remotes/*");
            let _ = walk.push_glob("refs/tags/*");
        } else {
            for name in refs {
                if walk.push_ref(name).is_err() {
                    let _ = walk.push_glob(name);
                }
            }
        }

        let mut commits = Vec::with_capacity(limit.min(2048));
        let mut seen = 0;
        for oid in walk {
            let Ok(oid) = oid else { continue };
            if seen < skip {
                seen += 1;
                continue;
            }
            seen += 1;
            let Ok(commit) = self.inner.find_commit(oid) else {
                continue;
            };
            commits.push(self.commit_from_git2(&commit));
            if commits.len() >= limit {
                break;
            }
        }
        Ok(commits)
    }

    /// The commits that touched `path`, newest first, as `git log -- path`
    /// picks them: a merge that did not change the path is left out, which is
    /// what makes a file's history readable.
    fn log_for_path(
        &self,
        limit: usize,
        skip: usize,
        refs: &[String],
        path: &str,
    ) -> Result<Vec<Commit>> {
        // One record per commit, its fields NUL-separated: a subject holds
        // no NUL and no newline, so the parse needs no quoting.
        let format = "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ct%x00%s";
        let count = format!("--max-count={limit}");
        let offset = format!("--skip={skip}");
        let mut args: Vec<&str> = vec!["log", format, count.as_str(), offset.as_str()];
        if refs.is_empty() {
            args.push("--all");
        } else {
            args.extend(refs.iter().map(String::as_str));
        }
        args.push("--");
        args.push(path);
        let output = crate::cli::run_checked(Some(&self.paths.root), &args)?;

        let mut commits = Vec::new();
        for record in output.lines().filter(|line| !line.is_empty()) {
            let fields: Vec<&str> = record.split('\u{0}').collect();
            if fields.len() < 9 {
                continue;
            }
            let hash = fields[0].to_owned();
            let oid = git2::Oid::from_str(&hash).ok();
            commits.push(Commit {
                refs: oid.map(|oid| self.refs_at(oid)).unwrap_or_default(),
                hash,
                short_hash: fields[1].to_owned(),
                parents: fields[2].split_whitespace().map(str::to_owned).collect(),
                author_name: fields[3].to_owned(),
                author_email: fields[4].to_owned(),
                author_time: fields[5].parse().unwrap_or(0),
                committer_name: fields[6].to_owned(),
                commit_time: fields[7].parse().unwrap_or(0),
                subject: fields[8].to_owned(),
            });
        }
        Ok(commits)
    }

    /// The commits the current branch has that its upstream does not, newest
    /// first: what the push dialog lists. Empty when git cannot name the
    /// range — a branch with no upstream has nothing known to push.
    pub fn unpushed_commits(&self, limit: usize) -> Result<Vec<Commit>> {
        let format = "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ct%x00%s";
        let count = format!("--max-count={limit}");
        let output = crate::cli::run_in(
            Some(&self.paths.root),
            &["log", format, &count, "@{upstream}..HEAD"],
        );
        let output = match output {
            Ok(output) if output.ok() => output,
            _ => return Ok(Vec::new()),
        };

        let mut commits = Vec::new();
        for record in output.stdout.lines().filter(|line| !line.is_empty()) {
            let fields: Vec<&str> = record.split('\u{0}').collect();
            if fields.len() < 9 {
                continue;
            }
            let hash = fields[0].to_owned();
            let oid = git2::Oid::from_str(&hash).ok();
            commits.push(Commit {
                refs: oid.map(|oid| self.refs_at(oid)).unwrap_or_default(),
                hash,
                short_hash: fields[1].to_owned(),
                parents: fields[2].split_whitespace().map(str::to_owned).collect(),
                author_name: fields[3].to_owned(),
                author_email: fields[4].to_owned(),
                author_time: fields[5].parse().unwrap_or(0),
                committer_name: fields[6].to_owned(),
                commit_time: fields[7].parse().unwrap_or(0),
                subject: fields[8].to_owned(),
            });
        }
        Ok(commits)
    }

    /// One commit by hash or revspec. (Named `commit_at` because
    /// `Repo::commit` writes a commit.)
    pub fn commit_at(&self, hash: &str) -> Result<Commit> {
        let commit = self
            .inner
            .revparse_single(hash)
            .and_then(|object| object.peel_to_commit())
            .map_err(|error| GitError::from_git2(format!("resolve {hash}"), error))?;
        Ok(self.commit_from_git2(&commit))
    }

    fn commit_from_git2(&self, commit: &git2::Commit<'_>) -> Commit {
        let author = commit.author();
        let committer = commit.committer();
        let hash = commit.id().to_string();
        Commit {
            short_hash: hash.chars().take(8).collect(),
            refs: self.refs_at(commit.id()),
            hash,
            parents: commit.parent_ids().map(|oid| oid.to_string()).collect(),
            author_name: author.name().unwrap_or_default().to_owned(),
            author_email: author.email().unwrap_or_default().to_owned(),
            author_time: author.when().seconds(),
            committer_name: committer.name().unwrap_or_default().to_owned(),
            commit_time: committer.when().seconds(),
            subject: commit
                .summary()
                .ok()
                .flatten()
                .unwrap_or_default()
                .to_owned(),
        }
    }

    /// The short ref names pointing at `oid`: `HEAD -> main`, `origin/main`,
    /// `tag: v1.0`.
    fn refs_at(&self, oid: git2::Oid) -> Vec<String> {
        let mut names = Vec::new();
        let head_name = self.head_name();
        let Ok(references) = self.inner.references() else {
            return names;
        };
        for reference in references.flatten() {
            if reference.target() != Some(oid) {
                continue;
            }
            let Ok(name) = reference.name() else { continue };
            let short = if let Some(branch) = name.strip_prefix("refs/heads/") {
                branch.to_owned()
            } else if let Some(remote) = name.strip_prefix("refs/remotes/") {
                remote.to_owned()
            } else if let Some(tag) = name.strip_prefix("refs/tags/") {
                format!("tag: {tag}")
            } else {
                continue;
            };
            if head_name.as_deref() == Some(name) {
                names.push(format!("HEAD -> {short}"));
            } else {
                names.push(short);
            }
        }
        names
    }

    /// The full message and the changed paths of one commit.
    pub fn commit_details(&self, hash: &str) -> Result<CommitDetails> {
        let commit = self
            .inner
            .revparse_single(hash)
            .and_then(|object| object.peel_to_commit())
            .map_err(|error| GitError::from_git2(format!("resolve {hash}"), error))?;
        let body = commit.message().unwrap_or_default().trim_end().to_owned();
        let mut listed = self.commit_from_git2(&commit);
        listed.subject = body.lines().next().unwrap_or_default().to_owned();
        let files = self.commit_files(&commit)?;
        Ok(CommitDetails {
            commit: listed,
            body,
            files,
        })
    }

    /// The paths one commit touched, with their status against its first
    /// parent.
    fn commit_files(&self, commit: &git2::Commit<'_>) -> Result<Vec<CommitFile>> {
        let tree = commit
            .tree()
            .map_err(|error| GitError::from_git2("tree", error))?;
        let parent_tree = match commit.parent(0) {
            Ok(parent) => Some(
                parent
                    .tree()
                    .map_err(|error| GitError::from_git2("parent tree", error))?,
            ),
            Err(_) => None,
        };
        let mut options = git2::DiffOptions::new();
        options.include_typechange(true);
        let diff = self
            .inner
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))
            .map_err(|error| GitError::from_git2("diff", error))?;
        Ok(diff.deltas().map(delta_to_file).collect())
    }

    /// The working-tree text of `path`; empty when the file is absent.
    pub fn read_file(&self, path: &str) -> Result<String> {
        match std::fs::read(self.paths.root.join(path)) {
            Ok(bytes) => Ok(decode_text(&bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(error) => Err(GitError::new(format!("read {path}"), error.to_string(), -1)),
        }
    }

    /// The text of `path` at `rev`; empty when it is not there.
    ///
    /// `rev` is a commit hash or a name; `"WORKTREE"` reads the file on disk
    /// and `":0"` the index's stage.
    pub fn read_file_at(&self, rev: &str, path: &str) -> Result<String> {
        if rev == "WORKTREE" {
            return self.read_file(path);
        }
        let found = if rev == ":0" {
            self.inner
                .index()
                .ok()
                .and_then(|index| index.get_path(Path::new(path), 0))
                .map(|entry| self.inner.find_blob(entry.id))
        } else {
            Some(
                self.inner
                    .revparse_single(&format!("{rev}:{path}"))
                    .and_then(|object| object.peel_to_blob()),
            )
        };
        match found {
            Some(Ok(blob)) => Ok(decode_text(blob.content())),
            Some(Err(error)) if error.code() == git2::ErrorCode::NotFound => Ok(String::new()),
            Some(Err(error)) => Err(GitError::from_git2(format!("read {rev}:{path}"), error)),
            None => Ok(String::new()),
        }
    }

    /// The diff of `path` between two revisions, as a patch.
    ///
    /// Either side may be a branch, a tag or a commit hash; an empty `from`
    /// reads the path out of nothing, which is what a new file compares as.
    pub fn file_diff_between(&self, path: &str, from: &str, to: &str) -> Result<String> {
        let mut options = git2::DiffOptions::new();
        options
            .pathspec(path)
            .include_typechange(true)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);
        let tree_of = |revision: &str| -> Option<git2::Tree<'_>> {
            if revision.is_empty() {
                return None;
            }
            self.inner
                .revparse_single(revision)
                .ok()
                .and_then(|object| object.peel_to_tree().ok())
        };
        let old = tree_of(from);
        let new = tree_of(to);
        let diff = self
            .inner
            .diff_tree_to_tree(old.as_ref(), new.as_ref(), Some(&mut options))
            .map_err(|error| GitError::from_git2(format!("diff {from}..{to}"), error))?;
        patch_of(&diff)
    }

    /// The diff of one path in the working tree or the index, as a patch.
    pub fn file_diff(&self, path: &str, staged: bool) -> Result<String> {
        let mut options = git2::DiffOptions::new();
        // A file the index does not have yet is part of the working tree's
        // changes, and the reference shows it as a diff of every line added.
        // Without this the diff of an untracked path comes back empty.
        options
            .pathspec(path)
            .include_typechange(true)
            .include_untracked(true)
            // A new file's directory is untracked too, and libgit2 does not
            // descend into it unless asked.
            .recurse_untracked_dirs(true)
            // An untracked file's lines are its content, not just its name.
            .show_untracked_content(true);
        let diff = if staged {
            let tree = self
                .inner
                .head()
                .ok()
                .and_then(|head| head.peel_to_tree().ok());
            self.inner
                .diff_tree_to_index(tree.as_ref(), None, Some(&mut options))
                .map_err(|error| GitError::from_git2("diff --cached", error))?
        } else {
            self.inner
                .diff_index_to_workdir(None, Some(&mut options))
                .map_err(|error| GitError::from_git2("diff", error))?
        };
        Ok(patch_of(&diff)?)
    }

    /// The patch one commit introduced.
    pub fn commit_diff(&self, hash: &str) -> Result<String> {
        let commit = self
            .inner
            .revparse_single(hash)
            .and_then(|object| object.peel_to_commit())
            .map_err(|error| GitError::from_git2(format!("resolve {hash}"), error))?;
        let tree = commit
            .tree()
            .map_err(|error| GitError::from_git2("tree", error))?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let mut options = git2::DiffOptions::new();
        options.include_typechange(true);
        let diff = self
            .inner
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))
            .map_err(|error| GitError::from_git2("diff", error))?;
        patch_of(&diff)
    }

    /// `git blame` of `path`, one entry per source line.
    pub fn blame(&self, path: &str) -> Result<Vec<BlameLine>> {
        let mut options = git2::BlameOptions::new();
        options.track_copies_same_file(true);
        let blame = self
            .inner
            .blame_file(Path::new(path), Some(&mut options))
            .map_err(|error| GitError::from_git2(format!("blame {path}"), error))?;
        let text = self.read_file(path)?;
        let mut lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = (index + 1) as u32;
            match blame.get_line(line_number as usize) {
                Some(hunk) => {
                    let hash = hunk.final_commit_id().to_string();
                    let signature = hunk.final_signature();
                    lines.push(BlameLine {
                        line: line_number,
                        short_hash: hash.chars().take(8).collect(),
                        hash,
                        author: signature
                            .as_ref()
                            .and_then(|s| s.name().ok())
                            .unwrap_or_default()
                            .to_owned(),
                        time: signature
                            .as_ref()
                            .map(|s| s.when().seconds())
                            .unwrap_or_default(),
                        content: line.to_owned(),
                    });
                }
                None => lines.push(BlameLine {
                    line: line_number,
                    content: line.to_owned(),
                    ..BlameLine::default()
                }),
            }
        }
        Ok(lines)
    }

    /// The stash list, newest first.
    pub fn stashes(&self) -> Result<Vec<StashEntry>> {
        let mut stashes = Vec::new();
        let mut inner = self.clone();
        // `stash_foreach` needs `&mut Repository`; the handle is shared, so a
        // second one is opened for this call rather than holding a lock.
        let mut repo = git2::Repository::open(&self.paths.git_dir)
            .map_err(|error| GitError::from_git2("open for stash", error))?;
        let _ = &mut inner;
        repo.stash_foreach(|index, message, _oid| {
            stashes.push(StashEntry {
                index: index as u32,
                message: message.to_owned(),
            });
            true
        })
        .map_err(|error| GitError::from_git2("stash list", error))?;
        Ok(stashes)
    }

    /// `(name, fetch url, push url)` for every remote.
    pub fn remotes(&self) -> Result<Vec<Remote>> {
        let names = self
            .inner
            .remotes()
            .map_err(|error| GitError::from_git2("remotes", error))?;
        let mut remotes = Vec::new();
        for name in names.iter().flatten().flatten() {
            let Ok(remote) = self.inner.find_remote(name) else {
                continue;
            };
            let fetch_url = remote.url().unwrap_or_default().to_owned();
            let push_url = remote
                .pushurl()
                .ok()
                .flatten()
                .unwrap_or(fetch_url.as_str())
                .to_owned();
            remotes.push(Remote {
                name: name.to_owned(),
                fetch_url,
                push_url,
            });
        }
        Ok(remotes)
    }
}

/// One line of a blame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlameLine {
    pub line: u32,
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub time: i64,
    pub content: String,
}

/// One path of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFile {
    pub path: String,
    pub kind: ChangeKind,
}

/// A commit with its message and the paths it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetails {
    pub commit: Commit,
    pub body: String,
    pub files: Vec<CommitFile>,
}

/// One stash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub index: u32,
    pub message: String,
}

/// One remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

/// Renders a diff as a unified patch, which is what `diff_view` and a file's
/// preview both read.
fn patch_of(diff: &git2::Diff<'_>) -> Result<String> {
    let mut text = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => text.push(line.origin() as u8),
            _ => {}
        }
        text.extend_from_slice(line.content());
        true
    })
    .map_err(|error| GitError::from_git2("diff text", error))?;
    Ok(String::from_utf8_lossy(&text).into_owned())
}

/// One delta of a diff as a change-list row.
fn delta_to_file(delta: git2::DiffDelta<'_>) -> CommitFile {
    let path = delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let kind = match delta.status() {
        git2::Delta::Added => ChangeKind::Added,
        git2::Delta::Deleted => ChangeKind::Deleted,
        git2::Delta::Renamed => ChangeKind::Renamed,
        git2::Delta::Copied => ChangeKind::Copied,
        git2::Delta::Typechange => ChangeKind::TypeChanged,
        git2::Delta::Untracked => ChangeKind::Untracked,
        git2::Delta::Conflicted => ChangeKind::Conflicted,
        _ => ChangeKind::Modified,
    };
    CommitFile { path, kind }
}

/// Maps libgit2's per-side flags to one [`ChangeKind`], most specific first.
fn change_from_git2(
    new: bool,
    modified: bool,
    deleted: bool,
    renamed: bool,
    typechange: bool,
) -> Option<ChangeKind> {
    if renamed {
        Some(ChangeKind::Renamed)
    } else if deleted {
        Some(ChangeKind::Deleted)
    } else if typechange {
        Some(ChangeKind::TypeChanged)
    } else if new {
        Some(ChangeKind::Added)
    } else if modified {
        Some(ChangeKind::Modified)
    } else {
        None
    }
}

/// Decodes file bytes, honouring a UTF-8 BOM and falling back to Latin-1 so a
/// file never loses its text entirely.
fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    }
}

/// Parses `git status --porcelain=v2 --branch -z`, the fallback path.
fn parse_porcelain_v2(raw: &str) -> Status {
    let mut status = Status::default();
    let mut tokens = raw.split('\0').peekable();
    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        if let Some(rest) = token.strip_prefix("# ") {
            let mut parts = rest.splitn(2, ' ');
            match parts.next().unwrap_or_default() {
                "branch.oid" => {
                    let oid = parts.next().unwrap_or_default();
                    status.branch.head_commit = if oid == "(initial)" {
                        None
                    } else {
                        Some(oid.to_owned())
                    };
                }
                "branch.head" => {
                    let head = parts.next().unwrap_or_default();
                    status.branch.branch = if head == "(detached)" {
                        None
                    } else {
                        Some(head.to_owned())
                    };
                }
                "branch.upstream" => {
                    status.branch.upstream = Some(parts.next().unwrap_or_default().to_owned());
                }
                "branch.ab" => {
                    let ab = parts.next().unwrap_or_default();
                    let mut counts = ab.split(' ');
                    if let Some(ahead) = counts.next().and_then(|n| n.strip_prefix('+')) {
                        status.branch.ahead = ahead.parse().unwrap_or(0);
                    }
                    if let Some(behind) = counts.next().and_then(|n| n.strip_prefix('-')) {
                        status.branch.behind = behind.parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
            continue;
        }
        match token.as_bytes()[0] {
            b'1' => {
                let fields: Vec<&str> = token.splitn(9, ' ').collect();
                if fields.len() < 9 {
                    continue;
                }
                let xy = fields[1].as_bytes();
                status.entries.push(StatusEntry {
                    path: fields[8].to_owned(),
                    orig_path: None,
                    staged: kind_from_letter(xy[0]),
                    unstaged: kind_from_letter(xy[1]),
                });
            }
            b'2' => {
                let fields: Vec<&str> = token.splitn(10, ' ').collect();
                if fields.len() < 10 {
                    continue;
                }
                let xy = fields[1].as_bytes();
                let orig_path = tokens.next().map(str::to_owned);
                status.entries.push(StatusEntry {
                    path: fields[9].to_owned(),
                    orig_path,
                    staged: kind_from_letter(xy[0]),
                    unstaged: kind_from_letter(xy[1]),
                });
            }
            b'u' => {
                let fields: Vec<&str> = token.splitn(11, ' ').collect();
                if fields.len() < 11 {
                    continue;
                }
                status.entries.push(StatusEntry {
                    path: fields[10].to_owned(),
                    orig_path: None,
                    staged: Some(ChangeKind::Conflicted),
                    unstaged: Some(ChangeKind::Conflicted),
                });
            }
            b'?' => status.entries.push(StatusEntry {
                path: token[2..].to_owned(),
                orig_path: None,
                staged: None,
                unstaged: Some(ChangeKind::Untracked),
            }),
            b'!' => status.entries.push(StatusEntry {
                path: token[2..].to_owned(),
                orig_path: None,
                staged: None,
                unstaged: Some(ChangeKind::Ignored),
            }),
            _ => {}
        }
    }
    status.entries.sort_by(|a, b| a.path.cmp(&b.path));
    status
}

/// One letter of `--porcelain=v2`'s `XY` to a [`ChangeKind`].
fn kind_from_letter(letter: u8) -> Option<ChangeKind> {
    Some(match letter {
        b'A' => ChangeKind::Added,
        b'M' => ChangeKind::Modified,
        b'D' => ChangeKind::Deleted,
        b'R' => ChangeKind::Renamed,
        b'C' => ChangeKind::Copied,
        b'T' => ChangeKind::TypeChanged,
        b'U' => ChangeKind::Conflicted,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_render_in_utc() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
        assert_eq!(format_timestamp(1_700_000_000), "2023-11-14 22:13");
    }

    #[test]
    fn status_entries_split_name_and_directory() {
        let entry = StatusEntry {
            path: "src/ui/button.rs".to_owned(),
            orig_path: None,
            staged: None,
            unstaged: Some(ChangeKind::Modified),
        };
        assert_eq!(entry.name(), "button.rs");
        assert_eq!(entry.dir(), "src/ui/");
        let top = StatusEntry {
            path: "README.md".to_owned(),
            ..entry.clone()
        };
        assert_eq!(top.name(), "README.md");
        assert_eq!(top.dir(), "");
    }

    #[test]
    fn porcelain_parses_an_ordinary_change() {
        let raw = "# branch.oid abc123\0# branch.head main\0# branch.ab +2 -1\01 M. N... 100644 100644 100644 aaa bbb src/main.rs\0? notes.txt\0";
        let status = parse_porcelain_v2(raw);
        assert_eq!(status.branch.branch.as_deref(), Some("main"));
        assert_eq!(status.branch.ahead, 2);
        assert_eq!(status.branch.behind, 1);
        assert_eq!(status.entries.len(), 2);
        // Rows are sorted by path, which is what the change list shows.
        assert_eq!(status.entries[0].path, "notes.txt");
        assert_eq!(status.entries[1].path, "src/main.rs");
        assert_eq!(status.entries[1].staged, Some(ChangeKind::Modified));
        assert_eq!(status.entries[1].unstaged, None);
        assert_eq!(status.entries[0].unstaged, Some(ChangeKind::Untracked));
    }

    #[test]
    fn porcelain_parses_a_rename_with_its_origin() {
        let raw = "2 R. N... 100644 100644 100644 aaa bbb R100 new/name.rs\0old/name.rs\0";
        let status = parse_porcelain_v2(raw);
        assert_eq!(status.entries.len(), 1);
        assert_eq!(status.entries[0].path, "new/name.rs");
        assert_eq!(status.entries[0].orig_path.as_deref(), Some("old/name.rs"));
        assert_eq!(status.entries[0].staged, Some(ChangeKind::Renamed));
    }

    #[test]
    fn a_conflict_lands_on_both_sides() {
        let raw = "u UU N... 100644 100644 100644 100644 aaa bbb ccc conflict.txt\0";
        let status = parse_porcelain_v2(raw);
        assert_eq!(status.entries[0].staged, Some(ChangeKind::Conflicted));
        assert_eq!(status.entries[0].unstaged, Some(ChangeKind::Conflicted));
        assert_eq!(status.conflicted().count(), 1);
    }
}
