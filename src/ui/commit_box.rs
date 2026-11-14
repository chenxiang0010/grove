//! The commit box under the log: the message, the amend toggle, and the
//! button that makes the commit.
//!
//! It stays visible whenever the repository is open, because a commit is the
//! one action a Git client exists for and it should never be a search away.

use nsh_base::{input, theme};
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::ROW_H;
use crate::ui::icons::Icon;
use crate::ui::{divider, primary_button, toggle_chip};

/// The commit box: the message field, the toggles, the commit button.
pub fn commit_box(store: Store) -> impl IntoElement {
    let message = store.message;
    let amend = store.amend;
    let snapshot = store.snapshot;
    let included = store.included;

    // The button counts what the boxes ticked, not what the index holds: the
    // press stages the ticked paths itself, so a partial commit is one step.
    let picked = move || {
        snapshot.with(|snapshot| {
            snapshot
                .status
                .entries
                .iter()
                .filter(|entry| included.with(|set| set.contains(&entry.path)))
                .count()
        })
    };
    let can_commit = move || !message.get().trim().is_empty() && (picked() > 0 || amend.get());

    view()
        .flex_none()
        .w_full()
        .flex_col()
        .child(divider())
        .child(
            view()
                .flex_col()
                .w_full()
                .gap_1()
                .p_2()
                .child(
                    input()
                        .multiline(true)
                        .placeholder("Commit message")
                        .value(move || message.get())
                        .on_change(move |text: &str| message.set(text.to_owned()))
                        .min_h(px(ROW_H * 2.0))
                        .max_h(px(ROW_H * 5.0))
                        .px_2()
                        .py_1p5()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(move || theme().input)
                        .bg(move || theme().background)
                        .text_size(px(12.0))
                        .text_color(move || theme().foreground),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(toggle_chip(
                            "Amend",
                            move || amend.get(),
                            move || amend.update(|on| *on = !*on),
                        ))
                        .child(toggle_chip("All", move || picked() > 0, {
                            let snapshot = store.snapshot;
                            move || {
                                let any =
                                    snapshot.with(|snapshot| {
                                        snapshot.status.entries.iter().any(|entry| {
                                            included.with(|set| set.contains(&entry.path))
                                        })
                                    });
                                state::set_all_included(store, !any);
                            }
                        }))
                        .child(view().flex_1())
                        .child(
                            text(move || {
                                let picked = picked();
                                if picked == 0 {
                                    "Nothing selected".to_owned()
                                } else {
                                    format!("{picked} in this commit")
                                }
                            })
                            .text_size(px(10.0))
                            .text_color(move || theme().muted_foreground),
                        )
                        .child(primary_button("Commit", can_commit, move || {
                            state::commit(store)
                        }))
                        .child(crate::ui::menus::commit_box_menu(
                            store,
                            crate::ui::frame::tool_icon(Icon::More, "More commit actions", || {}),
                        )),
                ),
        )
}
