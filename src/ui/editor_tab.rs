//! A file tab: the text in an `Editor`, and the bar above it with the save
//! action and the path.
//!
//! The tab owns its text in a signal, so typing updates the tab strip's dirty
//! mark in the same frame. Saving writes the file through `git-core` and
//! refreshes the change list, which is what makes the edit appear in Commit.

use nsh_base::editor;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::state::{self, Store, Tab};
use crate::theme_tokens::{BAR_H, border, fg_muted, fg_strong, island_bg};
use crate::ui::frame::tool_icon;
use crate::ui::icons::{Icon, file_icon};

/// The editor tab: its bar, and the editor filling the rest.
pub fn editor_tab(store: Store, tab: Tab) -> impl IntoElement {
    let key = tab.key.clone();
    let body = tab.text;
    let saved = tab.saved;
    let language = tab.language;
    let path = tab.path.clone();
    let title = tab.title.clone();
    // The text the change bars are measured against: the file's version in
    // the index, read when the tab opened.
    let baseline = tab.baseline.clone();
    // Both sides of the comparison are tracked reads, so the bar's note and
    // the tab strip's dot follow typing and saving alike.
    let dirty = move || body.get() != saved.get();

    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .w_full()
        .flex_col()
        .bg(move || island_bg())
        .child(
            view()
                .h(px(BAR_H))
                .w_full()
                .flex_none()
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_3()
                .child(file_icon(&title, 16.0))
                .child(
                    text(title.clone())
                        .flex_none()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || fg_strong()),
                )
                .child(
                    text(path.clone())
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(move || fg_muted()),
                )
                .child(
                    text(move || {
                        if dirty() {
                            "Unsaved".to_owned()
                        } else {
                            String::new()
                        }
                    })
                    .flex_none()
                    .text_size(px(11.0))
                    .text_color(move || theme().primary),
                )
                .child(tool_icon(Icon::Check, "Save", move || {
                    state::save_tab(store, key.clone())
                }))
                .child(tool_icon(Icon::Revert, "Reload from disk", {
                    let key = tab.key.clone();
                    let path = path.clone();
                    move || state::reload_tab(store, key.clone(), path.clone())
                })),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .w_full()
                .overflow_hidden()
                .child(
                    editor()
                        .language(language)
                        // The editor keeps its own text: it reports every edit
                        // through `on_change`, which writes the tab's signal.
                        // A controlled value would fight that report, so a
                        // reload replaces the whole tab instead and the editor
                        // is made again from the file on disk.
                        .default_value(Some(SharedString::from(body.get_untracked())))
                        .line_numbers(true)
                        .font_size(px(12.5))
                        .line_height(px(19.0))
                        // The bars beside the lines this edit has added,
                        // changed or removed, as the reference marks them.
                        // Recomputed from the text as it is typed.
                        .decorations(move || {
                            crate::editor_marks::decorations(&baseline, &body.get())
                        })
                        .gutter_decorations_width(px(4.0))
                        .size_full()
                        .on_change(move |next: &SharedString| body.set(next.to_string())),
                ),
        )
}
