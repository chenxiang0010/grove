//! The main area's tab strip, and the body of whichever tab is on screen.
//!
//! A tab is opened by a press elsewhere — a file in the Project tree, a file
//! in a commit's list, a change in the Commit pane — and this strip is the
//! only place it is closed. The strip is a row of pills: the icon of what the
//! tab holds, its name, a dot while an edit is unsaved, and the close cross.

use nsh_base::editor;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::state::{self, Store, Tab, TabKind};
use crate::theme_tokens::{
    HEADER_H, MOTION_FAST, border, change_color, fg, fg_muted, fg_strong, icon_color, island_bg,
    row_hover_bg, tab_selected_bg, tab_selected_border, toolbar_hover_bg,
};
use crate::ui::editor_tab::editor_tab;
use crate::ui::icons::{Icon, file_icon, icon};

/// The strip of open tabs, above the main area.
pub fn tab_strip(store: Store) -> impl IntoElement {
    let tabs = store.tabs;
    view()
        .h(px(HEADER_H - 6.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_0p5()
        .px_2()
        .bg(move || island_bg())
        .child(
            scroll()
                .flex_1()
                .child(view().flex_row().items_center().gap_0p5().child(each(
                    move || tabs.get(),
                    |tab: &Tab| tab.key.clone(),
                    move |tab: ReadSignal<Tab>, _: ReadSignal<usize>| tab_pill(store, tab),
                ))),
        )
}

/// One tab: its icon, its name, its dirty dot and its close cross.
fn tab_pill(store: Store, tab: ReadSignal<Tab>) -> impl IntoElement {
    let active = store.active;
    let snapshot = store.snapshot;
    let key = move || tab.with(|tab| tab.key.clone());
    let selected = move || active.get().as_deref() == Some(key().as_str());
    let title = move || tab.with(|tab| tab.title.clone());
    let kind = move || tab.with(|tab| tab.kind);
    let path = move || tab.with(|tab| tab.path.clone());
    let dirty = move || tab.with(Tab::is_dirty);
    // A file the working tree has touched carries its change colour on its
    // name, as the reference colours a changed file's tab: blue for a
    // modification, green for an addition, and so on.
    let change = move || {
        let path = tab.with(|tab| tab.path.clone());
        if path.is_empty() {
            return None;
        }
        snapshot.with(|snapshot| {
            snapshot
                .status
                .entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.display_kind())
        })
    };

    let close_key = key();
    let press_key = key();

    view()
        .h(px(26.0))
        .max_w(px(220.0))
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .rounded(px(6.0))
        .cursor_pointer()
        .border_1()
        .border_color(move || {
            if selected() {
                tab_selected_border()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .bg(move || {
            if selected() {
                tab_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .hover(|s| s.bg(move || row_hover_bg()))
        .transition_all()
        .duration(MOTION_FAST)
        .on_press(move |_: &()| state::activate_tab(store, press_key.clone()))
        .child(match kind() {
            TabKind::File => file_icon(&path(), 14.0).into_element(),
            TabKind::Diff => icon(Icon::Diff, 14.0)
                .text_color(move || icon_color())
                .into_element(),
            TabKind::Blame => icon(Icon::History, 14.0)
                .text_color(move || icon_color())
                .into_element(),
            TabKind::Compare => icon(Icon::Diff, 14.0)
                .text_color(move || icon_color())
                .into_element(),
        })
        .child(
            text(title)
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || match change() {
                    Some(kind) => change_color(kind),
                    None if selected() => fg_strong(),
                    None => fg(),
                }),
        )
        .child(
            when(dirty, move || {
                view()
                    .size_1p5()
                    .flex_none()
                    .rounded(px(3.0))
                    .bg(move || theme().primary)
            })
            .otherwise(|| view().size(px(0.0))),
        )
        .child(
            view()
                .size_4()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(move || toolbar_hover_bg()))
                .on_press(move |_: &()| state::close_tab(store, close_key.clone()))
                .accessible(true)
                .label("Close tab")
                .child(icon(Icon::Close, 12.0).text_color(move || icon_color())),
        )
}

/// The body of the tab on screen, or a hint while none is open.
pub fn tab_body(store: Store) -> impl IntoElement {
    let tabs = store.tabs;
    let active = store.active;

    let shown = computed(move || {
        let key = active.get()?;
        tabs.with(|tabs| tabs.iter().find(|tab| tab.key == key).cloned())
    });

    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .w_full()
        .flex_col()
        .bg(move || island_bg())
        .child(
            when(
                move || shown.with(|tab| tab.is_some()),
                move || {
                    dynamic(move || match shown.with(|tab| tab.clone()) {
                        Some(tab) => tab_content(store, tab).into_element(),
                        None => view().into_element(),
                    })
                },
            )
            .otherwise(|| {
                view()
                    .flex_1()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(
                        text("No open tabs")
                            .text_size(px(13.0))
                            .text_color(move || fg_muted()),
                    )
                    .child(
                        text("Open a file from Project, or a change from Commit.")
                            .text_size(px(11.5))
                            .text_color(move || fg_muted().opacity(0.75)),
                    )
            }),
        )
}

/// What one tab draws: an editor, or a patch.
fn tab_content(store: Store, tab: Tab) -> AnyElement {
    match tab.kind {
        TabKind::File => editor_tab(store, tab).into_element(),
        TabKind::Diff => {
            let body = tab.text;
            let language = tab.language;
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .flex_col()
                .child(
                    view()
                        .h(px(24.0))
                        .w_full()
                        .flex_none()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .child(
                            text(tab.title.clone())
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || fg()),
                        )
                        .child(
                            text(match &tab.commit {
                                Some(hash) => format!("{hash} · {}", tab.path),
                                None => tab.path.clone(),
                            })
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_size(px(11.0))
                            .text_color(move || fg_muted()),
                        ),
                )
                .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                .child(match tab.staged {
                    Some(staged) => crate::ui::diff::diff_pane_with_hunks(
                        store,
                        body,
                        language,
                        tab.path.clone(),
                        staged,
                        "No changes",
                        "This file has no difference to show.",
                    )
                    .into_element(),
                    None => crate::ui::diff::diff_pane(
                        body,
                        language,
                        "No changes",
                        "This file has no difference to show.",
                    )
                    .into_element(),
                })
                .into_element()
        }
        TabKind::Blame => {
            // A blame is a reading of history, so it opens in a plain editor
            // with no gutter marks and no save: the columns are the text.
            let body = tab.text;
            let language = tab.language;
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .flex_col()
                .child(
                    view()
                        .h(px(24.0))
                        .w_full()
                        .flex_none()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .child(
                            text(tab.title.clone())
                                .flex_1()
                                .min_w(px(0.0))
                                .truncate()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || fg()),
                        )
                        .child(
                            text(tab.path.clone())
                                .flex_none()
                                .text_size(px(11.0))
                                .text_color(move || fg_muted()),
                        ),
                )
                .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                .child(
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .w_full()
                        .overflow_hidden()
                        .child(
                            editor()
                                .language(language)
                                .default_value(Some(SharedString::from(body.get_untracked())))
                                .line_numbers(true)
                                .read_only(true)
                                .font_size(px(12.0))
                                .line_height(px(19.0))
                                .size_full(),
                        ),
                )
                .into_element()
        }
        TabKind::Compare => match (tab.comparison, tab.compare_mode, tab.compared) {
            (Some(comparison), Some(mode), Some(compared)) => {
                crate::ui::compare::compare_view(store, comparison, mode, compared).into_element()
            }
            _ => crate::ui::empty_state(
                "No comparison",
                "The tab lost the two folders it was comparing.",
            )
            .into_element(),
        },
    }
}
