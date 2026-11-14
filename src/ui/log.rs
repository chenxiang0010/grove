//! The Log tool window: the Branches pane, the filter bar, the commit table
//! and its graph.
//!
//! The table is virtualized: a history of a hundred thousand commits builds
//! the same handful of rows as a history of ten, because a row reads its
//! commit from the filtered list by index and the platform recycles the
//! cells.

use std::sync::Arc;

use git_core::{Commit, RefKind};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{
    ROW_H, fg, fg_muted, fg_strong, graph_color, icon_color, island_bg, row_hover_bg,
    row_selected_bg,
};
use crate::ui::frame::{bar, search_field, tool_icon, tree_chevron};
use crate::ui::graph_gutter::graph_cell;
use crate::ui::icons::{Icon, icon};
use crate::ui::{divider, empty_state};

/// The Log tab: the filter bar, the table, and the graph options beside it.
pub fn log_view(store: Store) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(filter_bar(store))
        .child(divider())
        .child(commit_list(store))
}

/// The commits that pass the filter, kept as one value so a row reads it by
/// index instead of re-checking the whole history.
fn visible_commits(store: Store) -> Memo<Arc<Vec<Commit>>> {
    let snapshot = store.snapshot;
    let filter = store.filter;
    computed(move || {
        let filter = filter.get();
        snapshot.with(|snapshot| {
            if filter.is_empty() {
                snapshot.commits.clone()
            } else {
                Arc::new(
                    snapshot
                        .commits
                        .iter()
                        .filter(|commit| filter.matches(commit))
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            }
        })
    })
}

/// The row above the table: the search field, the four filters and the
/// actions the reference puts beside them.
fn filter_bar(store: Store) -> impl IntoElement {
    let filter = store.filter;
    bar()
        .child(
            view()
                .h(px(26.0))
                .w(px(260.0))
                .flex_none()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .rounded(px(5.0))
                .border_1()
                .border_color(move || theme().input)
                .bg(move || theme().background)
                .child(icon(Icon::Search, 14.0).text_color(move || icon_color()))
                .child(
                    nsh_base::input()
                        .placeholder("Text or hash")
                        .value(move || filter.with(|filter| filter.text.clone()))
                        .on_change(move |text: &str| {
                            filter.update(|filter| filter.text = text.to_owned());
                        })
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .text_color(move || fg()),
                )
                .child(tool_icon(Icon::Regex, "Regex", || {}))
                .child(tool_icon(Icon::Case, "Match case", || {})),
        )
        .child(dropdown("Branch", move || {
            store
                .reference
                .with(|name| {
                    name.as_ref().map(|full| {
                        full.strip_prefix("refs/heads/")
                            .or_else(|| full.strip_prefix("refs/remotes/"))
                            .unwrap_or(full)
                            .to_owned()
                    })
                })
                .unwrap_or_else(|| "All".to_owned())
        }))
        .child(dropdown("User", move || {
            let author = filter.with(|filter| filter.author.clone());
            if author.is_empty() {
                "All".to_owned()
            } else {
                author
            }
        }))
        .child(dropdown("Date", || "All".to_owned()))
        .child(dropdown("Paths", move || {
            let path = filter.with(|filter| filter.path.clone());
            if path.is_empty() {
                "All".to_owned()
            } else {
                path
            }
        }))
        .child(tool_icon(Icon::Sort, "Sort", || {}))
        .child(view().flex_1())
        .child(tool_icon(Icon::Bookmark, "Bookmarks", || {}))
        .child(tool_icon(Icon::Refresh, "Refresh", move || {
            state::refresh(store)
        }))
        .child(tool_icon(Icon::Show, "Graph options", || {}))
        .child(tool_icon(Icon::Fetch, "Fetch", move || state::fetch(store)))
        .child(tool_icon(Icon::Push, "Push", move || state::push(store)))
}

/// A filter the reference shows as a label with a chevron, opening a popup.
///
/// The value is a closure so the label follows the filter after the bar is
/// built: a component runs once in NSH.
fn dropdown(label: &'static str, value: impl Fn() -> String + 'static) -> impl IntoElement {
    let value: std::rc::Rc<dyn Fn() -> String> = std::rc::Rc::new(value);
    view()
        .h(px(26.0))
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .px_1p5()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .child(text(label).text_size(px(11.0)).text_color(move || fg()))
        .child(
            text(move || {
                let value = value();
                if value == "All" { String::new() } else { value }
            })
            .text_size(px(11.0))
            .text_color(move || theme().primary),
        )
        .child(icon(Icon::ChevronDown, 10.0).text_color(move || icon_color()))
}

/// The commits, filtered, with the graph beside them.
fn commit_list(store: Store) -> impl IntoElement {
    let commits = visible_commits(store);
    let snapshot = store.snapshot;
    let graph = computed(move || snapshot.with(|snapshot| snapshot.graph.clone()));

    virtual_list(
        move || commits.with(|commits| commits.len()),
        move |at: Position| commit_row(store, commits, graph, at),
    )
    .estimated_item_size(px(ROW_H))
    .keys(move || {
        commits.with(|commits| {
            commits
                .iter()
                .map(|commit| commit.hash.clone())
                .collect::<Vec<_>>()
        })
    })
    .empty(empty_state(
        "No commits",
        "Nothing here matches the filter, or the repository has no history yet.",
    ))
    .flex_1()
    .min_h(px(0.0))
}

/// One commit: its graph row, its subject with the refs on it, its author,
/// the day it was made, and its hash.
fn commit_row(
    store: Store,
    commits: Memo<Arc<Vec<Commit>>>,
    graph: Memo<Arc<git_core::GraphLayout>>,
    at: Position,
) -> impl IntoElement {
    let target = store.target;

    // The row's own values, as closures the bindings re-read when the cell
    // shows another commit.
    let commit = move || {
        let at = at.index();
        commits.with(|commits| commits.get(at).cloned())
    };
    let subject = move || commit().map(|c| c.subject).unwrap_or_default();
    let author = move || commit().map(|c| c.author_name).unwrap_or_default();
    let date = move || {
        let now = git_core::now_seconds();
        commit().map(|c| c.relative_label(now)).unwrap_or_default()
    };
    let hash = move || commit().map(|c| c.hash).unwrap_or_default();
    let refs = move || commit().map(|c| c.refs).unwrap_or_default();
    let short = move || commit().map(|c| c.short_hash).unwrap_or_default();
    let is_selected = move || target.get().hash().is_some_and(|chosen| chosen == hash());

    let row = view()
        .h(px(ROW_H))
        .w_full()
        .flex_row()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .bg(move || {
            if is_selected() {
                row_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .on_press(move |_: &()| {
            let hash = hash();
            if !hash.is_empty() {
                state::select_commit(store, hash);
            }
        })
        .child(graph.with(move |layout| graph_cell(layout.clone(), at.index())))
        .child(
            // The subject, and the refs pointing at it pushed to the column's
            // right end, as the reference lays them out.
            view()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(subject)
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.5))
                        .text_color(move || fg_strong())
                        .truncate(),
                )
                .child(refs_for(refs)),
        )
        .child(
            text(author)
                .w(px(140.0))
                .flex_none()
                .pl(px(10.0))
                .truncate()
                .text_size(px(12.0))
                .text_color(move || fg_muted()),
        )
        .child(
            text(date)
                .w(px(104.0))
                .flex_none()
                .text_size(px(12.0))
                .text_color(move || fg_muted()),
        )
        .pr_3();

    crate::ui::menus::commit_menu(store, row, hash, short, subject)
}

/// The branch, remote and tag labels a commit carries.
fn refs_for(refs: impl Fn() -> Vec<String> + 'static + Copy) -> impl IntoElement {
    each(
        move || refs(),
        |label: &String| label.clone(),
        move |label: ReadSignal<String>, _: ReadSignal<usize>| {
            let text = label.get();
            let tint = if text.starts_with("tag:") {
                theme().color("tag").unwrap_or(theme().muted_foreground)
            } else if text.contains("->") {
                theme().color("graph-2").unwrap_or(theme().primary)
            } else if text.contains('/') {
                graph_color(4)
            } else {
                theme().primary
            };
            crate::ui::ref_badge(text, tint)
        },
    )
}

/// The Branches pane beside the log: the search field, then `HEAD`, the
/// local branches, the remotes and the tags.
pub fn branch_tree(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let reference = store.reference;
    let search = signal(String::new());

    let entries = computed(move || snapshot.with(|snapshot| snapshot.refs.clone()));
    let matches = move |entry: &git_core::RefEntry| {
        let needle = search.with(|needle| needle.to_lowercase());
        needle.is_empty() || entry.short_name.to_lowercase().contains(&needle)
    };

    view()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .flex_col()
        .bg(move || island_bg())
        .child(bar().child(search_field(
            "Branch or tag",
            move || search.get(),
            move |text: &str| search.set(text.to_owned()),
        )))
        .child(
            scroll().flex_1().child(
                view()
                    .flex_col()
                    .w_full()
                    .pb_2()
                    .child(section(Icon::Star, "HEAD (Current Branch)"))
                    .child(each(
                        move || {
                            entries.with(|entries| {
                                entries
                                    .iter()
                                    .filter(|entry| entry.is_head)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                        },
                        |entry: &git_core::RefEntry| entry.full_name.clone(),
                        move |entry: ReadSignal<git_core::RefEntry>, _: ReadSignal<usize>| {
                            let entry = entry.get();
                            branch_row(store, reference, entry, 1)
                        },
                    ))
                    .child(section(Icon::Branch, "Local"))
                    .child(each(
                        move || {
                            entries.with(|entries| {
                                entries
                                    .iter()
                                    .filter(|entry| {
                                        entry.kind == RefKind::LocalBranch && !entry.is_head
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                        },
                        |entry: &git_core::RefEntry| entry.full_name.clone(),
                        move |entry: ReadSignal<git_core::RefEntry>, _: ReadSignal<usize>| {
                            let entry = entry.get();
                            if matches(&entry) {
                                branch_row(store, reference, entry, 1).into_element()
                            } else {
                                view().h(px(0.0)).into_element()
                            }
                        },
                    ))
                    .child(section(Icon::Push, "Remote"))
                    .child(remote_rows(store, reference, entries, search))
                    .child(section(Icon::Tag, "Tags"))
                    .child(each(
                        move || {
                            entries.with(|entries| {
                                entries
                                    .iter()
                                    .filter(|entry| entry.kind == RefKind::Tag)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                        },
                        |entry: &git_core::RefEntry| entry.full_name.clone(),
                        move |entry: ReadSignal<git_core::RefEntry>, _: ReadSignal<usize>| {
                            let entry = entry.get();
                            if matches(&entry) {
                                branch_row(store, reference, entry, 1).into_element()
                            } else {
                                view().h(px(0.0)).into_element()
                            }
                        },
                    )),
            ),
        )
}

/// The remotes, each a folder row with its branches under it, as the
/// reference nests them.
fn remote_rows(
    store: Store,
    reference: Signal<Option<String>>,
    entries: Memo<Arc<Vec<git_core::RefEntry>>>,
    search: Signal<String>,
) -> impl IntoElement {
    // The remote names, in the order they first appear.
    let names = computed(move || {
        entries.with(|entries| {
            let mut names: Vec<String> = Vec::new();
            for entry in entries.iter() {
                if entry.kind != RefKind::RemoteBranch {
                    continue;
                }
                let remote = entry
                    .short_name
                    .split('/')
                    .next()
                    .unwrap_or("origin")
                    .to_owned();
                if !names.contains(&remote) {
                    names.push(remote);
                }
            }
            names
        })
    });

    view().flex_col().w_full().child(each(
        move || names.with(|names| names.clone()),
        |name: &String| name.clone(),
        move |name: ReadSignal<String>, _: ReadSignal<usize>| {
            let remote = name.get();
            let for_press = remote.clone();
            let head = view()
                .h(px(ROW_H))
                .w_full()
                .flex_row()
                .items_center()
                .gap_1()
                .pl(px(6.0))
                .pr_2()
                .cursor_pointer()
                .hover(|s| s.bg(move || row_hover_bg()))
                .child(tree_chevron(true, true))
                .child(icon(Icon::Folder, 14.0).text_color(move || icon_color()))
                .child(
                    text(for_press)
                        .flex_1()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(move || fg()),
                );
            view().flex_col().w_full().child(head).child(each(
                move || {
                    let remote = remote.clone();
                    entries.with(|entries| {
                        entries
                            .iter()
                            .filter(|entry| {
                                entry.kind == RefKind::RemoteBranch
                                    && entry
                                        .short_name
                                        .split('/')
                                        .next()
                                        .is_some_and(|name| name == remote)
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                },
                |entry: &git_core::RefEntry| entry.full_name.clone(),
                move |entry: ReadSignal<git_core::RefEntry>, _: ReadSignal<usize>| {
                    let entry = entry.get();
                    let needle = search.with(|needle| needle.to_lowercase());
                    if !needle.is_empty() && !entry.short_name.to_lowercase().contains(&needle) {
                        return view().h(px(0.0)).into_element();
                    }
                    branch_row(store, reference, entry, 2).into_element()
                },
            ))
        },
    ))
}

/// A group heading of the Branches tree.
fn section(glyph: Icon, label: &'static str) -> impl IntoElement {
    view()
        .h(px(24.0))
        .w_full()
        .flex_row()
        .items_center()
        .gap_1()
        .px(px(8.0))
        .child(icon(glyph, 12.0).text_color(move || icon_color()))
        .child(
            text(label)
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || fg_muted()),
        )
}

/// One branch or tag: the marker for `HEAD`, the short name, and the press
/// that narrows the log to it.
fn branch_row(
    store: Store,
    reference: Signal<Option<String>>,
    entry: git_core::RefEntry,
    depth: u32,
) -> impl IntoElement {
    let full = entry.full_name.clone();
    let short = entry.short_name.clone();
    let chosen = move || reference.get().as_deref() == Some(full.as_str());
    let is_head = entry.is_head;
    let is_tag = entry.kind == RefKind::Tag;
    let full_for_press = entry.full_name.clone();
    let short_for_menu = entry.short_name.clone();
    let row = view()
        .h(px(ROW_H))
        .w_full()
        .flex_row()
        .items_center()
        .gap_1()
        .pl(px(6.0 + 14.0 * depth as f32))
        .pr_2()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .bg(move || {
            if chosen() {
                row_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .on_press(move |_: &()| {
            reference.set(Some(full_for_press.clone()));
            state::refresh(store);
        })
        .child(
            icon(
                if is_head {
                    Icon::CurrentBranch
                } else if is_tag {
                    Icon::Tag
                } else {
                    Icon::Branch
                },
                14.0,
            )
            .text_color(move || {
                if is_head {
                    theme().primary
                } else {
                    icon_color()
                }
            }),
        )
        .child(
            text(short)
                .flex_1()
                .truncate()
                .text_size(px(12.5))
                .text_color(move || fg()),
        );

    crate::ui::menus::branch_menu(store, row, move || short_for_menu.clone(), is_head, is_tag)
}
