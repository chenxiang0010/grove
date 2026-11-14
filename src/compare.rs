//! Comparing two folders, or two files, that need not be in a repository.
//!
//! The reference's Project tree can compare any folder on disk against any
//! other: two checkouts of the same project on different branches, a folder
//! of an unpacked archive against the working tree, a file against a file.
//! None of that is Git's own question — there is no index, no commit and no
//! `a/`-`b/` pair — so it lives here rather than in `git_core`.
//!
//! A directory comparison is a walk of both trees into one path-keyed map,
//! so a path present on one side only is as easy to name as one present on
//! both. Whether a path both sides have is "different" is the
//! [`CompareMode`]'s question, as it is in the reference's *Compare by:*
//! control. A file comparison is a unified diff of the two texts, which is
//! what `nsh_base`'s diff view reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use imara_diff::{Algorithm, BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};

/// The names a walk leaves out: a repository's store, and the two folders a
/// build fills with files that are never the same twice.
const SKIPPED: [&str; 3] = [".git", "node_modules", "target"];

/// How many entries one side may hold before the walk gives up. A comparison
/// is a reading, not an index: past this the table is useless anyway.
const LIMIT: usize = 20_000;

/// How a comparison decides that a path both sides have is different.
///
/// The reference offers these four under *Compare by:* and starts on
/// [`CompareMode::Content`]; the labels are its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompareMode {
    /// The most honest and the slowest: the size, and then the bytes when
    /// the sizes match. The write time is ignored.
    #[default]
    Content,
    /// The text, with the line separators normalised: a file saved with
    /// CRLF on one side and LF on the other reads as the same file.
    Text,
    /// The size alone.
    Size,
    /// The size, and then the write time when the sizes match.
    SizeAndTimestamp,
}

impl CompareMode {
    /// Every mode, in the order the reference's menu lists them.
    pub const ALL: [CompareMode; 4] = [
        CompareMode::Content,
        CompareMode::Text,
        CompareMode::Size,
        CompareMode::SizeAndTimestamp,
    ];

    /// What the control says, and what the menu's item is called.
    pub fn label(self) -> &'static str {
        match self {
            CompareMode::Content => "Binary Content",
            CompareMode::Text => "Text",
            CompareMode::Size => "Size",
            CompareMode::SizeAndTimestamp => "Size and Timestamp",
        }
    }

    /// The mode `label` names, if any does.
    pub fn from_label(label: &str) -> Option<CompareMode> {
        CompareMode::ALL
            .into_iter()
            .find(|mode| mode.label() == label)
    }
}

/// One entry of a compared folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub is_dir: bool,
    /// The file's length; 0 for a folder.
    pub size: u64,
    /// When it was last written, in seconds since the epoch.
    pub modified: i64,
}

/// One path of a comparison: what one side, the other, or both have at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Relative to the compared roots, `/`-separated.
    pub path: String,
    pub left: Option<Entry>,
    pub right: Option<Entry>,
    /// Whether both sides have it and what they have differs, as the
    /// comparison's [`CompareMode`] decides.
    pub differs: bool,
}

impl Row {
    /// The file name, without the folders above it.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Whether this path is only on one side.
    pub fn only(&self) -> Option<Side> {
        match (&self.left, &self.right) {
            (Some(_), None) => Some(Side::Left),
            (None, Some(_)) => Some(Side::Right),
            _ => None,
        }
    }

    /// Whether the path is a folder on either side, so a press on it opens
    /// nothing.
    pub fn holds_folder(&self) -> bool {
        self.left.as_ref().is_some_and(|entry| entry.is_dir)
            || self.right.as_ref().is_some_and(|entry| entry.is_dir)
    }
}

/// Which of the two compared folders a path belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Two folders, walked and laid side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub left: PathBuf,
    pub right: PathBuf,
    /// How "both sides have it, and it differs" was decided.
    pub mode: CompareMode,
    /// Every path of either side, by path.
    pub rows: Vec<Row>,
    /// The files only the left side has.
    pub only_left: usize,
    /// The files only the right side has.
    pub only_right: usize,
    /// The files both sides have, with different content.
    pub different: usize,
}

impl Comparison {
    /// The row at `path`.
    #[cfg(test)]
    pub fn row(&self, path: &str) -> Option<&Row> {
        self.rows.iter().find(|row| row.path == path)
    }
}

/// Walks `left` and `right` and lays what they hold side by side, deciding
/// differences by the bytes, as the reference does by default.
pub fn compare_dirs(left: &Path, right: &Path) -> Comparison {
    compare_dirs_with(left, right, CompareMode::Content)
}

/// Walks `left` and `right` and lays what they hold side by side, with
/// `mode` deciding what counts as a difference.
pub fn compare_dirs_with(left: &Path, right: &Path, mode: CompareMode) -> Comparison {
    let mut sides: BTreeMap<String, (Option<Entry>, Option<Entry>)> = BTreeMap::new();
    let mut budget = LIMIT;
    walk(left, "", &mut sides, 0, &mut budget);
    walk(right, "", &mut sides, 1, &mut budget);

    let mut rows = Vec::with_capacity(sides.len());
    let (mut only_left, mut only_right, mut different) = (0, 0, 0);
    for (path, (left_entry, right_entry)) in sides {
        let differs = match (&left_entry, &right_entry) {
            (Some(one), Some(other)) => {
                if one.is_dir != other.is_dir {
                    // A folder here and a file there: they cannot be the same.
                    true
                } else if one.is_dir {
                    false
                } else {
                    files_differ(mode, &left.join(&path), &right.join(&path), one, other)
                }
            }
            _ => false,
        };
        let row = Row {
            path,
            left: left_entry,
            right: right_entry,
            differs,
        };
        match row.only() {
            Some(Side::Left) => only_left += 1,
            Some(Side::Right) => only_right += 1,
            None if row.differs => different += 1,
            None => {}
        }
        rows.push(row);
    }
    Comparison {
        left: left.to_path_buf(),
        right: right.to_path_buf(),
        mode,
        rows,
        only_left,
        only_right,
        different,
    }
}

/// Whether two files of the same size differ, as `mode` decides.
fn files_differ(mode: CompareMode, left: &Path, right: &Path, one: &Entry, other: &Entry) -> bool {
    // Text mode is the one mode whose answer the byte count cannot decide:
    // a CRLF file and its LF twin are the same text at different lengths,
    // so the sizes are read and then set aside.
    if mode != CompareMode::Text && one.size != other.size {
        return true;
    }
    match mode {
        CompareMode::Size => false,
        CompareMode::SizeAndTimestamp => one.size != other.size || one.modified != other.modified,
        CompareMode::Content => match (std::fs::read(left), std::fs::read(right)) {
            (Ok(one), Ok(other)) => one != other,
            // A file that cannot be read is not one whose sameness can be
            // claimed: it reads as different rather than as equal.
            _ => true,
        },
        CompareMode::Text => match (read_text(left), read_text(right)) {
            (Ok(one), Ok(other)) => normalize(&one) != normalize(&other),
            _ => true,
        },
    }
}

/// A file's text with its line separators made uniform, so a CRLF checkout
/// and an LF one read as the same file.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Reads `dir` into `out`, keyed by path relative to the compared root.
///
/// `side` says which half of the pair the entry fills: 0 the left, 1 the
/// right. A name in [`SKIPPED`] is left out of both, so a repository's store
/// never drowns the files the reader meant to compare.
fn walk(
    dir: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, (Option<Entry>, Option<Entry>)>,
    side: u8,
    budget: &mut usize,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIPPED.contains(&name.as_str()) {
            continue;
        }
        // A symlink is followed neither way: a pair of links can point at each
        // other, and a walk that follows one never ends.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let Ok(meta) = entry.metadata() else { continue };
        let is_dir = kind.is_dir();
        let entry_value = Entry {
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|since| since.as_secs() as i64)
                .unwrap_or(0),
        };
        *budget -= 1;
        let slot = out.entry(path.clone()).or_insert((None, None));
        if side == 0 {
            slot.0 = Some(entry_value);
        } else {
            slot.1 = Some(entry_value);
        }
        if is_dir {
            walk(&entry.path(), &path, out, side, budget);
        }
    }
}

/// The patch of two files, for the diff view.
///
/// `left` and `right` are the files' paths, `None` for a side that has no
/// file: every line of the other side then reads as added or deleted, which
/// is what a file only one folder has looks like.
pub fn diff_patch(left: Option<&Path>, right: Option<&Path>, name: &str) -> String {
    let left_text = left.map(read_text).unwrap_or(Ok(String::new()));
    let right_text = right.map(read_text).unwrap_or(Ok(String::new()));
    let (Ok(before), Ok(after)) = (left_text, right_text) else {
        // One side is not text. A patch of two made-up lines names both files
        // and says how big each is, which reads better than an empty pane.
        return format!(
            "--- {}\n+++ {}\n@@ -1,1 +1,1 @@\n-{}\n+{}\n",
            left.map(|path| path.display().to_string())
                .unwrap_or_else(|| "/dev/null".to_owned()),
            right
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "/dev/null".to_owned()),
            describe(left),
            describe(right),
        );
    };
    if before == after {
        return String::new();
    }

    let input = InternedInput::new(before.as_str(), after.as_str());
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let printer = BasicLineDiffPrinter(&input.interner);
    let mut config = UnifiedDiffConfig::default();
    config.context_len(3);
    let body = diff.unified_diff(&printer, config, &input);

    format!(
        "--- a/{name}\n+++ b/{name}\n{body}",
        name = name,
        body = body
    )
}

/// What a side holds, for the patch of a file that is not text.
fn describe(path: Option<&Path>) -> String {
    match path {
        None => "no file".to_owned(),
        Some(path) => match std::fs::metadata(path) {
            Ok(meta) => format!("binary file, {} bytes", meta.len()),
            Err(_) => "unreadable file".to_owned(),
        },
    }
}

/// A file's text, or why it is not text.
fn read_text(path: &Path) -> Result<String, ()> {
    let bytes = std::fs::read(path).map_err(|_| ())?;
    String::from_utf8(bytes).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of files, made fresh for one test.
    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Dir {
            let root = std::env::temp_dir().join(format!("grove-compare-{name}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("a temp folder");
            Dir(root)
        }

        fn write(&self, path: &str, text: &str) -> &Dir {
            let full = self.0.join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("a folder");
            std::fs::write(full, text).expect("a file");
            self
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_comparison_names_what_is_only_on_one_side() {
        let left = Dir::new("only-left");
        let right = Dir::new("only-right");
        left.write("both.txt", "same").write("mine.txt", "a");
        right.write("both.txt", "same").write("yours.txt", "b");

        let comparison = compare_dirs(left.path(), right.path());
        let only_left: Vec<&str> = comparison
            .rows
            .iter()
            .filter(|row| row.only() == Some(Side::Left))
            .map(|row| row.path.as_str())
            .collect();
        let only_right: Vec<&str> = comparison
            .rows
            .iter()
            .filter(|row| row.only() == Some(Side::Right))
            .map(|row| row.path.as_str())
            .collect();
        assert_eq!(only_left, ["mine.txt"]);
        assert_eq!(only_right, ["yours.txt"]);
        assert_eq!(comparison.only_left, 1);
        assert_eq!(comparison.only_right, 1);
        assert_eq!(comparison.different, 0);
    }

    #[test]
    fn a_comparison_counts_files_that_differ() {
        let left = Dir::new("differs-left");
        let right = Dir::new("differs-right");
        left.write("a.txt", "one").write("b.txt", "same");
        right.write("a.txt", "two").write("b.txt", "same");

        let comparison = compare_dirs(left.path(), right.path());
        assert_eq!(comparison.different, 1);
        assert!(comparison.row("a.txt").is_some_and(|row| row.differs));
        assert!(comparison.row("b.txt").is_some_and(|row| !row.differs));
    }

    #[test]
    fn a_folder_only_one_side_has_is_named_and_its_files_are_listed() {
        let left = Dir::new("folder-left");
        let right = Dir::new("folder-right");
        left.write("only/inner.txt", "x");

        let comparison = compare_dirs(left.path(), right.path());
        let paths: Vec<&str> = comparison
            .rows
            .iter()
            .map(|row| row.path.as_str())
            .collect();
        assert_eq!(paths, ["only", "only/inner.txt"]);
        assert!(comparison.row("only").is_some_and(|row| row.holds_folder()));
        assert!(
            comparison
                .row("only/inner.txt")
                .is_some_and(|row| !row.holds_folder())
        );
    }

    #[test]
    fn the_size_mode_ignores_two_files_of_one_length() {
        let left = Dir::new("size-left");
        let right = Dir::new("size-right");
        left.write("a.txt", "one");
        right.write("a.txt", "two");

        let comparison = compare_dirs_with(left.path(), right.path(), CompareMode::Size);
        assert_eq!(comparison.different, 0);
        assert!(comparison.row("a.txt").is_some_and(|row| !row.differs));
    }

    #[test]
    fn the_text_mode_ignores_the_line_separator() {
        let left = Dir::new("text-left");
        let right = Dir::new("text-right");
        left.write("a.txt", "one\ntwo\n");
        right.write("a.txt", "one\r\ntwo\r\n");

        let comparison = compare_dirs_with(left.path(), right.path(), CompareMode::Text);
        assert_eq!(comparison.different, 0);
        let by_bytes = compare_dirs_with(left.path(), right.path(), CompareMode::Content);
        assert_eq!(by_bytes.different, 1);
    }

    #[test]
    fn the_patch_of_two_files_names_both_and_their_lines() {
        let left = Dir::new("patch-left");
        let right = Dir::new("patch-right");
        left.write("a.txt", "one\ntwo\n");
        right.write("a.txt", "one\nthree\n");

        let patch = diff_patch(
            Some(&left.path().join("a.txt")),
            Some(&right.path().join("a.txt")),
            "a.txt",
        );
        assert!(patch.contains("--- a/a.txt"), "{patch}");
        assert!(patch.contains("+++ b/a.txt"), "{patch}");
        assert!(patch.contains("-two"), "{patch}");
        assert!(patch.contains("+three"), "{patch}");
    }

    #[test]
    fn a_file_only_one_side_has_reads_as_added_or_deleted() {
        let right = Dir::new("missing-right");
        right.write("new.txt", "hello\n");

        let patch = diff_patch(None, Some(&right.path().join("new.txt")), "new.txt");
        assert!(patch.contains("+hello"), "{patch}");
    }

    #[test]
    fn two_equal_files_have_no_patch() {
        let left = Dir::new("equal-left");
        let right = Dir::new("equal-right");
        left.write("a.txt", "same\n");
        right.write("a.txt", "same\n");

        let patch = diff_patch(
            Some(&left.path().join("a.txt")),
            Some(&right.path().join("a.txt")),
            "a.txt",
        );
        assert!(patch.is_empty());
    }
}
