//! One error type for both halves of the crate: libgit2's failures and the
//! `git` executable's.

/// What went wrong, kept as data so a screen can show it without unwrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    /// The operation, for the message: `git status --porcelain=v2`.
    pub operation: String,
    /// The reason, as git or libgit2 put it. Empty when nothing was said.
    pub message: String,
    /// The exit status of a `git` run, `-1` when the process did not start or
    /// the failure came from libgit2.
    pub code: i32,
}

impl GitError {
    pub fn new(operation: impl Into<String>, message: impl Into<String>, code: i32) -> Self {
        Self {
            operation: operation.into(),
            message: message.into(),
            code,
        }
    }

    /// A libgit2 failure.
    pub fn from_git2(operation: impl Into<String>, error: git2::Error) -> Self {
        Self {
            operation: operation.into(),
            message: error.message().to_owned(),
            code: -1,
        }
    }

    /// A one-line message for a toast: the first non-empty line git said, or
    /// the operation when it said nothing.
    pub fn summary(&self) -> String {
        self.message
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| self.operation.clone())
    }

    /// True when the failure is "this is not a repository", which the welcome
    /// screen answers by offering to open another folder.
    pub fn is_not_a_repo(&self) -> bool {
        let lower = self.message.to_lowercase();
        lower.contains("could not find repository")
            || lower.contains("not a git repository")
            || lower.contains("does not exist")
    }
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(f, "{} failed ({})", self.operation, self.code)
        } else {
            write!(f, "{}: {}", self.operation, self.message)
        }
    }
}

impl std::error::Error for GitError {}

impl From<git2::Error> for GitError {
    fn from(error: git2::Error) -> Self {
        Self::from_git2("git", error)
    }
}

pub type Result<T> = std::result::Result<T, GitError>;
