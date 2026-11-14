//! Splitting a unified patch, so one hunk of one file can be staged on its
//! own.
//!
//! A patch is a sequence of `diff --git` sections, each a header and a run of
//! `@@` hunks. Partial staging and partial discarding both need one hunk with
//! the header it belongs to, because `git apply` reads a hunk only inside its
//! file's header.

/// One file of a patch: the header every hunk needs, and the hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    /// The path on the new side, without `b/`.
    pub path: String,
    /// Everything before the first `@@`, ending with a newline.
    pub header: String,
    /// The hunks, each starting with its `@@` line.
    pub hunks: Vec<String>,
}

/// Splits `patch` into its files.
pub fn files(patch: &str) -> Vec<FilePatch> {
    let mut out: Vec<FilePatch> = Vec::new();
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut path = String::new();
    let mut in_hunk = false;

    let flush = |out: &mut Vec<FilePatch>,
                 header: &mut String,
                 hunks: &mut Vec<String>,
                 path: &mut String| {
        if header.is_empty() && hunks.is_empty() {
            return;
        }
        out.push(FilePatch {
            path: std::mem::take(path),
            header: std::mem::take(header),
            hunks: std::mem::take(hunks),
        });
    };

    for line in patch.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            flush(&mut out, &mut header, &mut hunks, &mut path);
            header.push_str(line);
            path = new_side_path(line).unwrap_or_default();
            in_hunk = false;
        } else if line.starts_with("@@") {
            in_hunk = true;
            hunks.push(line.to_owned());
        } else if in_hunk {
            // A hunk's body ends at the next header; a line starting with
            // `\` is the "no newline" marker and belongs to the hunk.
            match hunks.last_mut() {
                Some(hunk) => hunk.push_str(line),
                None => header.push_str(line),
            }
        } else {
            header.push_str(line);
        }
    }
    flush(&mut out, &mut header, &mut hunks, &mut path);
    out
}

/// The patch of one file, or of one hunk of it.
///
/// `hunk` is an index into that file's hunks; `None` takes all of them.
/// Returns an empty string when the file or the hunk is not in `patch`.
pub fn select(patch: &str, path: &str, hunk: Option<usize>) -> String {
    let Some(file) = files(patch).into_iter().find(|file| file.path == path) else {
        return String::new();
    };
    let chosen: Vec<&String> = match hunk {
        Some(index) => match file.hunks.get(index) {
            Some(found) => vec![found],
            None => return String::new(),
        },
        None => file.hunks.iter().collect(),
    };
    if chosen.is_empty() {
        return String::new();
    }
    let mut out = file.header;
    for hunk in chosen {
        out.push_str(hunk);
    }
    out
}

/// The number of hunks `patch` holds for `path`.
pub fn hunk_count(patch: &str, path: &str) -> usize {
    files(patch)
        .into_iter()
        .find(|file| file.path == path)
        .map(|file| file.hunks.len())
        .unwrap_or(0)
}

/// The new-side path of a `diff --git a/x b/y` line.
fn new_side_path(line: &str) -> Option<String> {
    // The line keeps its newline, which is not part of any path.
    let line = line.trim_end_matches('\n');
    let rest = line.strip_prefix("diff --git ")?;
    // Quoted paths can hold spaces; the new side is the last `b/…` token.
    if let Some(at) = rest.rfind(" b/") {
        let path = &rest[at + 3..];
        return Some(path.trim_matches('"').to_owned());
    }
    // A rename or a path with no `a/` side: take the second half.
    let (_, after) = rest.split_once(" b/")?;
    Some(after.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/src/a.rs b/src/a.rs\n\
index 111..222 100644\n\
--- a/src/a.rs\n\
+++ b/src/a.rs\n\
@@ -1,3 +1,4 @@\n\
 one\n\
-two\n\
+two changed\n\
+two and a half\n\
 three\n\
@@ -10,2 +11,2 @@\n\
 ten\n\
-eleven\n\
+eleven changed\n\
diff --git a/b.txt b/b.txt\n\
new file mode 100644\n\
--- /dev/null\n\
+++ b/b.txt\n\
@@ -0,0 +1,1 @@\n\
+hello\n";

    #[test]
    fn splits_files_and_their_hunks() {
        let files = files(PATCH);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/a.rs");
        assert_eq!(files[0].hunks.len(), 2);
        assert!(files[0].header.contains("--- a/src/a.rs"));
        assert!(!files[0].header.contains("@@"));
        assert_eq!(files[1].path, "b.txt");
        assert_eq!(files[1].hunks.len(), 1);
    }

    #[test]
    fn selecting_one_hunk_keeps_the_header() {
        let one = select(PATCH, "src/a.rs", Some(1));
        assert!(one.starts_with("diff --git a/src/a.rs b/src/a.rs\n"));
        assert!(one.contains("@@ -10,2 +11,2 @@"));
        assert!(!one.contains("@@ -1,3 +1,4 @@"));
        assert!(!one.contains("b.txt"));
    }

    #[test]
    fn selecting_a_file_takes_every_hunk_of_it() {
        let whole = select(PATCH, "src/a.rs", None);
        assert!(whole.contains("@@ -1,3 +1,4 @@"));
        assert!(whole.contains("@@ -10,2 +11,2 @@"));
        assert!(!whole.contains("hello"));
        assert_eq!(hunk_count(PATCH, "src/a.rs"), 2);
    }

    #[test]
    fn a_path_the_patch_does_not_name_gives_nothing() {
        assert!(select(PATCH, "nope.rs", None).is_empty());
        assert!(select(PATCH, "src/a.rs", Some(9)).is_empty());
        assert_eq!(hunk_count(PATCH, "nope.rs"), 0);
    }
}
