//! The Stashes tool window: the stash list and the two actions each entry
//! has.

use git_core::StashEntry;
use nsh_core::prelude::*;

use crate::state::{self, Store, Target};
use crate::theme_tokens::{
    ROW_H, fg, fg_muted, icon_color, island_bg, row_hover_bg, row_selected_bg,
};
use crate::ui::empty_state;
use crate::ui::frame::{bar, tool_icon};
use crate::ui::icons::{Icon, icon};

/// The Stashes list beside the log.
pub fn stashes_view(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let stashes = computed(move || snapshot.with(|snapshot| snapshot.stashes.clone()));

    view()
        .flex_1()
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(
            bar()
                .child(
                    text("Stashes")
                        .flex_1()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || fg()),
                )
                .child(tool_icon(Icon::Refresh, "Refresh", move || {
                    state::refresh(store)
                })),
        )
        .child(
            view()
                .h(px(1.0))
                .w_full()
                .flex_none()
                .bg(move || crate::theme_tokens::border()),
        )
        .child(
            when(
                move || !stashes.with(|stashes| stashes.is_empty()),
                move || {
                    scroll()
                        .flex_1()
                        .child(view().flex_col().w_full().p_1().child(each(
                            move || (*stashes.get()).clone(),
                            |stash: &StashEntry| stash.index,
                            move |stash: ReadSignal<StashEntry>, _: ReadSignal<usize>| {
                                stash_row(store, stash)
                            },
                        )))
                },
            )
            .otherwise(move || {
                empty_state("No stashes", "Stash the working tree to put it aside.")
            }),
        )
}

/// One stash: its message, and pop or drop.
fn stash_row(store: Store, stash: ReadSignal<StashEntry>) -> impl IntoElement {
    let target = store.target;
    let index = move || stash.with(|stash| stash.index);
    let selected = move || matches!(target.get(), Target::Stash(chosen) if chosen == index());

    let row = view()
        .h(px(ROW_H))
        .w_full()
        .flex_row()
        .items_center()
        .gap_1p5()
        .pl(px(8.0))
        .pr_2()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .bg(move || {
            if selected() {
                row_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .on_press(move |_: &()| {
            let index = index();
            state::select_stash(store, index);
            state::open_stash_tab(store, index);
        })
        .child(icon(Icon::Stash, 14.0).text_color(move || icon_color()))
        .child(
            text(move || stash.with(|stash| stash.message.clone()))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.5))
                .text_color(move || fg()),
        )
        .child(
            text(move || format!("stash@{{{}}}", index()))
                .flex_none()
                .text_size(px(10.5))
                .text_color(move || fg_muted()),
        )
        .child(tool_icon(Icon::Pull, "Pop", move || {
            state::stash_pop(store, index())
        }))
        .child(tool_icon(Icon::Delete, "Drop", move || {
            state::stash_drop(store, index())
        }));

    crate::ui::menus::stash_menu(store, row, index)
}
