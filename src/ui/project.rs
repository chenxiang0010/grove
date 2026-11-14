//! The Project tool window: the repository's working tree, as a tree.
//!
//! The reference lists the project as a root row — the folder's name, then
//! its path — and under it the working tree's folders and files, each with
//! the icon its type gets and, for a changed file, its change letter. This
//! pane answers "what is in this repository" and "what did I touch" at once.

use std::collections::HashSet;

use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{
    HEADER_H, ROW_H, change_color, fg, fg_muted, fg_strong, icon_color, island_bg, row_hover_bg,
    row_selected_bg,
};
use crate::ui::icons::{Icon, file_icon, icon};

/// One row the pane draws: a folder or a file, with its depth.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: String,
    path: String,
    depth: u32,
    folder: bool,
    /// The change letter, for a file the working tree has touched.
    change: Option<git_core::ChangeKind>,
}

/// The Project pane: its header, the project root row, and the tree under it.
pub fn project_view(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let expanded = store.expanded;

    let root_name = move || snapshot.with(|snapshot| snapshot.name.clone());
    let root_path = move || {
        snapshot.with(|snapshot| {
            snapshot
                .paths
                .as_ref()
                .map(|paths| paths.root.display().to_string())
                .unwrap_or_default()
        })
    };
    let root_open = move || expanded.with(|open| open.contains(""));

    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(
            view()
                .h(px(HEADER_H - 8.0))
                .w_full()
                .flex_none()
                .flex_row()
                .items_center()
                .gap_1()
                .pl(px(4.0))
                .pr_2()
                .rounded(px(5.0))
                .mx_1()
                .bg(move || row_selected_bg())
                .cursor_pointer()
                .on_press(move |_: &()| {
                    expanded.update(|open| {
                        if !open.remove("") {
                            open.insert(String::new());
                        }
                    });
                })
                .child(
                    view()
                        .w(px(12.0))
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .child(dynamic(move || {
                            icon(
                                if root_open() {
                                    Icon::ChevronDown
                                } else {
                                    Icon::ChevronRight
                                },
                                12.0,
                            )
                            .text_color(move || icon_color())
                            .into_element()
                        })),
                )
                .child(icon(Icon::Folder, 16.0).text_color(move || icon_color()))
                .child(
                    text(root_name)
                        .flex_none()
                        .max_w(px(160.0))
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || fg_strong()),
                )
                .child(
                    text(root_path)
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(move || fg_muted()),
                ),
        )
        .child(
            scroll()
                .flex_1()
                .child(view().flex_col().w_full().pb_2().child(project_tree(store))),
        )
}

/// The rows under the root: the working tree, one level at a time, with the
/// folders that are open showing their contents.
fn project_tree(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let expanded = store.expanded;
    let root = move || {
        snapshot.with(|snapshot| {
            snapshot
                .paths
                .as_ref()
                .map(|paths| paths.root.clone())
                .unwrap_or_default()
        })
    };

    // Every row, rebuilt when the snapshot or the set of open folders
    // changes. A repository's working tree is thousands of entries at most,
    // and the pane only lists the open ones, so this is cheap and keeps the
    // pane a plain list rather than a second virtualized view.
    let rows = computed(move || {
        let open = expanded.get();
        let root = root();
        let mut out: Vec<Row> = Vec::new();
        let changes = snapshot.with(|snapshot| snapshot.status.entries.clone());
        walk(&root, "", 0, &open, &changes, &mut out, 0);
        out
    });

    each(
        move || rows.with(|rows| rows.clone()),
        |row: &Row| row.path.clone(),
        move |row: ReadSignal<Row>, _: ReadSignal<usize>| {
            // Every field is read through the row's signal inside a binding:
            // a refresh replaces the row's item, and the text, the letter and
            // the chevron follow. Reading them here once would freeze the row
            // on the state it was made with.
            let path = move || row.with(|row| row.path.clone());
            let folder = move || row.with(|row| row.folder);
            let shown = move || row.with(|row| row.name.clone());
            let depth = row.with(|row| row.depth);
            let tint = move || row.with(|row| row.change).map(change_color);
            let is_open = move || {
                let path = row.with(|row| row.path.clone());
                expanded.with(|open| open.contains(&path))
            };
            let picked = move || state::is_picked(store, &path());

            let press_path = path();
            let chevron_path = press_path.clone();

            let line = view()
                .h(px(ROW_H))
                .w_full()
                .flex_row()
                .items_center()
                .gap_1()
                // The root row starts one chevron slot in, so a child at depth
                // 0 is indented one slot past the root's own chevron and its
                // folder icon lands under the root's. The step is the
                // reference's 20pt per level. `mx_1` matches the root row's
                // margin, or the two left edges drift apart.
                .pl(px(4.0 + 20.0 * (depth + 1) as f32))
                .pr_2()
                .mx_1()
                .rounded(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(move || row_hover_bg()))
                // The picked rows are the selection Ctrl+D acts on: a plain
                // press picks this one alone, Ctrl+Click adds it or takes it
                // out.
                .bg(move || {
                    if picked() {
                        row_selected_bg()
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                })
                .on_press({
                    let press_path = press_path.clone();
                    move |press: &PressEvent| {
                        state::pick_path(store, press_path.clone(), press.modifiers.ctrl);
                        // A plain press only picks the row: the picked rows
                        // are what Ctrl+D compares, and a press that opened a
                        // file would make picking two files impossible. A
                        // double press opens it, as the reference's tree does;
                        // the chevron beside a folder opens and closes it on
                        // its own press.
                        if press.clicks != 2 {
                            return;
                        }
                        if folder() {
                            let path = press_path.clone();
                            expanded.update(|open| {
                                if !open.remove(&path) {
                                    open.insert(path);
                                }
                            });
                        } else {
                            // A file opens in an editor tab, as the reference
                            // opens a file from the Project tree.
                            state::open_file_tab(store, press_path.clone());
                        }
                    }
                })
                .child(dynamic(move || {
                    if folder() {
                        let toggle_path = chevron_path.clone();
                        view()
                            .w(px(12.0))
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_press(move |_: &()| {
                                expanded.update(|open| {
                                    if !open.remove(&toggle_path) {
                                        open.insert(toggle_path.clone());
                                    }
                                });
                            })
                            .child(
                                icon(
                                    if is_open() {
                                        Icon::ChevronDown
                                    } else {
                                        Icon::ChevronRight
                                    },
                                    12.0,
                                )
                                .text_color(move || icon_color()),
                            )
                            .into_element()
                    } else {
                        view().w(px(12.0)).flex_none().into_element()
                    }
                }))
                .child(dynamic(move || {
                    if folder() {
                        icon(Icon::Folder, 16.0)
                            .text_color(move || icon_color())
                            .into_element()
                    } else {
                        file_icon(&shown(), 16.0).into_element()
                    }
                }))
                .child(
                    text(shown)
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(move || tint().unwrap_or_else(fg)),
                )
                .child(dynamic(move || match row.with(|row| row.change) {
                    // A folder is tinted while anything under it changed, but
                    // it carries no letter of its own: the letter names what
                    // happened to one file, and a folder did not change.
                    Some(kind) if !folder() => text(kind.letter())
                        .flex_none()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || change_color(kind))
                        .into_element(),
                    _ => view().into_element(),
                }));

            // A file's right-click menu carries the Git actions that apply
            // to one path; a folder's carries its own: what is made inside
            // it, its history, and the comparison of it against another
            // folder on disk.
            let changed = move || row.with(|row| row.change.is_some());
            let menu_path = path;
            if folder() {
                crate::ui::menus::project_folder_menu(store, line, menu_path, changed)
                    .into_element()
            } else {
                crate::ui::menus::project_file_menu(store, line, menu_path, changed).into_element()
            }
        },
    )
}

/// Walks `dir` into `out`, descending into the folders `open` names.
fn walk(
    dir: &std::path::Path,
    prefix: &str,
    depth: u32,
    open: &HashSet<String>,
    changes: &[git_core::StatusEntry],
    out: &mut Vec<Row>,
    budget: usize,
) {
    if depth > 24 || out.len() > 4000 {
        return;
    }
    let _ = budget;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut folders: Vec<(String, String)> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" {
            continue;
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            folders.push((name, path));
        } else {
            files.push((name, path));
        }
    }
    folders.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    for (name, path) in folders {
        let is_open = open.contains(&path);
        let change = change_under(changes, &path);
        let child_dir = dir.join(&name);
        out.push(Row {
            name,
            path: path.clone(),
            depth,
            folder: true,
            change,
        });
        if is_open {
            walk(&child_dir, &path, depth + 1, open, changes, out, budget);
        }
    }
    for (name, path) in files {
        let change = changes
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.display_kind());
        out.push(Row {
            name,
            path,
            depth,
            folder: false,
            change,
        });
    }
}

/// The change of the first entry at or under `prefix`.
fn change_under(entries: &[git_core::StatusEntry], prefix: &str) -> Option<git_core::ChangeKind> {
    let needle = format!("{prefix}/");
    entries
        .iter()
        .find(|entry| entry.path == prefix || entry.path.starts_with(&needle))
        .map(|entry| entry.display_kind())
}
