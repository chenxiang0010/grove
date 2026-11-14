//! The Commit section's change list: what is staged, what is not, and what
//! goes into the next commit.
//!
//! Every row carries a checkbox in front, as the reference's change list does:
//! a ticked row is committed, an unticked one is left behind. The tick is
//! what the commit action reads, so a partial commit is one press away and
//! needs no second staging step.
//!
//! The rows are grouped `Staged` above `Unstaged`, because the question a
//! reader asks of this pane is "what will the next commit contain", and one
//! sorted list does not answer it.

use git_core::{ChangeKind, StatusEntry};
use nsh_base::{checkbox, theme};
use nsh_core::prelude::*;

use crate::state::{self, Store, Target};
use crate::theme_tokens::{
    ROW_H, change_color, fg, fg_muted, icon_color, island_bg, row_hover_bg, row_selected_bg,
};
use crate::ui::frame::{bar, tool_icon};
use crate::ui::icons::{Icon, file_icon, icon};
use crate::ui::{divider, empty_state, tool_button};

/// The whole Commit section: its bar and the two groups.
pub fn changes_view(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(
            bar()
                .child(tool_button("Stage all", Icon::Add, move || {
                    stage_all(store)
                }))
                .child(tool_button("Unstage all", Icon::Remove, move || {
                    unstage_all(store)
                }))
                .child(tool_button("Stash", Icon::Stash, move || {
                    state::stash(store)
                }))
                .child(view().flex_1())
                .child(
                    text(move || {
                        let (staged, unstaged) = snapshot.with(|snapshot| {
                            (
                                snapshot.status.staged_count(),
                                snapshot.status.unstaged_count(),
                            )
                        });
                        if staged + unstaged == 0 {
                            "Clean".to_owned()
                        } else {
                            format!("{staged} staged · {unstaged} changed")
                        }
                    })
                    .text_size(px(11.0))
                    .text_color(move || fg_muted()),
                )
                .pr_2(),
        )
        .child(divider())
        .child(file_list(store))
}

/// Stages every path with an unstaged change.
fn stage_all(store: Store) {
    let paths = store.snapshot.with(|snapshot| {
        snapshot
            .status
            .unstaged()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    });
    state::stage(store, paths);
}

/// Unstages every path with a staged change.
fn unstage_all(store: Store) {
    let paths = store.snapshot.with(|snapshot| {
        snapshot
            .status
            .staged()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    });
    state::unstage(store, paths);
}

/// The two groups, or a note when the tree is clean.
fn file_list(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let staged = computed(move || {
        snapshot.with(|snapshot| snapshot.status.staged().cloned().collect::<Vec<_>>())
    });
    let unstaged = computed(move || {
        snapshot.with(|snapshot| snapshot.status.unstaged().cloned().collect::<Vec<_>>())
    });

    view().flex_1().min_h(px(0.0)).child(
        when(
            move || !staged.with(Vec::is_empty) || !unstaged.with(Vec::is_empty),
            move || {
                scroll().flex_1().child(
                    view()
                        .flex_col()
                        .w_full()
                        .pb_2()
                        .child(group(store, "Staged", staged, true))
                        .child(group(store, "Unstaged", unstaged, false)),
                )
            },
        )
        .otherwise(move || empty_state("No changes", "The working tree matches the last commit.")),
    )
}

/// One group of the change list: a heading with its count, then its rows.
fn group(
    store: Store,
    title: &'static str,
    entries: Memo<Vec<StatusEntry>>,
    staged: bool,
) -> impl IntoElement {
    let included = store.included;
    let all = move || {
        entries.with(|entries| {
            !entries.is_empty()
                && entries
                    .iter()
                    .all(|entry| included.with(|set| set.contains(&entry.path)))
        })
    };
    let some = move || {
        entries.with(|entries| {
            entries
                .iter()
                .any(|entry| included.with(|set| set.contains(&entry.path)))
        })
    };

    view().flex_col().w_full().child(
        when(
            move || !entries.with(Vec::is_empty),
            move || {
                view()
                    .flex_col()
                    .w_full()
                    .child(
                        view()
                            .h(px(24.0))
                            .w_full()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .child(
                                // The group's box ticks or clears every row
                                // in it, as a tree's parent box does.
                                checkbox()
                                    .checked(move || all())
                                    .indeterminate(move || !all() && some())
                                    .accessibility_label(title)
                                    .size(px(14.0))
                                    .on_change(move |next: &bool| {
                                        let next = *next;
                                        let paths = entries.with(|entries| {
                                            entries
                                                .iter()
                                                .map(|entry| entry.path.clone())
                                                .collect::<Vec<_>>()
                                        });
                                        for path in paths {
                                            state::set_included(store, path, next);
                                        }
                                    })
                                    .indicator(
                                        view()
                                            .size(px(14.0))
                                            .rounded(px(3.0))
                                            .border_1()
                                            .border_color(move || theme().border)
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                icon(Icon::Check, 10.0)
                                                    .text_color(move || theme().primary_foreground),
                                            ),
                                    )
                                    .bg(move || {
                                        if all() {
                                            theme().primary
                                        } else {
                                            AdaptiveColor::TRANSPARENT
                                        }
                                    }),
                            )
                            .child(
                                text(title)
                                    .text_size(px(10.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || fg_muted()),
                            )
                            .child(view().flex_1())
                            .child(crate::ui::badge(move || {
                                entries.with(|entries| entries.len().to_string())
                            })),
                    )
                    .child(each(
                        move || entries.get(),
                        |entry: &StatusEntry| entry.path.clone(),
                        move |entry: ReadSignal<StatusEntry>, _: ReadSignal<usize>| {
                            change_row(store, entry, staged)
                        },
                    ))
            },
        )
        .otherwise(|| view().h(px(0.0))),
    )
}

/// One file of the change list: its box, its icon, its name, its folder, the
/// change letter and the actions that apply to it.
fn change_row(store: Store, entry: ReadSignal<StatusEntry>, staged: bool) -> impl IntoElement {
    let target = store.target;
    let included = store.included;
    let path = move || entry.with(|entry| entry.path.clone());
    let selected = move || {
        matches!(target.get(), Target::Change { path: chosen, staged: chosen_staged }
            if chosen == path() && chosen_staged == staged)
    };
    let ticked = move || included.with(|set| set.contains(&path()));
    let untracked = move || entry.with(|entry| entry.display_kind() == ChangeKind::Untracked);

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
            // A press opens the change's diff in a tab, which is where the
            // reference shows a change; the Git area follows it too.
            let path = path();
            state::select_change(store, path.clone(), staged);
            state::open_change_tab(store, path, staged);
        })
        .child(
            // The box decides whether this row goes into the next commit.
            checkbox()
                .checked(ticked)
                .accessibility_label("Include in commit")
                .size(px(14.0))
                .on_change(move |next: &bool| {
                    let next = *next;
                    state::set_included(store, path(), next);
                })
                .indicator(
                    view()
                        .size(px(14.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(move || theme().border)
                        .items_center()
                        .justify_center()
                        .child(
                            icon(Icon::Check, 10.0).text_color(move || theme().primary_foreground),
                        ),
                )
                .bg(move || {
                    if ticked() {
                        theme().primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }),
        )
        .child(file_icon(&path(), 16.0))
        .child(
            text(move || entry.with(|entry| entry.name().to_owned()))
                .flex_none()
                .text_size(px(12.5))
                .text_color(move || fg()),
        )
        .child(
            text(move || entry.with(|entry| entry.dir().to_owned()))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(11.0))
                .text_color(move || fg_muted()),
        )
        .child(
            text(move || entry.with(|entry| entry.display_kind().letter()))
                .flex_none()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || change_color(entry.with(|entry| entry.display_kind()))),
        )
        .child(actions(store, entry, staged, untracked));

    crate::ui::menus::change_menu(store, row, path, move || staged)
}

/// The per-row actions: stage or unstage, discard, and for an untracked file
/// also ignore and delete.
fn actions(
    store: Store,
    entry: ReadSignal<StatusEntry>,
    staged: bool,
    untracked: impl Fn() -> bool + 'static + Copy,
) -> impl IntoElement {
    let path = move || entry.with(|entry| entry.path.clone());

    let toggle: AnyElement = if staged {
        tool_icon(Icon::Remove, "Unstage", move || {
            state::unstage(store, vec![path()]);
        })
        .into_element()
    } else {
        tool_icon(Icon::Add, "Stage", move || {
            state::stage(store, vec![path()]);
        })
        .into_element()
    };

    view()
        .flex_row()
        .items_center()
        .gap_0p5()
        .child(toggle)
        .child(tool_icon(Icon::Revert, "Discard", move || {
            state::discard(store, vec![path()]);
        }))
        .child(
            when(untracked, move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_0p5()
                    .child(tool_icon(Icon::Close, "Ignore", move || {
                        state::ignore(store, vec![path()]);
                    }))
                    .child(tool_icon(Icon::Delete, "Delete", move || {
                        state::delete(store, vec![path()]);
                    }))
            })
            .otherwise(|| view().h(px(0.0))),
        )
        .child(icon(Icon::More, 12.0).text_color(move || icon_color()))
}
