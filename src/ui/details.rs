//! The Git area's right pane: what the selected commit or stash holds.
//!
//! The reference splits it two ways: a 32pt bar of actions, and the commit's
//! message with the files it changed nested under it. Pressing a file opens
//! its patch in a tab of the main area, which is where the diff lives; a
//! stash shows its patch here, because a stash has no commit to open.
//!
//! The file list is a real tree: a folder shows the number of files under
//! it, and its subfolders nest inside it, so a commit touching a hundred
//! paths still reads as a handful of folders.

use std::collections::HashSet;

use git_core::ChangeKind;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::state::{self, Store, Target};
use crate::theme_tokens::{
    ROW_H, border, change_color, fg, fg_muted, fg_strong, icon_color, island_bg, row_hover_bg,
};
use crate::ui::frame::{bar, tool_icon};
use crate::ui::icons::{Icon, file_icon, icon};
use crate::ui::{divider, ref_badge};

/// The whole right pane: the bar, the commit block, the file tree, and a
/// stash's patch.
pub fn details_view(store: Store) -> impl IntoElement {
    let target = store.target;
    view()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex_col()
        .bg(move || island_bg())
        .child(details_bar(store))
        .child(divider())
        .child(summary(store))
        .child(file_tree(store))
        .child(when(
            move || matches!(target.get(), Target::Stash(_)),
            move || {
                crate::ui::diff::diff_pane(
                    store.patch,
                    "",
                    "No changes",
                    "This stash holds nothing to show.",
                )
            },
        ))
}

/// The pane's own toolbar: the actions the reference puts above the files.
fn details_bar(store: Store) -> impl IntoElement {
    let hash = store.target;
    bar()
        .child(tool_icon(Icon::Revert, "Revert commit", {
            let target = hash;
            move || {
                if let Target::Commit(hash) = target.get_untracked() {
                    state::revert_commit(store, hash);
                }
            }
        }))
        .child(tool_icon(Icon::Copy, "Copy commit hash", {
            let target = hash;
            move || {
                if let Target::Commit(hash) = target.get_untracked() {
                    state::copy(store, hash, "the hash");
                }
            }
        }))
        .child(tool_icon(Icon::History, "Filter the log to this commit", {
            let target = hash;
            move || {
                if let Target::Commit(hash) = target.get_untracked() {
                    let short: String = hash.chars().take(8).collect();
                    store.filter.update(|filter| filter.text = short);
                }
            }
        }))
        .child(view().flex_1())
        .child(tool_icon(Icon::Close, "Clear the selection", move || {
            store.target.set(Target::None)
        }))
}

/// What the pane is about: a commit's message, or a stash's.
fn summary(store: Store) -> impl IntoElement {
    let target = store.target;
    let details = store.details;
    let snapshot = store.snapshot;

    view()
        .flex_none()
        .w_full()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .child(
            text(move || match target.get() {
                Target::None => "Nothing selected".to_owned(),
                Target::Commit(_) => details
                    .with(|details| {
                        details
                            .as_ref()
                            .map(|details| details.commit.subject.clone())
                    })
                    .unwrap_or_else(|| "Reading…".to_owned()),
                Target::Change { path, .. } => path,
                Target::Stash(index) => snapshot.with(|snapshot| {
                    snapshot
                        .stashes
                        .iter()
                        .find(|stash| stash.index == index)
                        .map(|stash| stash.message.clone())
                        .unwrap_or_default()
                }),
            })
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || fg_strong()),
        )
        .child(
            text(move || match target.get() {
                Target::Commit(_) => details
                    .with(|details| {
                        details.as_ref().map(|details| {
                            let commit = &details.commit;
                            format!(
                                "{}  ·  {}  ·  {}",
                                commit.author_name,
                                commit.relative_label(git_core::now_seconds()),
                                commit.short_hash
                            )
                        })
                    })
                    .unwrap_or_default(),
                Target::Change { staged, .. } => {
                    if staged {
                        "Staged change".to_owned()
                    } else {
                        "Unstaged change".to_owned()
                    }
                }
                Target::Stash(index) => format!("stash@{{{index}}}"),
                Target::None => "Select commit to view changes".to_owned(),
            })
            .text_size(px(11.5))
            .text_color(move || fg_muted()),
        )
        .child(commit_body(details))
        .child(commit_refs(details))
}

/// The commit's message body, under its subject.
///
/// `body` is the whole message, subject included; the subject is already
/// drawn above, so only the lines after it show here.
fn commit_body(details: Signal<Option<git_core::CommitDetails>>) -> impl IntoElement {
    when(
        move || !message_rest(details).is_empty(),
        move || {
            view().pt_1().child(
                text(move || message_rest(details))
                    .text_size(px(12.0))
                    .text_color(move || fg())
                    .w_full(),
            )
        },
    )
}

/// A commit's message after its subject line, trimmed.
fn message_rest(details: Signal<Option<git_core::CommitDetails>>) -> String {
    details
        .with(|details| {
            details.as_ref().map(|details| {
                details
                    .body
                    .split_once('\n')
                    .map(|(_, rest)| rest.trim().to_owned())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

/// The refs pointing at the selected commit.
fn commit_refs(details: Signal<Option<git_core::CommitDetails>>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .flex_wrap()
        .gap_1()
        .child(each(
            move || {
                details.with(|details| {
                    details
                        .as_ref()
                        .map(|details| details.commit.refs.clone())
                        .unwrap_or_default()
                })
            },
            |label: &String| label.clone(),
            move |label: ReadSignal<String>, _: ReadSignal<usize>| {
                let text = label.get();
                let tint = if text.starts_with("tag:") {
                    theme().color("tag").unwrap_or(theme().muted_foreground)
                } else if text.contains("->") {
                    theme().color("graph-2").unwrap_or(theme().primary)
                } else {
                    theme().primary
                };
                ref_badge(text, tint)
            },
        ))
}

/// One node of the changed-file tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    /// The segment this node adds to its parent's path.
    name: String,
    /// The full path, which is the key that decides whether it is open.
    path: String,
    /// The file's own change; `None` for a folder.
    kind: Option<ChangeKind>,
    /// The files under a folder, at any depth.
    count: usize,
    children: Vec<Node>,
}

impl Node {
    fn folder(name: &str, path: &str) -> Self {
        Self {
            name: name.to_owned(),
            path: path.to_owned(),
            kind: None,
            count: 0,
            children: Vec::new(),
        }
    }

    fn file(name: &str, path: &str, kind: ChangeKind) -> Self {
        Self {
            name: name.to_owned(),
            path: path.to_owned(),
            kind: Some(kind),
            count: 1,
            children: Vec::new(),
        }
    }

    /// Puts `file` under this node, making the folders its path needs.
    ///
    /// `prefix` is the path of the node this is called on, so a folder's own
    /// path is the prefix and its segment, never the whole file's: a folder
    /// and the file inside it would otherwise share one key.
    fn insert(&mut self, parts: &[&str], prefix: &str, kind: ChangeKind) {
        match parts {
            [] => {}
            [name] => {
                self.children
                    .push(Node::file(name, &join(prefix, name), kind));
                self.count += 1;
            }
            [name, rest @ ..] => {
                let path = join(prefix, name);
                let at = match self
                    .children
                    .iter()
                    .position(|child| child.name == *name && child.kind.is_none())
                {
                    Some(at) => at,
                    None => {
                        self.children.push(Node::folder(name, &path));
                        self.children.len() - 1
                    }
                };
                self.children[at].insert(rest, &path, kind);
                self.count += 1;
            }
        }
    }

    /// Folders before files, each by name, as the reference orders them.
    fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            let (a_folder, b_folder) = (a.kind.is_none(), b.kind.is_none());
            b_folder
                .cmp(&a_folder)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        for child in &mut self.children {
            child.sort();
        }
    }

    /// The node and its descendants, in the order they are drawn, skipping
    /// what a closed folder hides.
    fn flatten(&self, depth: u32, closed: &HashSet<String>, out: &mut Vec<(u32, Node)>) {
        out.push((depth, self.clone()));
        if self.kind.is_some() || closed.contains(&self.path) {
            return;
        }
        for child in &self.children {
            child.flatten(depth + 1, closed, out);
        }
    }
}

/// `prefix` and `name` as one path, without a leading slash at the root.
fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

/// The files the commit changed, as the reference nests them.
fn file_tree(store: Store) -> impl IntoElement {
    let details = store.details;
    let target = store.target;
    // Which folders the reader has closed. Empty means everything is open,
    // which is how the reference shows a commit.
    let closed = signal(HashSet::<String>::new());

    let root = computed(move || {
        details.with(|details| {
            let Some(details) = details.as_ref() else {
                return Node::folder("", "");
            };
            let mut root = Node::folder("", "");
            for file in &details.files {
                let parts = file.path.split('/').collect::<Vec<_>>();
                root.insert(&parts, "", file.kind);
            }
            root.sort();
            root
        })
    });

    let rows = computed(move || {
        let closed = closed.get();
        let mut out = Vec::new();
        root.with(|root| {
            if root.children.is_empty() {
                return;
            }
            // The root row is the "44 files" line: the whole list, counted.
            out.push((0u32, root.clone()));
            for child in &root.children {
                child.flatten(1, &closed, &mut out);
            }
        });
        out
    });

    view()
        .flex_1()
        .min_h(px(0.0))
        .flex_col()
        .w_full()
        .child(
            scroll()
                .max_h(px(280.0))
                .child(view().flex_col().w_full().child(each(
                    move || rows.with(|rows| rows.clone()),
                    |row: &(u32, Node)| row.1.path.clone(),
                    move |row: ReadSignal<(u32, Node)>, _: ReadSignal<usize>| {
                        let (depth, node) = row.get();
                        let folder = node.kind.is_none();
                        let path = node.path.clone();
                        let name = if node.name.is_empty() {
                            if node.count == 1 {
                                "1 file".to_owned()
                            } else {
                                format!("{} files", node.count)
                            }
                        } else {
                            node.name.clone()
                        };
                        let count = node.count;
                        let is_open = move || !closed.with(|closed| closed.contains(&path));

                        let line = view()
                            .h(px(ROW_H))
                            .w_full()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .pl(px(4.0 + 12.0 * depth as f32))
                            .pr_2()
                            .rounded(px(5.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(move || row_hover_bg()))
                            .on_press({
                                let path = node.path.clone();
                                let folder = folder;
                                move |_: &()| {
                                    if folder {
                                        closed.update(|closed| {
                                            if !closed.remove(&path) {
                                                closed.insert(path.clone());
                                            }
                                        });
                                    } else {
                                        // A file opens its own patch in a tab,
                                        // which is where the reference shows
                                        // a diff.
                                        let Target::Commit(hash) = target.get_untracked() else {
                                            return;
                                        };
                                        state::open_commit_file_tab(store, hash, path.clone());
                                    }
                                }
                            })
                            .child(
                                view()
                                    .w(px(12.0))
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
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
                                    ),
                            );

                        match node.kind {
                            // A file: its status letter, its icon, its name.
                            Some(kind) => line
                                .child(
                                    text(kind.letter())
                                        .w(px(12.0))
                                        .flex_none()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(move || change_color(kind)),
                                )
                                .child(file_icon(&node.name, 16.0))
                                .child(
                                    text(node.name.clone())
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .truncate()
                                        .text_size(px(12.5))
                                        .text_color(move || fg()),
                                )
                                .into_element(),
                            // A folder: its icon, its name and the number of
                            // files it holds.
                            None => line
                                .child(icon(Icon::Folder, 16.0).text_color(move || icon_color()))
                                .child(
                                    text(name)
                                        .flex_none()
                                        .max_w(px(300.0))
                                        .truncate()
                                        .text_size(px(12.5))
                                        .text_color(move || fg()),
                                )
                                .child(
                                    text(if node.name.is_empty() {
                                        String::new()
                                    } else if count == 1 {
                                        "1 file".to_owned()
                                    } else {
                                        format!("{count} files")
                                    })
                                    .flex_none()
                                    .text_size(px(11.0))
                                    .text_color(move || fg_muted()),
                                )
                                .into_element(),
                        }
                    },
                ))),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
}
