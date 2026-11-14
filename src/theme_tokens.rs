//! The numbers and colours every screen draws with.
//!
//! The metrics are the reference IDE's own, at 100% zoom: a 40pt title bar,
//! a 40pt tool window header, a 30pt stripe button, 26pt rows, a 34pt status
//! bar, 6pt between islands and 10pt corners. Colours come from the theme's
//! tokens (`theme.json`), never from a literal, so light and dark stay in
//! step.
//!
//! A hover or press state settles in [`MOTION_FAST`]; nothing else moves.

use git_core::ChangeKind;
use nsh_base::theme;
use nsh_core::prelude::*;

/// The height of one commit row, one file row and one tree row.
pub const ROW_H: f32 = 26.0;

/// The radius of a commit's dot.
pub const DOT_R: f32 = 3.6;

/// The title bar.
pub const TITLEBAR_H: f32 = 40.0;
/// A tool window's header, holding its tabs.
pub const HEADER_H: f32 = 40.0;
/// A bar inside a tool window: the branch search, the log filters, the
/// details' own toolbar.
pub const BAR_H: f32 = 32.0;
/// One button of the activity stripe.
pub const STRIPE_BUTTON: f32 = 30.0;
/// The status bar.
pub const STATUS_H: f32 = 34.0;
/// The gap between islands, and the corner they are rounded by.
pub const ISLAND_GAP: f32 = 6.0;
pub const ISLAND_RADIUS: f32 = 10.0;

/// A hover or press state settles in this long.
pub const MOTION_FAST: f32 = 80.0;
/// How many lanes the palette cycles through; `git_core::PALETTE_LEN` colours
/// are defined, and a layout never asks for more than that.
pub const LANE_COLORS: u8 = git_core::PALETTE_LEN;

/// A token's colour, falling back to `fallback` when the theme leaves it out.
pub fn token(name: &str, fallback: AdaptiveColor) -> AdaptiveColor {
    theme().color(name).unwrap_or(fallback)
}

/// The window's frame: what the islands sit on.
pub fn frame_bg() -> AdaptiveColor {
    token("frame", theme().background)
}

/// An island: a tool window's own surface.
pub fn island_bg() -> AdaptiveColor {
    token("island", theme().background)
}

/// A row under the pointer.
pub fn row_hover_bg() -> AdaptiveColor {
    token("island-hover", theme().accent)
}

/// A selected row, in a pane that has focus.
pub fn row_selected_bg() -> AdaptiveColor {
    token("island-selected", theme().selection())
}

/// A toolbar button under the pointer.
pub fn toolbar_hover_bg() -> AdaptiveColor {
    token("toolbar-hover", theme().accent)
}

/// A toolbar button that stays lit.
pub fn toolbar_selected_bg() -> AdaptiveColor {
    token("toolbar-selected", theme().selection())
}

/// The ground of a selected tool window tab.
pub fn tab_selected_bg() -> AdaptiveColor {
    token("tab-selected", theme().selection())
}

/// The hairline around a selected tool window tab.
pub fn tab_selected_border() -> AdaptiveColor {
    token("tab-selected-border", theme().primary)
}

/// The rule between two surfaces.
pub fn border() -> AdaptiveColor {
    token("border", theme().border)
}

/// The brightest text: a subject, a title.
pub fn fg_strong() -> AdaptiveColor {
    token("fg-strong", theme().foreground)
}

/// Ordinary text.
pub fn fg() -> AdaptiveColor {
    theme().foreground
}

/// Secondary text: an author, a count.
pub fn fg_muted() -> AdaptiveColor {
    token("fg-muted", theme().muted_foreground)
}

/// A monochrome icon's colour, which an icon button overrides on hover.
pub fn icon_color() -> AdaptiveColor {
    token("icon", theme().muted_foreground)
}

/// The colour of lane `index`: one line of a branch keeps it for its whole
/// run, which is what makes the graph readable.
pub fn graph_color(index: u8) -> AdaptiveColor {
    let name = format!("graph-{}", index % LANE_COLORS + 1);
    token(&name, theme().primary)
}

/// The colour of a change letter in the file lists.
pub fn change_color(kind: ChangeKind) -> AdaptiveColor {
    let name = match kind {
        ChangeKind::Added | ChangeKind::Copied => "change-added",
        ChangeKind::Modified | ChangeKind::TypeChanged | ChangeKind::Renamed => "change-modified",
        ChangeKind::Deleted => "change-deleted",
        ChangeKind::Conflicted => "change-conflict",
        ChangeKind::Untracked | ChangeKind::Ignored => "change-untracked",
    };
    token(name, theme().muted_foreground)
}

/// The colour a project's avatar wears, picked by its name: one project
/// keeps one colour across the title bar and the project menu. The pairs are
/// the reference's project colours, each dark enough for white initials.
pub fn project_avatar_color(name: &str) -> AdaptiveColor {
    let mut hash: u32 = 0;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u32::from(byte));
    }
    const COLORS: [AdaptiveColor; 6] = [
        AdaptiveColor::rgb(0x57965c, 0x57965c), // green
        AdaptiveColor::rgb(0x3574f0, 0x3871e1), // blue
        AdaptiveColor::rgb(0xbf7726, 0xd59637), // amber
        AdaptiveColor::rgb(0x955ae0, 0x955ae0), // purple
        AdaptiveColor::rgb(0xc54e58, 0xe06c75), // red
        AdaptiveColor::rgb(0x24a394, 0x3fbcbc), // teal
    ];
    COLORS[(hash % COLORS.len() as u32) as usize]
}

/// A project's initials: the first letter of its name's first two parts,
/// which is what its avatar prints.
pub fn project_initials(name: &str) -> String {
    let mut out: String = name
        .split(['-', '_', ' ', '.'])
        .filter(|part| !part.is_empty())
        .take(2)
        .filter_map(|part| part.chars().next())
        .collect();
    if out.is_empty() {
        out = name.chars().take(2).collect();
    }
    out.to_uppercase()
}
