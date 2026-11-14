//! The sidebar's pane, and the header that names it.
//!
//! Which pane shows is chosen on the activity stripe at the window's left
//! edge; the sidebar itself carries the reference's tool window header — the
//! section's icon and name, the close cross and the `…` — and then the pane.

use nsh_core::prelude::*;

use crate::state::{Side, Store};
use crate::theme_tokens::{HEADER_H, border, fg, fg_muted, icon_color, island_bg, row_hover_bg};
use crate::ui::changes::changes_view;
use crate::ui::commit_box::commit_box;
use crate::ui::icons::{Icon, icon};
use crate::ui::project::project_view;

/// The sidebar: the tool window header, then the pane of the open section.
pub fn sidebar(store: Store) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(tool_window_header(store))
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        // Both panes stay mounted and the one off show is hidden: a pane
        // rebuilt on every switch would lose what the reader left in it —
        // how far the Project tree was scrolled, where the change list was.
        // `hidden()` takes a pane out of the layout, so the one on show
        // fills the sidebar as if it were alone.
        .child(
            view()
                .flex_1()
                .min_h(px(0.0))
                .flex_col()
                .child(
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .flex_col()
                        .style_when(move || store.side.get() != Side::Project, |s| s.hidden())
                        .child(project_view(store)),
                )
                .child(
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .flex_col()
                        .style_when(move || store.side.get() != Side::Commit, |s| s.hidden())
                        .child(commit_pane(store)),
                ),
        )
}

/// The header of the sidebar island: the icon and name of the section on
/// show, then the close cross and the options.
///
/// The name and the glyph are rebuilt through `dynamic`: a component's body
/// runs once, so a `match` on the signal here would freeze on the section
/// the sidebar happened to open with.
fn tool_window_header(store: Store) -> impl IntoElement {
    let side = store.side;
    view()
        .h(px(HEADER_H - 6.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1p5()
        .px(px(10.0))
        .child(dynamic(move || {
            let (name, glyph) = match side.get() {
                Side::Project => ("Project", Icon::Project),
                Side::Commit => ("Commit", Icon::Commit),
            };
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap_1p5()
                .child(icon(glyph, 14.0).text_color(move || fg_muted()))
                .child(
                    text(name)
                        .flex_1()
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || fg()),
                )
                .into_element()
        }))
        .child(header_glyph(Icon::Close, "Hide the tool window"))
        .child(header_glyph(Icon::More, "Tool window options"))
}

/// A small square button of the header.
fn header_glyph(glyph: Icon, label: &'static str) -> impl IntoElement {
    view()
        .size(px(24.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .accessible(true)
        .label(label)
        .child(icon(glyph, 14.0).text_color(move || icon_color()))
}

/// The Commit section: the change list above, the commit box below.
fn commit_pane(store: Store) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .child(changes_view(store))
        .child(commit_box(store))
}
