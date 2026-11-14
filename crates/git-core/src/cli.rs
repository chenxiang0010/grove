//! Running the `git` executable, for every operation that writes.
//!
//! libgit2 reads fast and in-process, but a write has to honour hooks,
//! credential helpers, commit signing and `.gitattributes` filters, which is
//! the executable's job. Every mutation therefore goes through here, and the
//! UI never builds a command line itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{GitError, Result};

/// A finished `git` run.
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl GitOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// The `git` binary to run. `None` means `git` on `PATH`.
static EXECUTABLE: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// Points the crate at a specific `git` (Settings › Git). An empty path
/// restores `PATH` lookup.
pub fn set_git_executable(path: Option<&Path>) {
    let next = path
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    if let Ok(mut slot) = EXECUTABLE.write() {
        *slot = next;
    }
}

/// The `git` this crate runs, for the settings screen.
pub fn git_executable() -> PathBuf {
    match EXECUTABLE.read() {
        Ok(slot) => slot.clone().unwrap_or_else(|| PathBuf::from("git")),
        Err(_) => PathBuf::from("git"),
    }
}

/// `git --version`, or why it could not run. Checked before the first window
/// offers any repository action.
pub fn version() -> Result<String> {
    let output = run_in(None, &["--version"])?;
    if output.ok() {
        Ok(output.stdout.trim().to_owned())
    } else {
        Err(GitError::new("git --version", output.stderr, output.code))
    }
}

/// Runs git in `dir` and returns its output whether or not it succeeded.
pub fn run_in(dir: Option<&Path>, args: &[&str]) -> Result<GitOutput> {
    let mut command = base_command(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let shown = format!("git {}", args.join(" "));
    let output = command
        .output()
        .map_err(|error| GitError::new(shown.clone(), format!("could not run git: {error}"), -1))?;
    Ok(GitOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_owned(),
        code: output.status.code().unwrap_or(-1),
    })
}

/// Runs git and fails on a non-zero status, with git's stderr as the reason.
pub fn run_checked(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let output = run_in(dir, args)?;
    if output.ok() {
        Ok(output.stdout)
    } else {
        Err(GitError::new(
            format!("git {}", args.join(" ")),
            output.stderr,
            output.code,
        ))
    }
}

/// Runs git with `body` on stdin (`git apply`), failing on a non-zero status.
pub fn run_checked_stdin(dir: Option<&Path>, args: &[&str], body: &str) -> Result<String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut command = base_command(args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let shown = format!("git {}", args.join(" "));
    let mut child = command
        .spawn()
        .map_err(|error| GitError::new(shown.clone(), format!("could not run git: {error}"), -1))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| GitError::new(shown.clone(), error.to_string(), -1))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| GitError::new(shown.clone(), error.to_string(), -1))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr)
        .trim_end()
        .to_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(GitError::new(
            shown,
            stderr,
            output.status.code().unwrap_or(-1),
        ))
    }
}

/// The flags every run shares: no pager, no quoting, no terminal prompts, and
/// no optional index writes from a read.
fn base_command(args: &[&str]) -> Command {
    let mut command = Command::new(git_executable());
    command
        .arg("--no-pager")
        .args(["-c", "core.quotepath=false"])
        .args(["-c", "color.ui=false"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    // A windowed app spawning git gets a console window for it, which takes
    // the focus on top of flashing; no window is wanted on either count.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// The process runs detached from any console (`windows_subsystem = "windows"`),
/// and its children must not open one: `CREATE_NO_WINDOW` does exactly that.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_version() {
        let version = version().expect("git is installed");
        assert!(version.starts_with("git version"), "{version}");
    }

    #[test]
    fn a_failed_run_carries_gits_own_words() {
        let error = run_checked(None, &["rev-parse", "--verify", "no-such-ref-xyz"]).unwrap_err();
        assert!(error.code != 0);
        assert!(!error.message.is_empty());
    }
}
