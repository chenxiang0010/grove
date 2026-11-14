//! Grove: a Git client drawn with NSH.
//!
//! The window is the JetBrains *rebased* Git tool window: two sidebar
//! sections (Project, Commit) and, below them, the Git area with the log,
//! its graph and the details of what is selected. Reads go through libgit2
//! in [`git_core`]; writes go through the `git` executable, so hooks,
//! signing and filters behave as on the command line.
//!
//! # How the UI is arranged
//!
//! - [`state`] owns every signal, in one `Copy` handle the whole tree reads.
//! - [`model`] is the `Send` data the signals hold: a snapshot of the
//!   repository, read on a worker.
//! - [`ui`] is Grove's own look: every colour, size and press state.
//! - Each region is one module under `ui/`: `sidebar`, `tabs`, `editor_tab`,
//!   `log`, `changes`, `stashes`, `details`, `commit_box`, and the frame.
//!
//! # The frame
//!
//! The window is the reference IDE's *islands* layout: a flat frame colour
//! with rounded surfaces floating on it, 6pt apart. Top to bottom it is a
//! 40pt title bar, the sidebar beside the main column, the Git area (hidden
//! until the stripe's Git button is lit), and a 34pt status bar.

mod compare;
mod editor_marks;
mod model;
mod state;
mod theme_tokens;
mod ui;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::state::{Store, Target};
use crate::theme_tokens::{ISLAND_GAP, border, frame_bg, island_bg};
use crate::ui::details::details_view;
use crate::ui::frame::{gap_v, island, status_bar, stripe, title_bar};
use crate::ui::log::{branch_tree, log_view};
use crate::ui::sidebar::sidebar;
use crate::ui::tabs::{tab_body, tab_strip};
use crate::ui::welcome_screen;

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry may import from
/// `grove/native`.
#[native_module("grove/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    let store = Store::new();
    // The window opens on the last repository that still exists; opening it
    // here rather than on a press is what makes the app feel like a desktop
    // client rather than a file picker.
    if let Some(folder) = state::remembered_folder() {
        state::open_folder(store, folder);
    }
    // Ctrl+S (Command+S on Apple platforms) writes the tab on screen, as
    // every editor does. It is the window's, not the editor's: the editor
    // keeps the key for its own commands and would not pass it on.
    state::bind_save(store);
    // Ctrl+D compares the two rows the Project tree has picked: two folders
    // side by side, or the diff of two files.
    state::bind_compare(store);

    view()
        .flex_1()
        .size_full()
        .flex_col()
        .bg(move || frame_bg())
        .text_color(move || theme().foreground)
        .child(title_bar(store))
        .child(gap_v())
        .child(
            when(move || store.is_open(), move || workspace(store))
                .otherwise(move || welcome_screen(store)),
        )
        .child(gap_v())
        .child(status_bar(store))
        .child(crate::ui::prompt::prompt_dialog(store))
        .child(crate::ui::prompt::confirm_dialog(store))
        .child(crate::ui::clone_dialog::clone_dialog(store))
        .child(crate::ui::update_dialog::update_dialog(store))
        .child(crate::ui::push_dialog::push_dialog(store))
        .child(crate::ui::push_dialog::push_diff_dialog(store))
}

/// The workspace under the title bar: the activity stripe down the left
/// edge, and beside it the upper row (sidebar and main area) above the Git
/// area, which spans the full width and is only up while its stripe button
/// is lit.
fn workspace(store: Store) -> impl IntoElement {
    let git_area_open = store.git_area;
    view()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .flex_row()
        .child(stripe(store))
        .child(
            nsh_base::v_resizable()
                .handle_style(|s| s.h(px(1.0)).bg(move || frame_bg()))
                .handle_accessibility_label("Resize the Git area")
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(
                    // The upper row has no size of its own: it takes what the
                    // Git area leaves, and the whole window while the area is
                    // hidden.
                    nsh_base::resizable_panel()
                        .flex_col()
                        .min_h(px(120.0))
                        .child(upper_row(store)),
                )
                .child(
                    nsh_base::resizable_panel()
                        .size(px(380.0))
                        .size_range(px(120.0)..px(1200.0))
                        .flex_col()
                        .min_h(px(120.0))
                        .visible(move || git_area_open.get())
                        .child(git_area(store)),
                ),
        )
}

/// The sidebar beside the main area.
fn upper_row(store: Store) -> impl IntoElement {
    view().flex_1().min_h(px(0.0)).w_full().flex_row().child(
        nsh_base::h_resizable()
            .handle_style(|s| s.w(px(1.0)).bg(move || frame_bg()))
            .handle_accessibility_label("Resize the sidebar")
            .flex_1()
            .min_h(px(0.0))
            .child(
                nsh_base::resizable_panel()
                    .size(px(300.0))
                    .size_range(px(200.0)..px(560.0))
                    .flex_col()
                    .min_h(px(0.0))
                    .child(
                        view()
                            .flex_1()
                            .min_h(px(0.0))
                            .mr(px(ISLAND_GAP))
                            .mb(px(ISLAND_GAP))
                            .child(sidebar(store)),
                    ),
            )
            .child(
                nsh_base::resizable_panel()
                    .flex_col()
                    .min_w(px(420.0))
                    .min_h(px(0.0))
                    .child(main_column(store)),
            ),
    )
}

/// The main area: one island whose top is the tab strip, as the reference's
/// editor island is — the strip is the island's own header, not a second
/// island floating above it.
fn main_column(store: Store) -> impl IntoElement {
    island()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .w_full()
        .mb(px(ISLAND_GAP))
        .flex_col()
        .child(
            view()
                .flex_none()
                .h(px(36.0))
                .w_full()
                .child(tab_strip(store)),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .child(tab_body(store)),
        )
}

/// The Git area: its header, the branches pane, the log with its graph, and
/// the details.
fn git_area(store: Store) -> impl IntoElement {
    let target = store.target;

    island()
        .flex_1()
        .w_full()
        .child(crate::ui::frame::log_header(store))
        .child(
            view()
                .h(px(1.0))
                .w_full()
                .flex_none()
                .bg(move || border()),
        )
        .child(
            // The Git area's three columns share the width 2 : 4 : 3. Every
            // panel carries a size in that proportion — 240 : 480 : 360 — so
            // the room left over is handed out by the same ratio, and the
            // split survives a resize of the window. The maximums stay well
            // above what the ratio asks at any window width, or a capped
            // column would push its share onto the others.
            nsh_base::h_resizable()
                .handle_style(|s| s.w(px(1.0)).bg(move || border()))
                .handle_accessibility_label("Resize the branches list")
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(
                    nsh_base::resizable_panel()
                        .size(px(240.0))
                        .size_range(px(160.0)..px(900.0))
                        .flex_col()
                        .min_h(px(0.0))
                        .child(
                            view()
                                .flex_none()
                                .min_h(px(120.0))
                                .max_h(px(240.0))
                                .flex_col()
                                .child(crate::ui::stashes::stashes_view(store)),
                        )
                        .child(crate::ui::divider())
                        .child(
                            view()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex_col()
                                .child(branch_tree(store)),
                        ),
                )
                .child(
                    nsh_base::resizable_panel()
                        .size(px(480.0))
                        .size_range(px(240.0)..px(2000.0))
                        .flex_col()
                        .min_w(px(240.0))
                        .min_h(px(0.0))
                        .child(log_view(store)),
                )
                .child(
                    nsh_base::resizable_panel()
                        .size(px(360.0))
                        .size_range(px(240.0)..px(1200.0))
                        .min_w(px(0.0))
                        .flex_col()
                        .min_h(px(0.0))
                        .bg(move || island_bg())
                        .child(
                            when(
                                move || !matches!(target.get(), Target::None),
                                move || details_view(store),
                            )
                            .otherwise(move || {
                                crate::ui::empty_state(
                                    "Select commit to view changes",
                                    "Pick a commit in the log, a file in the changes list, or a stash.",
                                )
                            }),
                        ),
                ),
        )
}
