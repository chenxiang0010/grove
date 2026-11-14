//! The change bars an editor tab draws in its gutter: the lines an edit has
//! added, changed or removed since the file's version in the index.
//!
//! The reference IDE marks each such line with a bar beside its number — a
//! green one for a line that is new, a blue one for a line that changed, and
//! a red line across the strip where lines were removed. The bars are the
//! editor's own decorations ([`EditorDecoration::gutter`]), so they move with
//! the text as it is typed and folded.

use git_core::ChangeKind;
use imara_diff::{Algorithm, Diff, InternedInput};
use nsh_base::{
    EditorDecoration, EditorGutterMark, EditorGutterPlacement, EditorPosition, EditorRange,
};
use nsh_core::element::Style as InlineStyle;
use nsh_core::{PropertyValue, StyleDecl, StyleKey};

use crate::theme_tokens::change_color;

/// The bars for the text `after`, measured against `before`.
///
/// An empty `before` (a file the index does not have) marks every line as
/// added, which is what a new file is.
pub fn decorations(before: &str, after: &str) -> Vec<EditorDecoration> {
    let input = InternedInput::new(before, after);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let mut out = Vec::new();
    for hunk in diff.hunks() {
        // A hunk with nothing on the new side is a removal: the reference
        // draws it as a line across the strip where the lines used to be.
        if hunk.after.is_empty() {
            out.push(boundary(hunk.after.start, ChangeKind::Deleted));
            continue;
        }
        let kind = match hunk.before.is_empty() {
            true => ChangeKind::Added,
            false => ChangeKind::Modified,
        };
        for line in hunk.after.start..hunk.after.end {
            out.push(bar(line, kind));
        }
    }
    out
}

/// A bar beside line `line` (from 0), in `kind`'s colour, with a faint wash
/// over the line itself.
fn bar(line: u32, kind: ChangeKind) -> EditorDecoration {
    let mut decoration = EditorDecoration::default();
    // The range ends at the last column of the line, not at column 0 of the
    // next one: the editor washes every line the range touches, and a range
    // ending at the next line's first column counts as touching that line too.
    decoration.range = EditorRange::new(
        EditorPosition::new(line, 0),
        EditorPosition::new(line, u32::MAX),
    );
    decoration.line_style = Some(background(kind, 0.16));
    decoration.gutter = Some(mark(kind, EditorGutterPlacement::Lines));
    decoration
}

/// A line across the strip before line `line`: the place lines were removed.
fn boundary(line: u32, kind: ChangeKind) -> EditorDecoration {
    let mut decoration = EditorDecoration::default();
    decoration.range = EditorRange::new(EditorPosition::new(line, 0), EditorPosition::new(line, 0));
    decoration.gutter = Some(mark(kind, EditorGutterPlacement::Before));
    decoration
}

fn mark(kind: ChangeKind, placement: EditorGutterPlacement) -> EditorGutterMark {
    let mut mark = EditorGutterMark::default();
    mark.placement = placement;
    mark.style = Some(background(kind, 1.0));
    mark
}

/// `kind`'s colour, as an inline style: `alpha` of it as a background.
fn background(kind: ChangeKind, alpha: f32) -> InlineStyle {
    let hsla = nsh_core::element::color(change_color(kind).opacity(alpha));
    InlineStyle::from_decls([StyleDecl::Set(
        StyleKey::BackgroundColor,
        PropertyValue::Color(Some(hsla.into())),
    )])
}
