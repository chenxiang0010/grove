//! The right-click menus: what a branch, a commit, a change, a stash and a
//! file offer.
//!
//! A menu is the only place a destructive action lives, and every one of them
//! goes through `git`, so the result is what the command line would have done.
//! The labels are the reference IDE's own, taken from its Git bundle, so the
//! same action reads the same way in both.
//!
//! The values a menu acts on are closures rather than strings: a menu built
//! inside a list cell outlives the row that cell was showing, and a captured
//! string would name the previous row once the list recycles it.

use std::rc::Rc;

use nsh_base::{context_menu, input, menu_item, menu_separator, submenu, theme, tooltip};
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{
    fg, fg_muted, fg_strong, icon_color, project_avatar_color, row_selected_bg,
};
use crate::ui::icons::{Icon, icon};

/// One item of a Grove menu.
fn item(label: &'static str, press: impl Fn() + 'static) -> nsh_base::MenuItemElement {
    item_with(label, "", press)
}

/// One item, with the shortcut the reference prints at its right end.
fn item_with(
    label: &'static str,
    keys: &'static str,
    press: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    menu_item()
        .on_select(press)
        .flex_row()
        .items_center()
        .gap_4()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(label)
                .flex_1()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
        .child(
            text(keys)
                .flex_none()
                .text_size(px(11.0))
                .text_color(move || fg_muted()),
        )
}

/// An item that cannot be chosen.
fn disabled(label: &'static str) -> nsh_base::MenuItemElement {
    menu_item()
        .disabled(true)
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .child(
            text(label)
                .text_size(px(12.0))
                .text_color(move || fg_muted().opacity(0.6)),
        )
}

/// One item whose label names the ref it acts on: the text reads the names
/// as the row renders, for the list cell it was built from may recycle them.
fn item_named(
    label: impl Fn() -> String + 'static,
    press: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    menu_item()
        .on_select(press)
        .flex_row()
        .items_center()
        .gap_4()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(label)
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// A rule between two groups.
fn rule() -> nsh_base::MenuSeparatorElement {
    menu_separator()
        .h(px(1.0))
        .my_1()
        .bg(move || theme().border)
}

/// A menu whose trigger is `trigger` and whose items are `children`.
fn menu(trigger: impl IntoElement, children: Vec<AnyElement>) -> impl IntoElement {
    let mut built = context_menu()
        .trigger(trigger)
        .popup_style(crate::ui::popup_style);
    for child in children {
        built = built.child(child);
    }
    built
}

/// One item of a Grove menu, with the icon its row carries: the project
/// menu's Open and Clone Repository rows.
fn item_with_icon(
    label: &'static str,
    glyph: Icon,
    press: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    menu_item()
        .on_select(press)
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .child(icon(glyph, 15.0).text_color(move || icon_color()))
        .child(
            text(label)
                .flex_1()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// The popup the title bar's project widget opens: the ways to open or clone
/// a project, then every project the window remembers, each with its path
/// and the branch it has checked out.
pub fn project_popup(store: Store, trigger: impl IntoElement) -> impl IntoElement {
    let open = item_with_icon("Open…", Icon::Folder, move || state::pick_folder(store));
    let clone = item_with_icon("Clone Repository…", Icon::Branch, move || {
        state::open_clone_dialog(store)
    });
    let recents = store.recents;

    // A left-press dropdown, as the title bar's branch popup is: the
    // context menu answers a right click, and this widget answers a press.
    let mut built = nsh_base::menu()
        .trigger(trigger)
        .popup_style(crate::ui::popup_style);
    for child in [
        open.into_element(),
        clone.into_element(),
        rule().into_element(),
        popup_heading("Open Projects").into_element(),
        dynamic(move || {
            let paths = recents.get();
            if paths.is_empty() {
                view()
                    .px_2()
                    .py_1()
                    .child(
                        text("No recent projects")
                            .text_size(px(11.5))
                            .text_color(move || fg_muted()),
                    )
                    .into_element()
            } else {
                view()
                    .flex_col()
                    .w_full()
                    .children(paths.into_iter().map(|path| recent_row(store, path)))
                    .into_element()
            }
        })
        .into_element(),
    ] {
        built = built.child(child);
    }
    built
}

/// One project of the Open Projects list: its avatar, its name, its path and
/// its checked-out branch.
fn recent_row(store: Store, path: std::path::PathBuf) -> nsh_base::MenuItemElement {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let shown = path.display().to_string();
    let color = project_avatar_color(&name);
    let initials = crate::theme_tokens::project_initials(&name);
    let branches = store.recent_branches;
    let branch_path = shown.clone();

    menu_item()
        .on_select(move || state::open_folder(store, path.clone()))
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .child(
            view()
                .size_5()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg(move || color)
                .child(
                    text(initials.clone())
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(move || theme().primary_foreground),
                ),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap_0p5()
                .child(
                    text(name)
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || fg_strong()),
                )
                .child(
                    text(shown.clone())
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(move || fg_muted()),
                )
                .child(dynamic(move || {
                    let branch = branches.with(|map| map.get(&branch_path).cloned());
                    match branch {
                        Some(branch) => view()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .pl(px(12.0))
                            .child(icon(Icon::Branch, 10.0).text_color(move || fg_muted()))
                            .child(
                                text(branch)
                                    .truncate()
                                    .text_size(px(10.5))
                                    .text_color(move || fg_muted()),
                            )
                            .into_element(),
                        None => view().h(px(0.0)).into_element(),
                    }
                })),
        )
}

/// A heading over a group of the branch popup.
fn popup_heading(label: &'static str) -> impl IntoElement {
    view().px_2().py_1().child(
        text(label)
            .text_size(px(10.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || fg_muted()),
    )
}

// ---------------------------------------------------------------------------
// A branch
// ---------------------------------------------------------------------------

/// What a branch row offers: check it out, start from it, merge, rebase,
/// rename, delete, and copy its name.
pub fn branch_menu(
    store: Store,
    trigger: impl IntoElement,
    short: impl Fn() -> String + 'static,
    is_head: bool,
    is_tag: bool,
) -> impl IntoElement {
    let short: Rc<dyn Fn() -> String> = Rc::new(short);
    menu(trigger, branch_items(store, short, is_head, is_tag))
}

/// The items of a branch's menu, in the reference's order and with its
/// wording: the ref's own name quoted in the labels that act on it.
fn branch_items(
    store: Store,
    short: Rc<dyn Fn() -> String>,
    is_head: bool,
    is_tag: bool,
) -> Vec<AnyElement> {
    // Every branch action closes the branch popup as it runs, as the
    // reference's does; from the Git pane's menus the signal is already
    // down, and setting it again costs nothing.
    let close_popup = move || store.branch_popup_open.set(false);
    // The branch the working tree is on: the one the merge and rebase names
    // as their target.
    let current = move || {
        store
            .snapshot
            .with(|snapshot| snapshot.branch.branch.clone())
            .unwrap_or_default()
    };
    let mut items: Vec<AnyElement> = Vec::new();

    if is_tag {
        items.push(
            item("Checkout", {
                let short = short.clone();
                move || {
                    close_popup();
                    state::checkout(store, short());
                }
            })
            .into_element(),
        );
        items.push(
            item_named(
                {
                    let name = short.clone();
                    move || format!("Merge '{}' into '{}'", name(), current())
                },
                {
                    let name = short.clone();
                    move || {
                        close_popup();
                        state::merge(store, name());
                    }
                },
            )
            .into_element(),
        );
        items.push(
            item("Push to origin", {
                let name = short.clone();
                move || {
                    close_popup();
                    state::push_tag(store, name());
                }
            })
            .into_element(),
        );
        items.push(
            item("Delete", {
                let name = short.clone();
                move || {
                    close_popup();
                    state::delete_tag(store, name());
                }
            })
            .into_element(),
        );
        items.push(rule().into_element());
        items.push(
            item("Copy Tag Name", {
                let name = short.clone();
                move || {
                    close_popup();
                    state::copy(store, name(), "the tag");
                }
            })
            .into_element(),
        );
        items.push(
            item("Show This Tag in the Log", {
                let name = short.clone();
                move || {
                    close_popup();
                    store.reference.set(Some(format!("refs/tags/{}", name())));
                    state::refresh(store);
                }
            })
            .into_element(),
        );
        return items;
    }

    if is_head {
        // The checked-out branch: the reference's menu starts at what grows
        // from it, and offers no checkout and no delete.
        items.push(
            item_named(
                {
                    let name = short.clone();
                    move || format!("New Branch from '{}'...", name())
                },
                {
                    let name = short.clone();
                    move || {
                        close_popup();
                        state::prompt_new_branch(store, name());
                    }
                },
            )
            .into_element(),
        );
        items.push(
            item("Update", {
                move || {
                    close_popup();
                    state::open_update_dialog(store);
                }
            })
            .into_element(),
        );
        items.push(
            item("Push...", {
                move || {
                    close_popup();
                    state::open_push(store);
                }
            })
            .into_element(),
        );
    } else {
        items.push(
            item("Checkout", {
                let short = short.clone();
                move || {
                    close_popup();
                    state::checkout(store, short());
                }
            })
            .into_element(),
        );
        items.push(
            item_named(
                {
                    let name = short.clone();
                    move || format!("New Branch from '{}'...", name())
                },
                {
                    let name = short.clone();
                    move || {
                        close_popup();
                        state::prompt_new_branch(store, name());
                    }
                },
            )
            .into_element(),
        );
        items.push(
            item_named(
                {
                    let name = short.clone();
                    move || format!("Rebase '{}' onto '{}'", current(), name())
                },
                {
                    let name = short.clone();
                    move || {
                        close_popup();
                        state::rebase(store, name());
                    }
                },
            )
            .into_element(),
        );
        items.push(
            item_named(
                {
                    let name = short.clone();
                    move || format!("Merge '{}' into '{}'", name(), current())
                },
                {
                    let name = short.clone();
                    move || {
                        close_popup();
                        state::merge(store, name());
                    }
                },
            )
            .into_element(),
        );
        if short().contains('/') {
            // A remote branch: pulling it is the reference's next item.
            items.push(
                item_named(move || format!("Pull into '{}' Using Merge", current()), {
                    move || {
                        close_popup();
                        state::pull_merge(store);
                    }
                })
                .into_element(),
            );
        }
    }

    items.push(rule().into_element());
    items.push(
        item("Rename...", {
            let short = short.clone();
            move || {
                close_popup();
                state::prompt_rename_branch(store, short());
            }
        })
        .into_element(),
    );
    if is_head {
        items.push(disabled("Delete").into_element());
    } else if short().contains('/') {
        // A remote branch: the reference greys its delete out, and Grove
        // deletes a local branch only.
        items.push(disabled("Delete").into_element());
    } else {
        items.push(
            item("Delete", {
                let short = short.clone();
                move || {
                    close_popup();
                    state::delete_branch(store, short());
                }
            })
            .into_element(),
        );
    }
    items.push(rule().into_element());
    items.push(
        item("Copy Branch Name", {
            let short = short.clone();
            move || {
                close_popup();
                state::copy(store, short(), "the branch");
            }
        })
        .into_element(),
    );
    items.push(
        item("Show This Branch in the Log", {
            let short = short.clone();
            move || {
                close_popup();
                store.reference.set(Some(format!("refs/heads/{}", short())));
                state::refresh(store);
            }
        })
        .into_element(),
    );

    items
}

// ---------------------------------------------------------------------------
// A commit
// ---------------------------------------------------------------------------
/// What a commit row offers, in the reference's order and with its labels.
pub fn commit_menu(
    store: Store,
    trigger: impl IntoElement,
    hash: impl Fn() -> String + 'static,
    short: impl Fn() -> String + 'static,
    subject: impl Fn() -> String + 'static,
) -> impl IntoElement {
    let hash: Rc<dyn Fn() -> String> = Rc::new(hash);
    let short: Rc<dyn Fn() -> String> = Rc::new(short);
    let subject: Rc<dyn Fn() -> String> = Rc::new(subject);

    let mut items: Vec<AnyElement> = Vec::new();

    let for_checkout = hash.clone();
    items.push(
        item("Checkout Revision", move || {
            state::checkout_detached(store, for_checkout())
        })
        .into_element(),
    );
    let for_new_branch = hash.clone();
    items.push(
        item("New Branch from Here…", move || {
            state::prompt_new_branch(store, for_new_branch())
        })
        .into_element(),
    );
    let for_new_tag = hash.clone();
    items.push(
        item("New Tag…", move || {
            state::prompt_new_tag(store, for_new_tag())
        })
        .into_element(),
    );
    items.push(rule().into_element());

    let for_cherry = hash.clone();
    items.push(
        item("Cherry-Pick", move || {
            state::cherry_pick(store, for_cherry())
        })
        .into_element(),
    );
    let for_revert = hash.clone();
    items.push(
        item("Revert Commit", move || {
            state::revert_commit(store, for_revert())
        })
        .into_element(),
    );

    // The reset modes are one submenu, as the reference nests them.
    let for_reset = hash.clone();
    items.push(
        submenu()
            .trigger(
                view()
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .hover(|s| s.bg(move || theme().accent))
                    .child(
                        text("Reset Current Branch to Here…")
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(move || fg()),
                    )
                    .child(text("›").text_size(px(11.0)).text_color(move || fg_muted())),
            )
            .child(item("Soft", {
                let target = for_reset.clone();
                move || state::reset_to(store, target(), "soft")
            }))
            .child(item("Mixed", {
                let target = for_reset.clone();
                move || state::reset_to(store, target(), "mixed")
            }))
            .child(item("Hard", {
                let target = for_reset.clone();
                move || state::reset_to(store, target(), "hard")
            }))
            .into_element(),
    );

    items.push(rule().into_element());

    let for_reword = hash.clone();
    items.push(
        item("Edit Commit Message…", move || {
            // The reference rewrites the message of the commit under the
            // cursor; Grove asks for the text and amends the current commit
            // when the selection is `HEAD`.
            let head = store
                .snapshot
                .with(|snapshot| snapshot.branch.head_commit.clone())
                .unwrap_or_default();
            if head == for_reword() {
                store.amend.set(true);
                let message = store.message;
                let shown = store.snapshot.with(|_| String::new());
                let _ = shown;
                let _ = message;
                store.side.set(state::Side::Commit);
                store.say("Amend the message in the commit box");
            } else {
                store.say("Only the current commit can be reworded here");
            }
        })
        .into_element(),
    );

    let for_squash = hash.clone();
    items.push(
        item("Squash Commits…", move || {
            let _ = for_squash;
            store.say("Select the commits to squash in the log");
        })
        .into_element(),
    );

    items.push(rule().into_element());

    let for_copy_hash = hash.clone();
    items.push(
        item_with("Copy Hash", "Ctrl+C", move || {
            state::copy(store, for_copy_hash(), "the hash")
        })
        .into_element(),
    );
    let for_copy_short = short.clone();
    items.push(
        item("Copy Short Hash", move || {
            state::copy(store, for_copy_short(), "the short hash")
        })
        .into_element(),
    );
    let for_copy_subject = subject.clone();
    items.push(
        item("Copy Commit Message", move || {
            state::copy(store, for_copy_subject(), "the message")
        })
        .into_element(),
    );

    let for_filter = short.clone();
    items.push(
        item("Filter to This Commit", move || {
            store.filter.update(|filter| filter.text = for_filter());
        })
        .into_element(),
    );
    let for_select = hash.clone();
    items.push(
        item("Show Repository at Revision", move || {
            let target = for_select();
            state::select_commit(store, target);
        })
        .into_element(),
    );

    menu(trigger, items)
}

// ---------------------------------------------------------------------------
// A change
// ---------------------------------------------------------------------------

/// What a change row offers: stage or unstage, discard, ignore, delete, and
/// the ways to look at the file.
pub fn change_menu(
    store: Store,
    trigger: impl IntoElement,
    path: impl Fn() -> String + 'static,
    staged: impl Fn() -> bool + 'static,
) -> impl IntoElement {
    let path: Rc<dyn Fn() -> String> = Rc::new(path);
    let staged: Rc<dyn Fn() -> bool> = Rc::new(staged);
    let mut items: Vec<AnyElement> = Vec::new();

    if staged() {
        let for_unstage = path.clone();
        items.push(
            item("Unstage", move || {
                state::unstage(store, vec![for_unstage()])
            })
            .into_element(),
        );
    } else {
        let for_stage = path.clone();
        items.push(
            item("Add to Index", move || {
                state::stage(store, vec![for_stage()])
            })
            .into_element(),
        );
    }

    let for_open = path.clone();
    items.push(
        item("Show Diff", move || {
            let staged = staged();
            state::open_change_tab(store, for_open(), staged);
        })
        .into_element(),
    );
    let for_edit = path.clone();
    items.push(item("Edit File", move || state::open_file_tab(store, for_edit())).into_element());

    items.push(rule().into_element());

    let for_discard = path.clone();
    items.push(
        item("Rollback…", move || {
            state::discard(store, vec![for_discard()])
        })
        .into_element(),
    );
    let for_ignore = path.clone();
    items.push(
        item("Add to .gitignore", move || {
            state::ignore(store, vec![for_ignore()])
        })
        .into_element(),
    );
    let for_delete = path.clone();
    items.push(
        item("Delete…", move || {
            state::delete(store, vec![for_delete()])
        })
        .into_element(),
    );

    items.push(rule().into_element());

    let for_copy = path.clone();
    items.push(
        item("Copy Path", move || {
            state::copy(store, for_copy(), "the path")
        })
        .into_element(),
    );
    let for_reveal = path.clone();
    items.push(
        item("Reveal in File Manager", move || {
            if let Some(root) = store.paths().map(|paths| paths.root) {
                let _ = reveal_path(root.join(for_reveal()));
            }
        })
        .into_element(),
    );
    let for_history = path.clone();
    items.push(
        item("Show History", move || {
            store.filter.update(|filter| filter.path = for_history());
        })
        .into_element(),
    );

    menu(trigger, items)
}

// ---------------------------------------------------------------------------
// A stash
// ---------------------------------------------------------------------------

/// What a stash row offers: pop it, look at it, drop it.
pub fn stash_menu(
    store: Store,
    trigger: impl IntoElement,
    index: impl Fn() -> u32 + 'static,
) -> impl IntoElement {
    let index: Rc<dyn Fn() -> u32> = Rc::new(index);
    let mut items: Vec<AnyElement> = Vec::new();

    let for_pop = index.clone();
    items.push(item("Pop Stash", move || state::stash_pop(store, for_pop())).into_element());
    let for_show = index.clone();
    items.push(
        item("Preview Diff", move || {
            let index = for_show();
            state::select_stash(store, index);
            state::open_stash_tab(store, index);
        })
        .into_element(),
    );
    items.push(rule().into_element());
    let for_drop = index.clone();
    items.push(
        item("Drop Stash…", move || {
            state::stash_drop(store, for_drop())
        })
        .into_element(),
    );

    menu(trigger, items)
}

// ---------------------------------------------------------------------------
// A file in the Project tree
// ---------------------------------------------------------------------------

/// What a file in the Project tree offers, in the reference's order.
///
/// The groups read: the ways to open it and to make things beside it; the
/// file clipboard; renaming and deleting; the places it can open; its
/// history and blame; the ways to compare it; and finally where it sits on
/// disk.
pub fn project_file_menu(
    store: Store,
    trigger: impl IntoElement,
    path: impl Fn() -> String + 'static,
    changed: impl Fn() -> bool + 'static,
) -> impl IntoElement {
    let path: Rc<dyn Fn() -> String> = Rc::new(path);
    let changed: Rc<dyn Fn() -> bool> = Rc::new(changed);
    let folder_of = {
        let path = path.clone();
        Rc::new(move || match path().rfind('/') {
            Some(at) => path()[..at].to_owned(),
            None => String::new(),
        }) as Rc<dyn Fn() -> String>
    };
    let mut items: Vec<AnyElement> = Vec::new();

    // Open, and make something beside it.
    let for_open = path.clone();
    items.push(
        item_with("Open", "", move || state::open_file_tab(store, for_open())).into_element(),
    );
    let for_new_file = folder_of.clone();
    items.push(
        item("New File…", move || {
            state::prompt_new_file(store, for_new_file())
        })
        .into_element(),
    );
    let for_new_folder = folder_of.clone();
    items.push(
        item("New Folder…", move || {
            state::prompt_new_folder(store, for_new_folder())
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // The file clipboard: what a Cut, a Copy and a Paste act on.
    let for_cut = path.clone();
    items.push(
        item_with("Cut", "Ctrl+X", move || {
            state::cut_path(store, for_cut(), true)
        })
        .into_element(),
    );
    let for_copy = path.clone();
    items.push(
        item_with("Copy", "Ctrl+C", move || {
            state::cut_path(store, for_copy(), false)
        })
        .into_element(),
    );
    let for_copy_path = path.clone();
    items.push(
        item("Copy Path/Reference…", move || {
            state::copy(store, for_copy_path(), "the path")
        })
        .into_element(),
    );
    let for_paste = folder_of.clone();
    let can_paste = move || store.file_clip.get().is_some();
    if can_paste() {
        items.push(
            item_with("Paste", "Ctrl+V", move || {
                state::paste_into(store, for_paste())
            })
            .into_element(),
        );
    } else {
        items.push(disabled("Paste").into_element());
    }

    items.push(rule().into_element());

    // Rename and delete.
    let for_rename = path.clone();
    items.push(
        item_with("Rename…", "Shift+F6", move || {
            state::prompt_rename_file(store, for_rename())
        })
        .into_element(),
    );
    let for_delete = path.clone();
    items.push(
        item_with("Delete…", "Delete", move || {
            state::delete(store, vec![for_delete()])
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // The places the file opens.
    let for_reveal = path.clone();
    items.push(
        item("Open In", move || {
            if let Some(root) = store.paths().map(|paths| paths.root) {
                let _ = reveal_path(root.join(for_reveal()));
            }
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // Its history: the commits that touched it, and who wrote each line.
    let for_history = path.clone();
    items.push(
        item("Show History", move || {
            state::show_history(store, for_history());
        })
        .into_element(),
    );
    let for_blame = path.clone();
    items.push(
        item("Annotate with Git Blame", move || {
            state::open_blame_tab(store, for_blame());
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // The ways to see what changed in it.
    let for_diff = path.clone();
    items.push(
        item_with("Show Diff", "Ctrl+D", move || {
            state::open_change_tab(store, for_diff(), false)
        })
        .into_element(),
    );
    let for_staged_diff = path.clone();
    items.push(
        item("Show Staged Diff", move || {
            state::open_change_tab(store, for_staged_diff(), true)
        })
        .into_element(),
    );
    // The file as `HEAD` has it, which is what a reader checks before
    // deciding a change is what they meant to make.
    let for_revision = path.clone();
    items.push(
        item("Show Current Revision", move || {
            state::open_compare_tab(
                store,
                for_revision(),
                "HEAD".to_owned(),
                "WORKTREE".to_owned(),
            );
        })
        .into_element(),
    );
    let for_compare_rev = path.clone();
    items.push(
        item("Compare with Revision…", move || {
            state::prompt_compare(store, for_compare_rev());
        })
        .into_element(),
    );
    let for_compare_branch = path.clone();
    items.push(
        item("Compare with Branch or Tag…", move || {
            state::prompt_compare(store, for_compare_branch());
        })
        .into_element(),
    );
    // Two files, neither of which has to be in this repository: the
    // reference's way of reading one file of one checkout against the same
    // file of another.
    let for_compare_file = path.clone();
    items.push(
        item_with("Compare with File…", "Ctrl+D", move || {
            let picked = store.picked.get_untracked();
            if picked.len() == 2 {
                state::compare_picked(store);
            } else {
                state::compare_file(store, for_compare_file());
            }
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // The Git actions that apply to a path.
    let for_stage = path.clone();
    items.push(
        item("Add to Index", move || {
            state::stage(store, vec![for_stage()])
        })
        .into_element(),
    );
    let for_unstage = path.clone();
    items.push(
        item("Unstage", move || {
            state::unstage(store, vec![for_unstage()])
        })
        .into_element(),
    );
    let for_rollback = path.clone();
    let has_change = changed();
    if has_change {
        items.push(
            item("Rollback…", move || {
                state::discard(store, vec![for_rollback()])
            })
            .into_element(),
        );
    }
    let for_ignore = path.clone();
    items.push(
        item("Add to .gitignore", move || {
            state::ignore(store, vec![for_ignore()])
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // Where it sits on disk.
    let for_reload = path.clone();
    items.push(
        item("Reload from Disk", move || {
            let key = format!("file:{}", for_reload());
            state::reload_tab(store, key, for_reload());
        })
        .into_element(),
    );
    let for_reveal_file = path.clone();
    items.push(
        item("Reveal in File Manager", move || {
            if let Some(root) = store.paths().map(|paths| paths.root) {
                let _ = reveal_path(root.join(for_reveal_file()));
            }
        })
        .into_element(),
    );

    menu(trigger, items)
}

/// The right-click menu of a folder of the Project tree.
///
/// The reference offers the folder's own actions — a file or a folder made
/// inside it, the tree's clipboard, its history — and, what the reader comes
/// to a folder menu for, the two comparisons: this folder against another on
/// disk, which is how two checkouts of one project on different branches are
/// read side by side.
pub fn project_folder_menu(
    store: Store,
    trigger: impl IntoElement,
    path: impl Fn() -> String + 'static,
    changed: impl Fn() -> bool + 'static,
) -> impl IntoElement {
    let path: Rc<dyn Fn() -> String> = Rc::new(path);
    let changed: Rc<dyn Fn() -> bool> = Rc::new(changed);
    let mut items: Vec<AnyElement> = Vec::new();

    // Make something inside it, and take what is on the clipboard.
    let for_new_file = path.clone();
    items.push(
        item("New File…", move || {
            state::prompt_new_file(store, for_new_file())
        })
        .into_element(),
    );
    let for_new_folder = path.clone();
    items.push(
        item("New Folder…", move || {
            state::prompt_new_folder(store, for_new_folder())
        })
        .into_element(),
    );

    items.push(rule().into_element());

    let for_cut = path.clone();
    items.push(
        item_with("Cut", "Ctrl+X", move || {
            state::cut_path(store, for_cut(), true)
        })
        .into_element(),
    );
    let for_copy = path.clone();
    items.push(
        item_with("Copy", "Ctrl+C", move || {
            state::cut_path(store, for_copy(), false)
        })
        .into_element(),
    );
    let for_copy_path = path.clone();
    items.push(
        item("Copy Path/Reference…", move || {
            state::copy(store, for_copy_path(), "the path")
        })
        .into_element(),
    );
    let for_paste = path.clone();
    let can_paste = move || store.file_clip.get().is_some();
    if can_paste() {
        items.push(
            item_with("Paste", "Ctrl+V", move || {
                state::paste_into(store, for_paste())
            })
            .into_element(),
        );
    } else {
        items.push(disabled("Paste").into_element());
    }

    items.push(rule().into_element());

    let for_rename = path.clone();
    items.push(
        item_with("Rename…", "Shift+F6", move || {
            state::prompt_rename_file(store, for_rename())
        })
        .into_element(),
    );
    let for_delete = path.clone();
    items.push(
        item_with("Delete…", "Delete", move || {
            state::delete(store, vec![for_delete()])
        })
        .into_element(),
    );

    items.push(rule().into_element());

    let for_reveal = path.clone();
    items.push(
        item("Open In", move || {
            if let Some(root) = store.paths().map(|paths| paths.root) {
                let _ = reveal_path(root.join(for_reveal()));
            }
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // The folder's history: every commit that touched anything under it.
    let for_history = path.clone();
    items.push(
        item("Show History", move || {
            state::show_history(store, for_history());
        })
        .into_element(),
    );

    items.push(rule().into_element());

    // Comparing is what the reader picked two rows for: with two picked
    // folders this lays them side by side, and with two picked files it shows
    // their diff. With one row picked it asks for the other folder on disk,
    // which is the only case the reference needs a dialog in.
    let for_compare = path.clone();
    items.push(
        item_with("Compare Directories", "Ctrl+D", move || {
            let picked = store.picked.get_untracked();
            if picked.len() == 2 {
                state::compare_picked(store);
            } else {
                state::compare_folder(store, for_compare());
            }
        })
        .into_element(),
    );
    let for_commit = path.clone();
    items.push(
        item("Commit Directory…", move || {
            store.side.set(state::Side::Commit);
            state::include_under(store, for_commit());
        })
        .into_element(),
    );

    items.push(rule().into_element());

    let for_stage = path.clone();
    items.push(
        item("Add to Index", move || {
            state::stage(store, vec![for_stage()])
        })
        .into_element(),
    );
    let for_unstage = path.clone();
    items.push(
        item("Unstage", move || {
            state::unstage(store, vec![for_unstage()])
        })
        .into_element(),
    );
    let for_rollback = path.clone();
    if changed() {
        items.push(
            item("Rollback…", move || {
                state::discard(store, vec![for_rollback()])
            })
            .into_element(),
        );
    }
    let for_ignore = path.clone();
    items.push(
        item("Add to .gitignore", move || {
            state::ignore(store, vec![for_ignore()])
        })
        .into_element(),
    );

    menu(trigger, items)
}

/// The popup the title bar's branch widget opens, as the reference's does:
/// a search field with the fetch and settings buttons over it, the
/// top-level actions under the field, and the branches, their remotes and
/// the tags as a tree, each row carrying its own actions on a right click.
pub fn branch_popup(store: Store, trigger: impl IntoElement) -> impl IntoElement {
    // The app owns whether the popup is up: the actions and the rows close
    // it as they run, and the trigger toggles it.
    let popup_open = store.branch_popup_open;
    let mut built = nsh_base::menu()
        .trigger(trigger)
        .popup_style(|s| crate::ui::popup_style(s).w(px(400.0)))
        .open(move || Some(popup_open.get()))
        .on_open_change(move |event: &nsh_base::MenuEvent| {
            if let nsh_base::MenuEvent::OpenChange(open, _) = event {
                popup_open.set(*open);
            }
        });
    built = built
        .child(branch_popup_header(store).into_element())
        .child(branch_popup_actions(store).into_element())
        .child(branch_popup_refs(store).into_element());
    built
}

/// The popup's head: the search field, then the fetch and the settings
/// buttons at its right end, as the reference's toolbar holds them.
fn branch_popup_header(store: Store) -> nsh_core::element::View {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .px_1()
        .pt_1()
        .pb_1p5()
        .w_full()
        .child(
            view()
                .h(px(26.0))
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_2()
                .rounded(px(5.0))
                .bg(move || theme().input)
                .child(icon(Icon::Search, 13.0).text_color(move || fg_muted()))
                .child(
                    input()
                        .placeholder("Search for branches and actions")
                        .value(move || store.branch_popup_query.get())
                        .on_change(move |text: &str| store.branch_popup_query.set(text.to_owned()))
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .text_color(move || fg()),
                ),
        )
        .child(icon_button(Icon::Fetch, "Fetch", move || {
            state::fetch(store)
        }))
        .child(branch_popup_settings(store))
}

/// A small icon button of the popup's head: the fetch and the settings
/// gear, each saying what it does when the pointer rests on it.
fn icon_button(
    glyph: Icon,
    label: &'static str,
    press: impl Fn() + 'static,
) -> nsh_core::element::View {
    view()
        .size(px(26.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(press)
        .child(icon(glyph, 14.0).text_color(move || icon_color()))
        .tooltip(tooltip().child(crate::ui::frame::hover_bubble(label)))
}

/// The settings gear: the popup's switches. The reference keeps several;
/// Grove honours the one it can — whether the tags are listed at all.
fn branch_popup_settings(store: Store) -> nsh_base::SubmenuElement {
    let show_tags = store.branch_popup_show_tags;
    submenu()
        .trigger(icon_button(Icon::Settings, "Settings", move || {}))
        .child(check_item(
            "Show Tags",
            move || show_tags.get(),
            move || show_tags.set(!show_tags.get()),
        ))
}

/// One switch of the settings submenu: a checkable item.
fn check_item(
    label: &'static str,
    checked: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    menu_item()
        .checked(checked)
        .on_select(press)
        .flex_row()
        .items_center()
        .gap_4()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .child(
            text(label)
                .flex_1()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// The top-level actions under the search field, in the reference's order
/// and wording. Grove has no worktree support, so its "New Worktree..." is
/// left out; the first action wears the default look, as the reference's
/// highlighted "Commit..." does. The search narrows them like the tree.
fn branch_popup_actions(store: Store) -> nsh_core::element::View {
    view().w_full().px_1().py_1().child(dynamic(move || {
        let needle = store.branch_popup_query.get().to_lowercase();
        let mut rows: Vec<AnyElement> = Vec::new();
        if "commit...".contains(&needle) {
            rows.push(
                action_row("Commit...", true, move || {
                    store.branch_popup_open.set(false);
                    store.side.set(state::Side::Commit);
                })
                .into_element(),
            );
        }
        if "new branch...".contains(&needle) {
            rows.push(
                action_row("New Branch...", false, move || {
                    let start = store
                        .snapshot
                        .with(|snapshot| snapshot.branch.head_commit.clone())
                        .unwrap_or_else(|| "HEAD".to_owned());
                    state::prompt_new_branch(store, start);
                })
                .into_element(),
            );
        }
        if "checkout tag or revision...".contains(&needle) {
            rows.push(
                action_row("Checkout Tag or Revision...", false, move || {
                    let accept: Rc<dyn Fn(String)> =
                        Rc::new(move |revision: String| state::checkout(store, revision));
                    state::ask(
                        store,
                        state::Prompt {
                            title: "Checkout",
                            hint: "A branch, a tag or a commit hash.",
                            value: signal(String::new()),
                            confirm: "Checkout",
                            accept,
                        },
                    );
                })
                .into_element(),
            );
        }
        view().flex_col().w_full().children(rows).into_element()
    }))
}

/// One top-level action of the popup.
fn action_row(
    label: &'static str,
    default: bool,
    press: impl Fn() + 'static,
) -> nsh_base::MenuItemElement {
    menu_item()
        .on_select(press)
        .flex_row()
        .items_center()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .style_when(default, |s| s.bg(row_selected_bg()))
        .child(
            text(label)
                .flex_1()
                .text_size(px(12.0))
                .text_color(move || theme().primary_foreground),
        )
}

/// The tree: the local branches, the remotes with theirs, and the tags,
/// each group folding on a press and staying folded the way it was, the
/// search narrowing what shows and unfolding everything it matches.
fn branch_popup_refs(store: Store) -> nsh_core::element::View {
    view().w_full().px_1().pb_1().child(
        scroll()
            .w_full()
            .max_h(px(460.0))
            .child(dynamic(move || branch_rows(store))),
    )
}

/// What one row of the tree shows: the name it displays, the ref it acts
/// on, its marker and the extras around the name.
struct RefRow {
    shown: String,
    full: String,
    glyph: Icon,
    is_head: bool,
    is_tag: bool,
    /// The sync counts the checked-out branch wears.
    ahead: u32,
    behind: u32,
    /// The tracked branch's name at the row's right end, for a local branch.
    note: Option<String>,
    /// Whether the row carries the star: the branch the current one tracks.
    starred: bool,
    indent: f32,
}

/// The rows of the tree, built from the repository's refs and the search.
fn branch_rows(store: Store) -> AnyElement {
    let snapshot = store.snapshot;
    let entries = snapshot.with(|snapshot| snapshot.refs.clone());
    let branch = snapshot.with(|snapshot| snapshot.branch.clone());
    let needle = store.branch_popup_query.get().to_lowercase();
    let collapsed = store.branch_popup_collapsed.get();
    let show_tags = store.branch_popup_show_tags.get();
    let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
    // Searching unfolds everything: the folds give way to showing every
    // match at once.
    let folded = |key: &str| needle.is_empty() && collapsed.contains(key);

    let mut rows: Vec<AnyElement> = Vec::new();

    // Local: `HEAD` first, then the rest alphabetically.
    let mut local: Vec<&git_core::RefEntry> = entries
        .iter()
        .filter(|entry| entry.kind == git_core::RefKind::LocalBranch && matches(&entry.short_name))
        .collect();
    local.sort_by(|a, b| match (a.is_head, b.is_head) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.short_name.cmp(&b.short_name),
    });
    if !local.is_empty() {
        rows.push(group_row(store, "Local".to_owned(), "Local", !folded("Local")).into_element());
        if !folded("Local") {
            for entry in &local {
                rows.push(
                    ref_row(
                        store,
                        RefRow {
                            shown: entry.short_name.clone(),
                            full: entry.short_name.clone(),
                            glyph: if entry.is_head {
                                Icon::CurrentBranch
                            } else {
                                Icon::Branch
                            },
                            is_head: entry.is_head,
                            is_tag: false,
                            ahead: branch.ahead,
                            behind: branch.behind,
                            note: entry.upstream.clone(),
                            starred: false,
                            indent: 24.0,
                        },
                    )
                    .into_element(),
                );
            }
        }
    }

    // Remote: one folder per remote, its branches under it.
    let remotes: Vec<&git_core::RefEntry> = entries
        .iter()
        .filter(|entry| entry.kind == git_core::RefKind::RemoteBranch && matches(&entry.short_name))
        .collect();
    if !remotes.is_empty() {
        rows.push(
            group_row(store, "Remote".to_owned(), "Remote", !folded("Remote")).into_element(),
        );
        if !folded("Remote") {
            let mut names: Vec<String> = remotes
                .iter()
                .map(|entry| {
                    entry
                        .short_name
                        .split('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect();
            names.sort();
            names.dedup();
            for remote in names {
                let key = format!("remote/{}", remote);
                let open = !folded(&key);
                rows.push(group_row(store, key.clone(), &remote, open).into_element());
                if open {
                    let prefix = format!("{}/", remote);
                    for entry in remotes
                        .iter()
                        .filter(|entry| entry.short_name.starts_with(&prefix))
                    {
                        // The name under its folder drops the remote's
                        // prefix, as the reference's tree shows `main`
                        // under `origin`.
                        let shown = entry
                            .short_name
                            .strip_prefix(&prefix)
                            .unwrap_or(&entry.short_name)
                            .to_owned();
                        rows.push(
                            ref_row(
                                store,
                                RefRow {
                                    shown,
                                    full: entry.short_name.clone(),
                                    glyph: Icon::Branch,
                                    is_head: false,
                                    is_tag: false,
                                    ahead: 0,
                                    behind: 0,
                                    note: None,
                                    starred: branch.upstream.as_deref()
                                        == Some(entry.short_name.as_str()),
                                    indent: 40.0,
                                },
                            )
                            .into_element(),
                        );
                    }
                }
            }
        }
    }

    // Tags, unless the settings turn them off.
    let tags: Vec<&git_core::RefEntry> = entries
        .iter()
        .filter(|entry| entry.kind == git_core::RefKind::Tag && matches(&entry.short_name))
        .collect();
    if show_tags && !tags.is_empty() {
        rows.push(group_row(store, "Tags".to_owned(), "Tags", !folded("Tags")).into_element());
        if !folded("Tags") {
            for entry in &tags {
                rows.push(
                    ref_row(
                        store,
                        RefRow {
                            shown: entry.short_name.clone(),
                            full: entry.short_name.clone(),
                            glyph: Icon::Tag,
                            is_head: false,
                            is_tag: true,
                            ahead: 0,
                            behind: 0,
                            note: None,
                            starred: false,
                            indent: 24.0,
                        },
                    )
                    .into_element(),
                );
            }
        }
    }

    if rows.is_empty() {
        return view()
            .px_2()
            .py_2()
            .child(
                text("Nothing found")
                    .text_size(px(12.0))
                    .text_color(move || fg_muted()),
            )
            .into_element();
    }
    view().flex_col().w_full().children(rows).into_element()
}

/// One group of the tree: its chevron and its name. A press folds or
/// unfolds it, and the fold survives the popup's openings.
fn group_row(store: Store, key: String, label: &str, open: bool) -> nsh_core::element::View {
    let label = label.to_owned();
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move || {
            let collapsed = store.branch_popup_collapsed;
            let mut set = collapsed.get();
            if set.remove(&key) {
                collapsed.set(set);
            } else {
                set.insert(key.clone());
                collapsed.set(set);
            }
        })
        .child(
            icon(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                11.0,
            )
            .text_color(move || fg_muted()),
        )
        .child(
            text(label)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || fg_strong()),
        )
}

/// One row of the tree. A left press checks the branch out — a tag's row
/// leaves the checkout to its menu — and a right click opens the actions.
fn ref_row(store: Store, row: RefRow) -> AnyElement {
    let RefRow {
        shown,
        full,
        glyph,
        is_head,
        is_tag,
        ahead,
        behind,
        note,
        starred,
        indent,
    } = row;
    let press_name = full.clone();
    let menu_name = full.clone();
    let built = view()
        .flex_row()
        .items_center()
        .gap_2()
        .pl(px(indent))
        .pr_2()
        .py_1()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move || {
            if !is_tag {
                store.branch_popup_open.set(false);
                state::checkout(store, press_name.clone());
            }
        })
        .child(icon(glyph, 13.0).text_color(move || {
            if is_head {
                theme().primary
            } else {
                icon_color()
            }
        }))
        .child(
            text(shown)
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .text_color(move || if is_head { theme().primary } else { fg() }),
        );
    let built = if is_head && (ahead > 0 || behind > 0) {
        built.child(
            text(format!("↑{ahead} ↓{behind}"))
                .text_size(px(11.0))
                .text_color(move || fg_muted()),
        )
    } else {
        built
    };
    let built = if starred {
        built.child(icon(Icon::Star, 12.0).text_color(move || fg_muted()))
    } else {
        built
    };
    let built = if let Some(note) = note {
        built.child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .flex_none()
                .child(
                    text(note)
                        .text_size(px(11.0))
                        .text_color(move || fg_muted()),
                )
                .child(icon(Icon::ChevronRight, 10.0).text_color(move || fg_muted())),
        )
    } else {
        built
    };
    crate::ui::menus::branch_menu(store, built, move || menu_name.clone(), is_head, is_tag)
        .into_element()
}

// ---------------------------------------------------------------------------
// The commit box
// ---------------------------------------------------------------------------

/// What the commit box's `…` offers: the ways to make the commit the list
/// describes.
pub fn commit_box_menu(store: Store, trigger: impl IntoElement) -> impl IntoElement {
    let mut items: Vec<AnyElement> = Vec::new();
    items.push(
        item("Commit and Push…", move || {
            state::commit(store);
            state::push(store);
        })
        .into_element(),
    );
    items.push(item("Commit Silently", move || state::commit(store)).into_element());
    items.push(rule().into_element());
    items.push(item("Stash Changes…", move || state::stash(store)).into_element());
    items.push(item("Unstash All…", move || state::stash_pop(store, 0)).into_element());
    menu(trigger, items)
}
