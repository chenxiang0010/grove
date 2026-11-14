//! The Push dialog: what the branch would send, and the send itself.
//!
//! The reference lists the commits the local branch has that its upstream
//! does not. With nothing picked — the dialog as it opens, its focus on the
//! top row — the right pane is the tree of every path the commits touch;
//! a commit picked, the tree of that commit's own paths. Clicking a file
//! opens its diff in a dialog of its own, and the footer sends the commits,
//! with the tags the box says to take along.

use nsh_base::{SelectItem, checkbox, select, theme};
use nsh_core::prelude::*;
use std::collections::HashSet;

use crate::state::{self, Store};
use crate::theme_tokens::{border, fg, fg_muted, fg_strong, icon_color};
use crate::ui::diff::diff_pane;
use crate::ui::icons::{Icon, file_icon, icon};

/// How wide the dialog is, as the reference's is wide enough for a commit
/// message and a file tree side by side.
const WIDTH: f32 = 900.0;
/// How tall the dialog is, under the title and above the footer.
const HEIGHT: f32 = 620.0;
/// How tall the commit's message pane is, under the tree, while a commit is
/// picked — as the reference pins its details under the changes.
const DETAILS_HEIGHT: f32 = 200.0;

/// The dialog, while it is open.
pub fn push_dialog(store: Store) -> impl IntoElement {
    let open = store.push_open;
    when(
        move || open.get(),
        move || {
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(WIDTH))
                        .h(px(HEIGHT))
                        .flex_col()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .overflow_hidden()
                        .child(push_title(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(push_body(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(push_footer(store)),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}

/// The dialog's title: the app mark, the dialog's name, and the cross that
/// closes it.
fn push_title(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    view()
        .h(px(40.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .child(
            view()
                .size_5()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg(move || theme().primary)
                .child(
                    text("G")
                        .text_size(px(10.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(move || theme().primary_foreground),
                ),
        )
        .child(
            text(move || {
                let name = snapshot.with(|snapshot| snapshot.name.clone());
                format!("Push Commits to {name}")
            })
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || fg_strong()),
        )
        .child(view().flex_1())
        .child(close_button(store))
}

/// The cross that closes the dialog.
fn close_button(store: Store) -> impl IntoElement {
    view()
        .size(px(24.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| state::dismiss_push(store))
        .accessible(true)
        .label("Close")
        .child(icon(Icon::Close, 14.0).text_color(move || icon_color()))
}

/// The body: the branch chain above the commits and their file tree.
fn push_body(store: Store) -> impl IntoElement {
    view()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .flex_col()
        .child(branch_chain(store))
        .child(
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .flex_row()
                .overflow_hidden()
                .child(commit_column(store))
                .child(view().w(px(1.0)).flex_none().bg(move || border()))
                .child(file_column(store)),
        )
}

/// The chain the branch would join: the local branch, then the branch on the
/// remote, as the reference prints `main → origin : main`.
fn branch_chain(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let data = store.push_data;
    view()
        .h(px(34.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .bg(move || theme().background)
        .child(icon(Icon::Branch, 14.0).text_color(move || icon_color()))
        .child(
            text(move || snapshot.with(|snapshot| snapshot.branch.label()))
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_strong()),
        )
        .child(icon(Icon::ArrowRight, 12.0).text_color(move || fg_muted()))
        .child(
            text(move || {
                snapshot
                    .with(|snapshot| snapshot.branch.upstream.clone())
                    .unwrap_or_else(|| "No upstream".to_owned())
            })
            .text_size(px(12.0))
            .text_color(move || theme().primary),
        )
        .child(
            // What the branch sends, next to the chain it would land on.
            text(move || {
                let count = data.with(|data| data.commits.len());
                match count {
                    0 => String::new(),
                    n => format!("{n} new"),
                }
            })
            .text_size(px(10.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(move || fg_strong())
            .style_when(
                move || data.with(|data| data.commits.is_empty()),
                |s| s.hidden(),
            )
            .px_1()
            .rounded(px(4.0))
            .bg(move || theme().accent),
        )
}

/// The commits the push would send, one row each.
fn commit_column(store: Store) -> impl IntoElement {
    view()
        .w(px(360.0))
        .flex_none()
        .flex_col()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(scroll().flex_1().min_h(px(0.0)).child(commit_rows(store)))
}

/// The commit rows, or nothing where the reference's list would be.
fn commit_rows(store: Store) -> impl IntoElement {
    let data = store.push_data;
    view().w_full().flex_col().pb_1().child(dynamic(move || {
        if data.with(|data| data.commits.is_empty()) {
            return view().into_element();
        }
        view()
            .w_full()
            .flex_col()
            .child(each(
                move || data.with(|data| data.commits.clone()),
                |commit: &git_core::Commit| commit.hash.clone(),
                move |commit: ReadSignal<git_core::Commit>, index: ReadSignal<usize>| {
                    commit_row(store, commit, index)
                },
            ))
            .into_element()
    }))
}

/// One commit's row: its message, lit while it is the one picked.
///
/// The dialog lights the top row even as it opens, when nothing is picked
/// and the tree holds every commit's paths — the reference highlights the
/// first row the same way, and a click puts the pick on it for real.
fn commit_row(
    store: Store,
    commit: ReadSignal<git_core::Commit>,
    index: ReadSignal<usize>,
) -> impl IntoElement {
    let selected = store.push_selected;
    view()
        .h(px(30.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .px_3()
        .cursor_pointer()
        .bg(move || {
            let picked = selected.get() == Some(index.get());
            let focus = selected.get().is_none() && index.get() == 0;
            if picked || focus {
                theme().selection()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| {
            // Picking the picked one puts the tree back to every commit's
            // paths, which is what the dialog shows as it opens.
            if selected.get() == Some(index.get()) {
                state::select_push_commit(store, None);
            } else {
                state::select_push_commit(store, Some(index.get()));
            }
        })
        .accessible(true)
        .label(move || commit.get().subject.clone())
        .child(
            text(move || commit.get().subject.clone())
                .truncate()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// The right column: the file tree of what would go, and the picked
/// commit's message under it.
fn file_column(store: Store) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.0))
        .flex_col()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(file_tree(store))
        .child(commit_details(store))
}

/// The file tree: every path the shown commits touch, grouped by folder.
///
/// An empty tree is the dialog on a branch its remote already has, and the
/// empty state says so the way the reference's does.
fn file_tree(store: Store) -> impl IntoElement {
    let data = store.push_data;
    let selected = store.push_selected;
    view()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .flex_col()
        .overflow_hidden()
        .child(dynamic(move || {
            let pick = selected.get();
            let files = data.with(|data| match pick {
                Some(index) => data
                    .commits
                    .get(index)
                    .and_then(|commit| data.files.get(&commit.hash))
                    .map(|files| files.as_ref().clone())
                    .unwrap_or_default(),
                None => data.all_files.as_ref().clone(),
            });
            if files.is_empty() {
                return crate::ui::empty_state(
                    "No commits selected",
                    "The branch has nothing its remote does not have.",
                )
                .into_element();
            }
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .flex_col()
                .overflow_hidden()
                .child(
                    scroll()
                        .flex_1()
                        .min_h(px(0.0))
                        .child(tree_rows(store, files.len())),
                )
                .into_element()
        }))
}

/// The tree's rows: the count of the files at the top, the folders and
/// files under it.
fn tree_rows(store: Store, total: usize) -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .pb_1()
        .child(
            view()
                .h(px(26.0))
                .w_full()
                .flex_none()
                .flex_row()
                .items_center()
                .gap_1p5()
                .child(view().w(px(2.0)).flex_none())
                .child(icon(Icon::ChevronDown, 11.0).text_color(move || fg_muted()))
                .child(icon(Icon::Folder, 13.0).text_color(move || fg_muted()))
                .child(
                    text(format_files(total))
                        .text_size(px(11.5))
                        .text_color(move || fg_muted()),
                ),
        )
        .child(each(
            move || {
                let data = store.push_data;
                let collapsed = store.push_collapsed.get();
                let files = data.with(|data| match store.push_selected.get() {
                    Some(index) => data
                        .commits
                        .get(index)
                        .and_then(|commit| data.files.get(&commit.hash))
                        .map(|files| files.as_ref().clone())
                        .unwrap_or_default(),
                    None => data.all_files.as_ref().clone(),
                });
                let tree = build_tree(&files);
                let mut rows = Vec::new();
                flatten(&tree, 0, &collapsed, &mut rows);
                rows
            },
            |row: &TreeRow| match row {
                TreeRow::Dir { prefix, .. } => prefix.clone(),
                TreeRow::File { path, .. } => path.clone(),
            },
            move |row: ReadSignal<TreeRow>, _: ReadSignal<usize>| match row.get() {
                TreeRow::Dir {
                    depth,
                    name,
                    prefix,
                    count,
                } => dir_row(store, depth, name, prefix, count).into_element(),
                TreeRow::File { depth, name, path } => {
                    file_tree_row(store, depth, name, path).into_element()
                }
            },
        ))
}

/// One folder's row: its arrow, its icon, its name and its file count.
fn dir_row(
    store: Store,
    depth: usize,
    name: String,
    prefix: String,
    count: usize,
) -> impl IntoElement {
    let collapsed = store.push_collapsed;
    let prefix_to_toggle = prefix.clone();
    view()
        .h(px(26.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| {
            let mut folded = store.push_collapsed.get();
            if folded.contains(&prefix_to_toggle) {
                folded.remove(&prefix_to_toggle);
            } else {
                folded.insert(prefix_to_toggle.clone());
            }
            store.push_collapsed.set(folded);
        })
        .accessible(true)
        .label(name.clone())
        .child(view().w(px(depth as f32 * 14.0 + 2.0)).flex_none())
        .child(dynamic(move || {
            let glyph = if collapsed.get().contains(&prefix) {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            };
            icon(glyph, 11.0)
                .text_color(move || fg_muted())
                .into_element()
        }))
        .child(icon(Icon::Folder, 13.0).text_color(move || fg_muted()))
        .child(
            text(name)
                .truncate()
                .text_size(px(12.0))
                .text_color(move || fg_strong()),
        )
        .child(
            text(format_files(count))
                .text_size(px(11.5))
                .text_color(move || fg_muted()),
        )
}

/// One file's row: its icon and its name; opening it opens its diff.
fn file_tree_row(store: Store, depth: usize, name: String, path: String) -> impl IntoElement {
    let label = path.clone();
    view()
        .h(px(26.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1p5()
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| {
            if let Some(hash) = diff_hash_for(&store, &path) {
                state::open_push_diff(store, hash, path.clone());
            }
        })
        .accessible(true)
        .label(label)
        .child(view().w(px(depth as f32 * 14.0 + 14.0)).flex_none())
        .child(file_icon(&name, 13.0))
        .child(
            text(name)
                .truncate()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// The commit whose diff a file row opens: the one picked, or the newest
/// commit that touched the path while the tree shows every commit's files.
fn diff_hash_for(store: &Store, path: &str) -> Option<String> {
    let picked = store.push_selected.get_untracked().and_then(|index| {
        store
            .push_data
            .with(|data| data.commits.get(index).map(|commit| commit.hash.clone()))
    });
    picked.or_else(|| {
        store.push_data.with(|data| {
            data.commits
                .iter()
                .find(|commit| {
                    data.files
                        .get(&commit.hash)
                        .is_some_and(|files| files.iter().any(|file| file.path == path))
                })
                .map(|commit| commit.hash.clone())
        })
    })
}

/// The picked commit's message, pinned under the tree as the reference pins
/// its details; gone while the tree holds every commit's paths.
fn commit_details(store: Store) -> impl IntoElement {
    let data = store.push_data;
    let selected = store.push_selected;
    view()
        .flex_none()
        .w_full()
        .flex_col()
        .child(dynamic(move || {
            let message = data.with(|data| {
                selected.get().and_then(|index| {
                    let commit = data.commits.get(index)?;
                    let body = data.bodies.get(&commit.hash)?;
                    Some((commit.subject.clone(), body.clone()))
                })
            });
            match message {
                None => view().h(px(0.0)).into_element(),
                Some((subject, body)) => view()
                    .h(px(DETAILS_HEIGHT))
                    .w_full()
                    .flex_none()
                    .flex_col()
                    .p_3()
                    .child(
                        text(subject)
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(move || fg_strong()),
                    )
                    .child(
                        scroll().flex_1().min_h(px(0.0)).child(
                            view()
                                .w_full()
                                .flex_col()
                                .children(body.lines().map(|line| {
                                    text(line.to_owned())
                                        .text_size(px(12.0))
                                        .text_color(move || fg())
                                })),
                        ),
                    )
                    .into_element(),
            }
        }))
}

/// The diff the dialog opens for a file: a dialog of its own, over the push
/// dialog, as the reference opens its diff window for the file clicked.
pub fn push_diff_dialog(store: Store) -> impl IntoElement {
    let shown = store.push_diff;
    when(
        move || shown.get().is_some(),
        move || {
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(1100.0))
                        .h(px(760.0))
                        .flex_col()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .overflow_hidden()
                        .child(diff_title(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(diff_pane(
                            store.push_patch,
                            "",
                            "No diff",
                            "The file's patch could not be read.",
                        )),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}

/// The diff dialog's title: the file it shows, and the cross that closes it.
fn diff_title(store: Store) -> impl IntoElement {
    let shown = store.push_diff;
    view()
        .h(px(40.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .child(
            text(move || shown.get().map(|(_, path)| path).unwrap_or_default())
                .truncate()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || fg_strong()),
        )
        .child(view().flex_1())
        .child(
            view()
                .size(px(24.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(move || theme().accent))
                .on_press(move |_: &()| state::close_push_diff(store))
                .accessible(true)
                .label("Close")
                .child(icon(Icon::Close, 14.0).text_color(move || icon_color())),
        )
}

/// The footer: the tags box, the error, and the send.
fn push_footer(store: Store) -> impl IntoElement {
    let tags = store.push_tags;
    let tags_all = store.push_tags_all;
    let running = store.push_running;
    view()
        .h(px(48.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .child(
            view()
                .size(px(22.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(move || border())
                .child(text("?").text_size(px(11.0)).text_color(move || fg_muted())),
        )
        .child(
            checkbox()
                .checked(move || tags.get())
                .accessibility_label("Push tags")
                .size(px(14.0))
                .on_change(move |next: &bool| tags.set(*next))
                .indicator(
                    view()
                        .size(px(14.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(move || theme().border)
                        .items_center()
                        .justify_center()
                        // The tick shows only while the box is checked: an
                        // unchecked box with a white tick on its ground
                        // reads as checked.
                        .child(icon(Icon::Check, 10.0).text_color(move || {
                            if tags.get() {
                                theme().primary_foreground
                            } else {
                                AdaptiveColor::TRANSPARENT
                            }
                        })),
                )
                .bg(move || {
                    if tags.get() {
                        theme().primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }),
        )
        .child(
            text("Push tags:")
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
        .child(
            select()
                .items(vec![
                    SelectItem::new("all", "All"),
                    SelectItem::new("current", "Current Branch"),
                ])
                .value(move || {
                    Some(SharedString::from(if tags_all.get() {
                        "all"
                    } else {
                        "current"
                    }))
                })
                .on_change(move |value: &Option<SharedString>| {
                    tags_all.set(value.as_deref() == Some("all"));
                })
                .accessibility_label("Which tags go")
                .popup_style(crate::ui::popup_style)
                .trigger(
                    view()
                        .h(px(24.0))
                        .w(px(150.0))
                        .flex_none()
                        .px_2()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(move || theme().input)
                        .bg(move || theme().background)
                        .cursor_pointer()
                        .child(
                            text(move || {
                                if tags_all.get() {
                                    "All"
                                } else {
                                    "Current Branch"
                                }
                            })
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(move || fg()),
                        )
                        .child(icon(Icon::ChevronDown, 11.0).text_color(move || icon_color())),
                ),
        )
        .child(view().flex_1())
        .child(
            // Why the last push did not go, until the next one tries.
            text(move || store.push_error.get())
                .truncate()
                .text_size(px(11.0))
                .text_color(move || theme().destructive)
                .style_when(move || store.push_error.get().is_empty(), |s| s.hidden()),
        )
        .child(crate::ui::primary_button(
            "Push",
            move || !running.get(),
            move || state::confirm_push(store, false),
        ))
        .child(cancel_button(store))
}

/// The dialog's Cancel.
fn cancel_button(store: Store) -> impl IntoElement {
    view()
        .h(px(28.0))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| state::dismiss_push(store))
        .accessible(true)
        .label("Cancel")
        .child(
            text("Cancel")
                .text_size(px(12.0))
                .text_color(move || fg_muted()),
        )
}

/// "1 file" for one, "744 files" for the rest.
fn format_files(count: usize) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}

/// One node of the file tree the dialog renders.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeEntry {
    Dir {
        /// The segments the row names, joined the way the reference prints
        /// them: a chain of folders that holds no file of its own and has a
        /// single folder beneath it names as one, `src\UniAutos`.
        name: String,
        /// The full `/`-joined path of the first folder of the chain, which
        /// is what the collapse state is keyed by.
        prefix: String,
        /// How many files the folder and its folders hold.
        count: usize,
        children: Vec<TreeEntry>,
    },
    File {
        name: String,
        path: String,
    },
}

/// Groups `files` into the tree the dialog renders: folders first in every
/// level, each folder with the count of the files it holds.
fn build_tree(files: &[git_core::CommitFile]) -> Vec<TreeEntry> {
    let mut root: Vec<TreeEntry> = Vec::new();
    for file in files {
        let segments: Vec<&str> = file.path.split('/').collect();
        insert_file(&mut root, &segments, &file.path);
    }
    count_files(&mut root);
    with_prefixes(&mut root, "");
    compress_chains(root)
}

/// Puts one file's remaining segments into `entries`, reusing the folder the
/// first segment names when a row for it is already there.
fn insert_file(entries: &mut Vec<TreeEntry>, segments: &[&str], path: &str) {
    let Some((&first, rest)) = segments.split_first() else {
        return;
    };
    if rest.is_empty() {
        entries.push(TreeEntry::File {
            name: first.to_owned(),
            path: path.to_owned(),
        });
        return;
    }
    let index = match entries
        .iter()
        .position(|entry| matches!(entry, TreeEntry::Dir { name, .. } if name == first))
    {
        Some(index) => index,
        None => {
            entries.push(TreeEntry::Dir {
                name: first.to_owned(),
                prefix: String::new(),
                count: 0,
                children: Vec::new(),
            });
            entries.len() - 1
        }
    };
    let TreeEntry::Dir { children, .. } = &mut entries[index] else {
        return;
    };
    insert_file(children, rest, path);
}

/// Fills every folder's count with the files it and its folders hold.
fn count_files(entries: &mut [TreeEntry]) -> usize {
    let mut total = 0;
    for entry in entries.iter_mut() {
        match entry {
            TreeEntry::Dir {
                count, children, ..
            } => {
                *count = count_files(children);
                total += *count;
            }
            TreeEntry::File { .. } => total += 1,
        }
    }
    total
}

/// Names a folder chain that holds no file of its own and has a single
/// folder beneath it as one, and sorts folders before files, both by name.
fn compress_chains(entries: Vec<TreeEntry>) -> Vec<TreeEntry> {
    let mut compressed: Vec<TreeEntry> = entries.into_iter().map(compress_entry).collect();
    compressed.sort_by(|a, b| match (a, b) {
        (TreeEntry::File { .. }, TreeEntry::Dir { .. }) => std::cmp::Ordering::Greater,
        (TreeEntry::Dir { .. }, TreeEntry::File { .. }) => std::cmp::Ordering::Less,
        _ => display_name(a)
            .to_lowercase()
            .cmp(&display_name(b).to_lowercase()),
    });
    compressed
}

/// Compresses one entry: its folders first, then a lone folder beneath a
/// folder that holds no file joins the row above it.
fn compress_entry(entry: TreeEntry) -> TreeEntry {
    match entry {
        TreeEntry::Dir {
            name,
            prefix,
            count,
            children,
        } => {
            let children = compress_chains(children);
            if let [
                TreeEntry::Dir {
                    name: child_name,
                    children: grandchildren,
                    ..
                },
            ] = children.as_slice()
            {
                return TreeEntry::Dir {
                    name: format!("{name}\\{child_name}"),
                    prefix,
                    count,
                    children: grandchildren.clone(),
                };
            }
            TreeEntry::Dir {
                name,
                prefix,
                count,
                children,
            }
        }
        file => file,
    }
}

/// The name a row sorts by: a file's name, a folder chain's whole name.
fn display_name(entry: &TreeEntry) -> &str {
    match entry {
        TreeEntry::Dir { name, .. } | TreeEntry::File { name, .. } => name,
    }
}

/// One rendered row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeRow {
    Dir {
        depth: usize,
        name: String,
        prefix: String,
        count: usize,
    },
    File {
        depth: usize,
        name: String,
        path: String,
    },
}

/// Lays the tree out in rows, leaving out what a collapsed folder hides.
fn flatten(
    entries: &[TreeEntry],
    depth: usize,
    collapsed: &HashSet<String>,
    out: &mut Vec<TreeRow>,
) {
    for entry in entries {
        match entry {
            TreeEntry::Dir {
                name,
                prefix,
                count,
                children,
            } => {
                out.push(TreeRow::Dir {
                    depth,
                    name: name.clone(),
                    prefix: prefix.clone(),
                    count: *count,
                });
                if !collapsed.contains(prefix) {
                    flatten(children, depth + 1, collapsed, out);
                }
            }
            TreeEntry::File { name, path } => out.push(TreeRow::File {
                depth,
                name: name.clone(),
                path: path.clone(),
            }),
        }
    }
}

/// Fills the folder prefixes a chain collapses by: the `/`-joined path each
/// folder has in the file system, which the rows keep unique.
fn with_prefixes(entries: &mut Vec<TreeEntry>, parent: &str) {
    for entry in entries.iter_mut() {
        if let TreeEntry::Dir {
            name,
            prefix,
            children,
            ..
        } = entry
        {
            let joined = if parent.is_empty() {
                name.clone()
            } else {
                format!("{parent}/{name}")
            };
            with_prefixes(children, &joined);
            *prefix = joined;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> git_core::CommitFile {
        git_core::CommitFile {
            path: path.to_owned(),
            kind: git_core::ChangeKind::Modified,
        }
    }

    #[test]
    fn a_chain_of_folders_with_one_folder_beneath_names_as_one() {
        // `src/UniAutos` with nothing of its own and one folder beneath,
        // whose own chain joins too: the whole lone chain is one row.
        let mut tree = build_tree(&[file("src/UniAutos/Component/a.py")]);
        assert_eq!(tree.len(), 1);
        match tree.remove(0) {
            TreeEntry::Dir {
                name,
                count,
                children,
                ..
            } => {
                assert_eq!(name, "src\\UniAutos\\Component");
                assert_eq!(count, 1);
                assert_eq!(children.len(), 1);
            }
            other => panic!("expected a folder, got {other:?}"),
        }
    }

    #[test]
    fn a_folder_with_a_file_of_its_own_names_alone() {
        // `b` holds a file, so the chain stops at `a`.
        let mut tree = build_tree(&[file("a/b/c.py"), file("a/b.txt")]);
        match tree.remove(0) {
            TreeEntry::Dir {
                name,
                count,
                children,
                ..
            } => {
                assert_eq!(name, "a");
                assert_eq!(count, 2);
                assert_eq!(children.len(), 2);
            }
            other => panic!("expected a folder, got {other:?}"),
        }
    }

    #[test]
    fn folders_come_before_files_and_both_sort_by_name() {
        let mut tree = build_tree(&[file("z.txt"), file("a/d.txt"), file("a/c.txt")]);
        match tree.remove(0) {
            TreeEntry::Dir { name, children, .. } => {
                assert_eq!(name, "a");
                assert_eq!(children.len(), 2);
            }
            other => panic!("expected a folder, got {other:?}"),
        }
        match tree.remove(0) {
            TreeEntry::File { name, .. } => assert_eq!(name, "z.txt"),
            other => panic!("expected a file, got {other:?}"),
        }
    }

    #[test]
    fn the_collapse_state_hides_what_the_folder_holds() {
        let tree = build_tree(&[file("a/b.py"), file("a/c/d.py")]);
        let mut rows = Vec::new();
        flatten(&tree, 0, &HashSet::from(["a".to_owned()]), &mut rows);
        assert_eq!(rows.len(), 1);
        let mut rows = Vec::new();
        flatten(&tree, 0, &HashSet::new(), &mut rows);
        assert_eq!(rows.len(), 4);
    }
}
