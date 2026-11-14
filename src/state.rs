//! The window's state: one set of signals the whole UI reads.
//!
//! A component runs once in NSH, so state is not props passed down and
//! compared: it is signals in one owner (`App`), shared by context. Every
//! screen reads the same [`Store`], which is `Copy` because a signal handle
//! is, so passing it costs nothing.
//!
//! The window has three regions and each keeps its state here: the sidebar's
//! section ([`Side`]), the open tabs ([`Tab`]), and what the Git area below
//! the tabs is about ([`Target`]).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use git_core::CommitDetails;
use nsh_core::prelude::*;

use crate::model::{self, Filter, Snapshot};

/// How many commits one page of the log reads.
pub const PAGE: usize = 400;

/// Which section the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// The Project tool window: the working tree, as a tree.
    Project,
    /// The Commit tool window: the change list and the commit box.
    #[default]
    Commit,
}

/// What the Git area below the tabs is about: the selection the log and the
/// lists share.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    None,
    /// A commit: its message and its patch.
    Commit(String),
    /// One file of the working tree, and which side of the index.
    Change { path: String, staged: bool },
    /// A stash, by index.
    Stash(u32),
}

impl Target {
    /// The commit the Git area is showing, when it is one.
    pub fn hash(&self) -> Option<&str> {
        match self {
            Self::Commit(hash) => Some(hash),
            _ => None,
        }
    }
}

/// What an open tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    /// A working-tree file, which the reader can edit and save.
    File,
    /// A patch: a commit's, a change's, a stash's.
    Diff,
    /// A file's lines, each with the commit and the author that last
    /// touched it. Read-only: a blame is a reading of history, not text.
    Blame,
    /// Two folders laid side by side: the files only one side has, and the
    /// files both have with different content. Neither has to be in a
    /// repository, which is what makes this the way to compare two checkouts
    /// of one project on different branches.
    Compare,
}

/// One open tab of the main area.
///
/// The text is a signal so an edit updates the tab in place: the tab strip
/// reads its dirty mark, and saving writes the same value to disk. A diff's
/// text is its patch, which never changes after it loads.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    /// What identifies the tab: a file opens once, however often it is
    /// pressed.
    pub key: String,
    pub title: String,
    pub kind: TabKind,
    /// Repository-relative, empty for a diff that is about a commit.
    pub path: String,
    /// The highlighting language, as the editor names one.
    pub language: &'static str,
    pub text: Signal<String>,
    /// The text as it was read from disk, for the dirty mark and for save.
    /// It is a signal of its own so saving puts the dirty mark out without
    /// rebuilding the editor, which would throw away its undo history.
    pub saved: Signal<String>,
    /// The commit a diff tab was opened from, for the tab's tooltip.
    pub commit: Option<String>,
    /// A change diff's side of the index: `Some(true)` staged,
    /// `Some(false)` unstaged, `None` for a commit or a stash.
    pub staged: Option<bool>,
    /// Bumped when the text under the editor is replaced from disk. The
    /// editor keeps its own text, so a reload has to make it again, and this
    /// is what makes the tab compare unequal and the body rebuild.
    pub revision: u32,
    /// The file's text in the index when the tab was opened, for the change
    /// bars the editor draws beside the lines the reader has touched. A file
    /// the index does not have starts empty, so every line of a new file
    /// reads as added.
    pub baseline: String,
    /// A folder comparison's two sides, for a [`TabKind::Compare`] tab.
    pub comparison: Option<Signal<Option<crate::compare::Comparison>>>,
    /// A folder comparison's *Compare by:* value: what makes two files of
    /// the same path count as different.
    pub compare_mode: Option<Signal<crate::compare::CompareMode>>,
    /// The path of the comparison's selected row, whose diff the pane under
    /// the table shows.
    pub compared: Option<Signal<Option<String>>>,
}

impl Tab {
    /// Whether the tab holds an edit that is not on disk.
    ///
    /// The text is read as a tracked read: the tab strip's dot and the
    /// editor's own bar subscribe to it, so typing lights the dot in the same
    /// frame and saving puts it out.
    pub fn is_dirty(&self) -> bool {
        self.kind == TabKind::File && self.text.get() != self.saved.get()
    }
}

/// What an open prompt is for.
///
/// A Git action that needs a name — a new branch, a new tag, a rename —
/// asks through one modal rather than one dialog per action: the action
/// decides the title, the hint, the start value, and what a press does with
/// the text.
#[derive(Clone)]
pub struct Prompt {
    pub title: &'static str,
    pub hint: &'static str,
    pub value: Signal<String>,
    pub confirm: &'static str,
    /// What the press does, given the trimmed text.
    pub accept: Rc<dyn Fn(String)>,
}

impl std::fmt::Debug for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prompt")
            .field("title", &self.title)
            .finish()
    }
}

impl PartialEq for Prompt {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title && self.value == other.value
    }
}

/// What an open confirmation is for.
///
/// The actions that cannot be undone ask here first — a delete, a rollback —
/// with the name of what they would touch, so a press on a menu item cannot
/// throw away a file the reader did not mean to pick.
#[derive(Clone)]
pub struct Confirm {
    pub title: String,
    pub hint: String,
    pub confirm: &'static str,
    /// What the confirm button does.
    pub accept: Rc<dyn Fn()>,
}

impl std::fmt::Debug for Confirm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Confirm")
            .field("title", &self.title)
            .finish()
    }
}

impl PartialEq for Confirm {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title && self.hint == other.hint
    }
}

/// The signals the UI shares.
#[derive(Clone, Copy)]
pub struct Store {
    pub snapshot: Signal<Snapshot>,
    pub filter: Signal<Filter>,
    /// The branch the log walks; `None` walks every ref.
    pub reference: Signal<Option<String>>,
    /// What the Git area shows.
    pub target: Signal<Target>,
    /// The selected commit's message and files.
    pub details: Signal<Option<CommitDetails>>,
    /// The selected target's patch: a commit's, a file's, a stash's.
    pub patch: Signal<String>,
    /// The open tabs, in the order they were opened.
    pub tabs: Signal<Vec<Tab>>,
    /// The key of the tab on screen.
    pub active: Signal<Option<String>>,
    pub side: Signal<Side>,
    /// Whether the Git area under the tabs is showing. It starts hidden: the
    /// log is a tool window the stripe's Git button opens and closes.
    pub git_area: Signal<bool>,
    /// The paths whose changes go into the next commit. A path absent from
    /// the set is left out; every path starts in it.
    pub included: Signal<HashSet<String>>,
    /// The Project tree's open folders, as repository-relative paths; the
    /// root is `""` and starts open. It lives here rather than in the pane,
    /// so a switch to Commit and back keeps the tree as the reader left it.
    pub expanded: Signal<HashSet<String>>,
    pub busy: Signal<bool>,
    /// The commit message being written.
    pub message: Signal<String>,
    pub amend: Signal<bool>,
    pub toast: Signal<Option<String>>,
    pub limit: Signal<usize>,
    /// The name prompt on screen, when one is.
    pub prompt: Signal<Option<Prompt>>,
    /// The confirmation on screen, when one is: a delete, a rollback.
    pub confirm: Signal<Option<Confirm>>,
    /// A path Cut or Copy put aside, and whether Cut took it: a paste moves
    /// the file for a cut and makes a copy of it for a copy. `None` while
    /// nothing is on the file clipboard.
    pub file_clip: Signal<Option<(String, bool)>>,
    /// The Project tree's picked rows, in press order, as repository-relative
    /// paths. Ctrl+Click adds one and Ctrl+Click again takes it out; a plain
    /// press replaces the set with the one row. Two picked folders are what
    /// Ctrl+D compares.
    pub picked: Signal<Vec<String>>,
    /// Whether the Clone Repository dialog is on screen.
    pub clone_open: Signal<bool>,
    /// The dialog's fields: what is cloned, where into, and how deep.
    pub clone_url: Signal<String>,
    pub clone_into: Signal<String>,
    /// The directory the clone's folder goes under: what the "Clone into"
    /// field's head is derived from, and what browsing replaces.
    pub clone_parent: Signal<String>,
    /// Whether the "Clone into" field was edited by hand: while it has not,
    /// the URL keeps deriving the field's tail.
    pub clone_custom_into: Signal<bool>,
    pub clone_shallow: Signal<bool>,
    pub clone_depth: Signal<String>,
    /// Whether the clone's `git` is still running.
    pub clone_running: Signal<bool>,
    /// What the clone is doing, or why it failed.
    pub clone_status: Signal<String>,
    /// The folders the window remembers, newest first, as the project menu
    /// lists them.
    pub recents: Signal<Vec<PathBuf>>,
    /// The branch each recent project has checked out, by its path on disk.
    pub recent_branches: Signal<HashMap<String, String>>,
    /// The update dialog, while it is open, the way it merges, and the
    /// "Don't show again" of its footer.
    pub update_open: Signal<bool>,
    pub update_rebase: Signal<bool>,
    pub update_forget: Signal<bool>,
    /// The push dialog: whether it is up, what it shows, the commit picked
    /// in it, the file whose diff is on, and the footer's tags choice.
    pub push_open: Signal<bool>,
    pub push_data: Signal<Arc<model::PushData>>,
    pub push_selected: Signal<Option<usize>>,
    /// The file tree's collapsed folders, by their full path prefix.
    pub push_collapsed: Signal<std::collections::HashSet<String>>,
    pub push_diff: Signal<Option<(String, String)>>,
    pub push_patch: Signal<String>,
    pub push_tags: Signal<bool>,
    pub push_tags_all: Signal<bool>,
    pub push_running: Signal<bool>,
    pub push_error: Signal<String>,
    /// The branch popup's own state: whether it is up, what its search field
    /// shows, the groups it has folded (`Local`, `remote/origin`, `Tags`),
    /// and whether it lists the tags at all.
    pub branch_popup_open: Signal<bool>,
    pub branch_popup_query: Signal<String>,
    pub branch_popup_collapsed: Signal<std::collections::HashSet<String>>,
    pub branch_popup_show_tags: Signal<bool>,
}

impl Store {
    pub fn new() -> Self {
        let store = Self {
            snapshot: signal(Snapshot::default()),
            filter: signal(Filter::default()),
            reference: signal(None),
            target: signal(Target::default()),
            details: signal(None),
            patch: signal(String::new()),
            tabs: signal(Vec::new()),
            active: signal(None),
            side: signal(Side::default()),
            git_area: signal(false),
            included: signal(HashSet::new()),
            expanded: signal(HashSet::from([String::new()])),
            busy: signal(false),
            message: signal(String::new()),
            amend: signal(false),
            toast: signal(None),
            limit: signal(PAGE),
            prompt: signal(None),
            confirm: signal(None),
            file_clip: signal(None),
            picked: signal(Vec::new()),
            clone_open: signal(false),
            clone_url: signal(String::new()),
            clone_into: signal(String::new()),
            clone_parent: signal(String::new()),
            clone_custom_into: signal(false),
            clone_shallow: signal(false),
            clone_depth: signal("1".to_owned()),
            clone_running: signal(false),
            clone_status: signal(String::new()),
            recents: signal(model::recent_paths()),
            recent_branches: signal(HashMap::new()),
            update_open: signal(false),
            update_rebase: signal(false),
            update_forget: signal(false),
            push_open: signal(false),
            push_data: signal(Arc::new(model::PushData::default())),
            push_selected: signal(None),
            push_diff: signal(None),
            push_collapsed: signal(Default::default()),
            push_patch: signal(String::new()),
            push_tags: signal(false),
            push_tags_all: signal(true),
            push_running: signal(false),
            push_error: signal(String::new()),
            branch_popup_open: signal(false),
            branch_popup_query: signal(String::new()),
            branch_popup_collapsed: signal(Default::default()),
            branch_popup_show_tags: signal(true),
        };
        // Every recent project's checked-out branch, read off disk when the
        // list changes: the project menu shows one under each project's path.
        let branches = store.recent_branches;
        effect(move || {
            let paths = store.recents.get();
            let read_branches = move || {
                paths
                    .into_iter()
                    .filter_map(|path| {
                        model::head_branch_of(&path)
                            .map(|branch| (path.display().to_string(), branch))
                    })
                    .collect::<HashMap<String, String>>()
            };
            off_thread(read_branches, move |read| {
                // A stale read outlives a change of the list: keep only
                // the paths still on it.
                let current: HashSet<String> = store
                    .recents
                    .get_untracked()
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                branches.set(
                    read.into_iter()
                        .filter(|(path, _)| current.contains(path))
                        .collect(),
                );
            });
        });
        store
    }

    pub fn paths(&self) -> Option<git_core::RepoPaths> {
        self.snapshot.get_untracked().paths
    }

    /// Whether a repository is open.
    ///
    /// A tracked read: the pane that swaps the workspace for the welcome
    /// screen re-runs when the first snapshot arrives. Everywhere else a
    /// read outside a binding is the same read.
    pub fn is_open(&self) -> bool {
        self.snapshot.with(|snapshot| snapshot.is_open())
    }

    /// Shows `text` for a few seconds, over the status bar.
    pub fn say(&self, text: impl Into<String>) {
        let toast = self.toast;
        let shown = text.into();
        toast.set(Some(shown.clone()));
        set_timeout(Duration::from_secs(5), move || {
            // Only clear what is still showing: a newer message keeps its
            // own five seconds.
            if toast.get_untracked().as_deref() == Some(shown.as_str()) {
                toast.set(None);
            }
        });
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `work` off the engine thread and hands its value back on it.
///
/// Reading a repository is milliseconds of libgit2, but a status on a huge
/// working tree is seconds, and the engine thread draws every frame: the read
/// goes to the runtime's blocking pool and only its result comes back.
pub fn off_thread<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
    then: impl FnOnce(R) + 'static,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Detached: what asked for the read usually dies first. A menu row
        // opens a repository and the menu closes; a clone dialog opens what
        // it cloned and the dialog unmounts. A task of the asking view dies
        // with it, at the next wake of its await — the snapshot would come
        // back to nobody. Detached, the read runs to its end and its
        // completion reaches the store whatever closed in the meantime.
        Owner::detached().run(|| {
            spawn(async move {
                if let Ok(value) = blocking(work).await {
                    then(value);
                }
            });
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        Owner::detached().run(|| {
            spawn(async move {
                then(work());
            });
        });
    }
}

/// Reads the repository again, keeping what is selected.
pub fn refresh(store: Store) {
    let Some(paths) = store.paths() else { return };
    let limit = store.limit.get_untracked();
    let reference = store.reference.get_untracked();
    // The path filter narrows the walk itself: a file's history is the
    // commits that touched it, and filtering a listing afterwards would page
    // through commits that never did.
    let filter_path = store.filter.with(|filter| {
        let path = filter.path.trim().to_owned();
        (!path.is_empty()).then_some(path)
    });
    store.busy.set(true);
    let root = paths.root.clone();
    off_thread(
        move || model::load(&root, limit, reference.as_deref(), filter_path.as_deref()),
        move |snapshot| {
            if let Some(error) = snapshot.error.clone() {
                store.say(error);
            }
            // A path that is no longer changed leaves the commit selection.
            let changed: HashSet<String> = snapshot
                .status
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect();
            store.included.update(|included| {
                included.retain(|path| changed.contains(path));
                for path in &changed {
                    if !included.contains(path) {
                        included.insert(path.clone());
                    }
                }
            });
            store.snapshot.set(snapshot);
            store.busy.set(false);
        },
    );
}

/// Opens `folder` (the repository containing it) and remembers it.
pub fn open_folder(store: Store, folder: PathBuf) {
    let limit = store.limit.get_untracked();
    store.busy.set(true);
    store.target.set(Target::None);
    store.details.set(None);
    store.patch.set(String::new());
    store.tabs.set(Vec::new());
    store.active.set(None);
    store.included.set(HashSet::new());
    let remembered = folder.clone();
    off_thread(
        move || model::open_folder(&folder, limit),
        move |snapshot| {
            if let Some(error) = snapshot.error.clone() {
                store.say(error);
            }
            if snapshot.is_open() {
                model::remember_path(&remembered);
                store.recents.set(model::recent_paths());
            }
            store.included.set(
                snapshot
                    .status
                    .entries
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect(),
            );
            store.snapshot.set(snapshot);
            store.busy.set(false);
        },
    );
}

/// Asks for a folder and opens it.
pub fn pick_folder(store: Store) {
    spawn(async move {
        let answer = pick_paths(nsh_core::file_dialog::OpenDialog::folders()).await;
        if let Ok(Some(mut folders)) = answer
            && let Some(folder) = folders.pop()
        {
            open_folder(store, folder);
        }
    });
}

/// Opens the Clone Repository dialog, its fields reset to the defaults.
pub fn open_clone_dialog(store: Store) {
    store.clone_url.set(String::new());
    // The field starts at the directory clones go into; the URL derives the
    // clone's folder under it.
    let parent = model::clone_dir_default().display().to_string();
    store.clone_parent.set(parent.clone());
    store.clone_into.set(parent);
    store.clone_custom_into.set(false);
    store.clone_shallow.set(false);
    store.clone_depth.set("1".to_owned());
    store.clone_status.set(String::new());
    store.clone_running.set(false);
    store.clone_open.set(true);
}

/// Closes the Clone Repository dialog.
///
/// While a clone is running it stays: closing it would leave the `git`
/// working with nowhere to say what it did.
pub fn dismiss_clone(store: Store) {
    if store.clone_running.get_untracked() {
        store.say("The clone is still running");
        return;
    }
    store.clone_open.set(false);
}

/// Asks for the directory clones go into: the picked directory becomes the
/// parent, and the clone's folder is derived under it again.
pub fn browse_clone_into(store: Store) {
    let into = store.clone_into;
    let parent = store.clone_parent;
    let custom = store.clone_custom_into;
    spawn(async move {
        if let Ok(Some(mut folders)) =
            pick_paths(nsh_core::file_dialog::OpenDialog::folders()).await
            && let Some(folder) = folders.pop()
        {
            let picked = folder.display().to_string();
            parent.set(picked.clone());
            custom.set(false);
            into.set(model::join_clone_path(
                &picked,
                &model::clone_dir_name(&into.get_untracked()),
            ));
        }
    });
}

/// Writes the URL field. While the "Clone into" field has not been edited by
/// hand, the URL keeps deriving its tail — the clone's folder name under the
/// remembered parent — as the reference's does, so typing a URL fills the
/// whole target path.
pub fn set_clone_url(store: Store, url: &str) {
    store.clone_url.set(url.to_owned());
    if store.clone_custom_into.get_untracked() {
        return;
    }
    store.clone_into.set(model::join_clone_path(
        &store.clone_parent.get_untracked(),
        &model::clone_dir_name(url),
    ));
}

/// Writes the "Clone into" field by hand: the URL stops deriving it.
pub fn set_clone_into(store: Store, path: &str) {
    store.clone_custom_into.set(true);
    store.clone_into.set(path.to_owned());
}

/// Clones the URL the dialog names into its directory, then opens it.
pub fn start_clone(store: Store) {
    if store.clone_running.get_untracked() {
        return;
    }
    let url = store.clone_url.get_untracked().trim().to_owned();
    let Some(name) = repo_name(&url) else {
        store.say("Enter a repository URL to clone");
        return;
    };
    let into = PathBuf::from(store.clone_into.get_untracked().trim());
    if into.as_os_str().is_empty() {
        store.say("Choose a directory to clone into");
        return;
    }
    // The field names the whole target, tail derived from the URL: clone
    // into it as named.
    let target = into;
    if target.exists() {
        store.say(format!("{} already exists", target.display()));
        return;
    }
    let depth = store
        .clone_shallow
        .get_untracked()
        .then(|| store.clone_depth.get_untracked().trim().parse::<u32>().ok())
        .flatten()
        .filter(|depth| *depth > 0);
    store.clone_running.set(true);
    store.clone_status.set(format!("Cloning {name}"));
    let remember_into = target.clone();
    let remember_url = url.clone();
    let cloned = target.clone();
    off_thread(
        move || {
            let _ = std::fs::create_dir_all(&target);
            let mut command = std::process::Command::new("git");
            command
                .arg("clone")
                .arg("--progress")
                .arg(&url)
                .arg(&target);
            if let Some(depth) = depth {
                command.arg(format!("--depth={depth}"));
            }
            // As every run of git: no console window for the child to open
            // (it would flash one and take the focus).
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }
            command.output()
        },
        move |output| {
            store.clone_running.set(false);
            let succeeded = output
                .as_ref()
                .map(|output| output.status.success())
                .unwrap_or(false);
            if succeeded {
                model::remember_clone_dir(&remember_into);
                model::remember_clone_url(&remember_url);
                store.clone_status.set(String::new());
                store.clone_open.set(false);
                store.say(format!("Cloned {name}"));
                open_folder(store, cloned);
            } else {
                // `--progress` fills the output with receive counts; the
                // failure is the fatal or error line among them.
                let message = output
                    .ok()
                    .and_then(|output| {
                        String::from_utf8_lossy(&output.stderr)
                            .lines()
                            .rev()
                            .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "git could not clone the repository".to_owned());
                store.clone_status.set(message);
            }
        },
    );
}

/// A repository's name, from the URL it is cloned by: the last path segment,
/// without its `.git`, reduced to a folder name. Empty when the URL names
/// nothing.
fn repo_name(url: &str) -> Option<String> {
    let name = model::clone_dir_name(url);
    (!name.is_empty()).then_some(name)
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// Opens `tab`, or shows it when it is already open.
pub fn open_tab(store: Store, tab: Tab) {
    let key = tab.key.clone();
    let mut already = false;
    store.tabs.update(|tabs| {
        if let Some(at) = tabs.iter().position(|open| open.key == key) {
            // A file tab keeps its edits; reopening it does not reload.
            tabs[at] = tab.clone();
            already = true;
        } else {
            tabs.push(tab.clone());
        }
    });
    let _ = already;
    store.active.set(Some(key));
}

/// Closes the tab `key` names, and shows the one beside it.
pub fn close_tab(store: Store, key: String) {
    let mut next: Option<String> = None;
    store.tabs.update(|tabs| {
        let Some(at) = tabs.iter().position(|tab| tab.key == key) else {
            return;
        };
        tabs.remove(at);
        next = tabs
            .get(at.saturating_sub(1))
            .or_else(|| tabs.first())
            .map(|tab| tab.key.clone());
    });
    if store.active.get_untracked().as_deref() == Some(key.as_str()) {
        store.active.set(next);
    }
}

/// Shows the tab `key` names.
pub fn activate_tab(store: Store, key: String) {
    store.active.set(Some(key));
}

/// Binds the window's save key: Ctrl+S (Command+S on Apple platforms)
/// writes the file tab on screen, and does nothing while a diff is.
///
/// The editor holds the keyboard while it has the focus and keeps the keys
/// of its own commands, so the save is bound for the whole window rather
/// than by a press on the tab.
pub fn bind_save(store: Store) {
    on_key("mod+s", move |_: &str| {
        let Some(key) = store.active.get_untracked() else {
            return;
        };
        let is_file = store
            .tabs
            .get_untracked()
            .iter()
            .any(|tab| tab.key == key && tab.kind == TabKind::File);
        if is_file {
            save_tab(store, key);
        }
    });
}

/// Writes the tab's text to disk and clears its dirty mark.
pub fn save_tab(store: Store, key: String) {
    let Some(tab) = store
        .tabs
        .get_untracked()
        .into_iter()
        .find(|tab| tab.key == key)
    else {
        return;
    };
    if tab.kind != TabKind::File || tab.path.is_empty() {
        return;
    }
    let Some(paths) = store.paths() else { return };
    let text = tab.text.get_untracked();
    let writing = text.clone();
    let path = tab.path.clone();
    store.busy.set(true);
    off_thread(
        move || {
            let repo = paths.open()?;
            repo.write_file(&path, &writing)
        },
        move |result| {
            store.busy.set(false);
            match result {
                Ok(()) => {
                    store.tabs.update(|tabs| {
                        if let Some(open) = tabs.iter_mut().find(|open| open.key == key) {
                            open.saved.set(text.clone());
                        }
                    });
                    refresh(store);
                }
                Err(error) => store.say(format!("Save: {}", error.summary())),
            }
        },
    );
}

/// Reads the tab's file again, dropping the edit it holds.
pub fn reload_tab(store: Store, key: String, path: String) {
    let Some(paths) = store.paths() else { return };
    off_thread(
        move || paths.open().map(|repo| repo.read_file(&path)),
        move |result| {
            let Ok(Ok(text)) = result else { return };
            store.tabs.update(|tabs| {
                if let Some(tab) = tabs.iter_mut().find(|tab| tab.key == key) {
                    tab.saved.set(text.clone());
                    tab.text.set(text);
                    // The editor holds its own copy of the text, so the tab
                    // body is rebuilt from the file just read.
                    tab.revision = tab.revision.wrapping_add(1);
                }
            });
        },
    );
}

/// Asks for a name, and does what the answer is for.
pub fn ask(store: Store, prompt: Prompt) {
    prompt.value.set(String::new());
    store.prompt.set(Some(prompt));
}

/// Shows a confirmation and holds what its confirm button does.
pub fn ask_confirm(store: Store, confirm: Confirm) {
    store.confirm.set(Some(confirm));
}

/// Runs the open confirmation's action, and closes it.
pub fn accept_confirm(store: Store) {
    let Some(confirm) = store.confirm.get_untracked() else {
        return;
    };
    store.confirm.set(None);
    (confirm.accept)();
}

/// Closes the open confirmation without doing anything.
pub fn dismiss_confirm(store: Store) {
    store.confirm.set(None);
}

/// Runs the open prompt with the text it holds, and closes it.
pub fn accept_prompt(store: Store) {
    let Some(prompt) = store.prompt.get_untracked() else {
        return;
    };
    let text = prompt.value.get_untracked().trim().to_owned();
    store.prompt.set(None);
    if text.is_empty() {
        return;
    }
    (prompt.accept)(text);
}

/// Closes the open prompt without doing anything.
pub fn dismiss_prompt(store: Store) {
    store.prompt.set(None);
}

/// Asks for a name for a new branch started at `start`.
pub fn prompt_new_branch(store: Store, start: String) {
    let accept: Rc<dyn Fn(String)> = Rc::new(move |name: String| {
        create_branch(store, name, start.clone());
    });
    ask(
        store,
        Prompt {
            title: "New branch",
            hint: "The branch starts at the selected commit and is checked out.",
            value: signal(String::new()),
            confirm: "Create",
            accept,
        },
    );
}

/// Asks for a name for a new tag at `target`.
pub fn prompt_new_tag(store: Store, target: String) {
    let accept: Rc<dyn Fn(String)> = Rc::new(move |name: String| {
        create_tag(store, name, target.clone());
    });
    ask(
        store,
        Prompt {
            title: "New tag",
            hint: "The tag points at the selected commit.",
            value: signal(String::new()),
            confirm: "Create",
            accept,
        },
    );
}

/// Asks for the new name of `from`.
pub fn prompt_rename_branch(store: Store, from: String) {
    let value = signal(from.clone());
    let accept: Rc<dyn Fn(String)> = Rc::new(move |to: String| {
        rename_branch(store, from.clone(), to);
    });
    ask(
        store,
        Prompt {
            title: "Rename branch",
            hint: "The branch keeps its commits and its upstream.",
            value,
            confirm: "Rename",
            accept,
        },
    );
}

/// Asks for the new path of `from`, and moves the file there.
pub fn prompt_rename_file(store: Store, from: String) {
    let value = signal(from.clone());
    let accept: Rc<dyn Fn(String)> = Rc::new(move |to: String| {
        rename_file(store, from.clone(), to);
    });
    ask(
        store,
        Prompt {
            title: "Rename file",
            hint: "Git records the move, so the file keeps its history.",
            value,
            confirm: "Rename",
            accept,
        },
    );
}

/// Asks for the path of a new file, and makes it in `folder`.
pub fn prompt_new_file(store: Store, folder: String) {
    let accept: Rc<dyn Fn(String)> = Rc::new(move |name: String| {
        let path = join_path(&folder, &name);
        create_file(store, path);
    });
    ask(
        store,
        Prompt {
            title: "New file",
            hint: "Relative to the folder that was pressed.",
            value: signal(String::new()),
            confirm: "Create",
            accept,
        },
    );
}

/// Asks for the name of a new folder inside `folder`, and makes it.
pub fn prompt_new_folder(store: Store, folder: String) {
    let accept: Rc<dyn Fn(String)> = Rc::new(move |name: String| {
        let path = join_path(&folder, &name);
        create_folder(store, path);
    });
    ask(
        store,
        Prompt {
            title: "New folder",
            hint: "A `.gitkeep` goes inside, so Git keeps the empty folder.",
            value: signal(String::new()),
            confirm: "Create",
            accept,
        },
    );
}

/// `folder` and `name` as one repository-relative path.
fn join_path(folder: &str, name: &str) -> String {
    let name = name.trim().trim_start_matches('/');
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{}", folder.trim_end_matches('/'), name)
    }
}

/// Opens the working-tree file `path` in an editor tab.
pub fn open_file_tab(store: Store, path: String) {
    let key = format!("file:{path}");
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let read = path.clone();
    let title = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let language = language_of(&path);
    off_thread(
        move || {
            paths.open().map(|repo| {
                // The text the change bars are measured against: the file's
                // version in the index, which is what the change list calls
                // modified. A path the index does not have reads empty, so a
                // new file's every line is added.
                let baseline = repo.read_file_at(":0", &read).unwrap_or_default();
                let text = repo.read_file(&read)?;
                Ok::<_, git_core::GitError>((text, baseline))
            })
        },
        move |result| {
            let (text, baseline) = match result {
                Ok(Ok(pair)) => pair,
                Ok(Err(error)) => {
                    store.say(format!("Read: {}", error.summary()));
                    return;
                }
                Err(error) => {
                    store.say(format!("Read: {}", error.summary()));
                    return;
                }
            };
            let saved = text.clone();
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::File,
                    path: path.clone(),
                    language,
                    text: signal(text.clone()),
                    saved: signal(saved),
                    commit: None,
                    staged: None,
                    revision: 0,
                    baseline,
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// Opens a diff tab of one file of commit `hash`.
pub fn open_commit_file_tab(store: Store, hash: String, path: String) {
    let key = format!("diff:{hash}:{path}");
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let read = path.clone();
    let looked_up = hash.clone();
    let title = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let language = language_of(&path);
    let short: String = hash.chars().take(8).collect();
    off_thread(
        move || {
            paths.open().ok().and_then(|repo| {
                let patch = repo.commit_diff(&looked_up).ok()?;
                Some(git_core::patch::select(&patch, &read, None))
            })
        },
        move |patch| {
            let patch = patch.unwrap_or_default();
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Diff,
                    path: path.clone(),
                    language,
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: Some(short),
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// Opens a blame tab of the working-tree file `path`.
pub fn open_blame_tab(store: Store, path: String) {
    let key = format!("blame:{path}");
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let read = path.clone();
    let title = format!("{} (Blame)", path.rsplit('/').next().unwrap_or(&path));
    let language = language_of(&path);
    off_thread(
        move || {
            paths
                .open()
                .ok()
                .and_then(|repo| repo.blame(&read).ok())
                .unwrap_or_default()
        },
        move |lines| {
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Blame,
                    path: path.clone(),
                    language,
                    text: signal(blame_text(&lines)),
                    saved: signal(String::new()),
                    commit: None,
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// The blame of `lines` as one aligned block of text.
///
/// Each row reads `short hash  author  date  |  line`, the columns as wide as
/// the widest entry needs, which is what makes a blame readable in a plain
/// editor.
fn blame_text(lines: &[git_core::BlameLine]) -> String {
    let hash_width = lines
        .iter()
        .map(|line| line.short_hash.len())
        .max()
        .unwrap_or(8);
    let author_width = lines
        .iter()
        .map(|line| line.author.chars().count())
        .max()
        .unwrap_or(0)
        .min(18);
    let mut out = String::new();
    for line in lines {
        let date = git_core::format_timestamp(line.time);
        out.push_str(&format!(
            "{:hash_width$}  {:author_width$}  {}  |  {}\n",
            line.short_hash,
            truncate(&line.author, author_width),
            date,
            line.content,
            hash_width = hash_width,
            author_width = author_width,
        ));
    }
    out
}

/// `text` cut to `width` characters, with an ellipsis when it was longer.
fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Opens a diff tab of `path` between two revisions.
///
/// `from` is what the file is compared against — a branch, a tag, a commit
/// hash, or empty for the index's side; `to` is what it is compared to, with
/// `WORKTREE` reading the file as it is on disk.
pub fn open_compare_tab(store: Store, path: String, from: String, to: String) {
    let key = format!("compare:{from}..{to}:{path}");
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let read = path.clone();
    let title = format!(
        "{} ({})",
        path.rsplit('/').next().unwrap_or(&path),
        if to == "WORKTREE" || to.is_empty() {
            from.clone()
        } else {
            format!("{from}..{to}")
        }
    );
    let language = language_of(&path);
    let (for_from, for_to) = (from.clone(), to.clone());
    off_thread(
        move || {
            paths
                .open()
                .ok()
                .map(|repo| {
                    // The working tree is not a revision: comparing against
                    // it is the index-to-workdir diff, read from the same
                    // call the change tab uses.
                    if for_to == "WORKTREE" || for_to.is_empty() {
                        repo.file_diff(&read, false)
                    } else if for_from.is_empty() {
                        repo.file_diff_between(&read, "", &for_to)
                    } else {
                        repo.file_diff_between(&read, &for_from, &for_to)
                    }
                })
                .and_then(Result::ok)
                .unwrap_or_default()
        },
        move |patch| {
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Diff,
                    path: path.clone(),
                    language,
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: Some(from.clone()),
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// Asks what to compare `path` against, then opens the comparison.
pub fn prompt_compare(store: Store, path: String) {
    let accept: Rc<dyn Fn(String)> = Rc::new(move |revision: String| {
        open_compare_tab(store, path.clone(), revision, "WORKTREE".to_owned());
    });
    ask(
        store,
        Prompt {
            title: "Compare with revision",
            hint: "A branch, a tag or a commit hash; the working tree is the other side.",
            value: signal(String::new()),
            confirm: "Compare",
            accept,
        },
    );
}

// ---------------------------------------------------------------------------
// Comparing two folders, or two files, outside the repository
// ---------------------------------------------------------------------------

/// Narrows the log to the commits that touched `path`, and shows it.
///
/// The filter is what the log's Paths chip reads, so setting it and reading
/// the repository again is the whole of "show this file's history": the walk
/// itself is narrowed, not the listing after it.
pub fn show_history(store: Store, path: String) {
    store.filter.update(|filter| filter.path = path);
    store.git_area.set(true);
    refresh(store);
}

/// Picks `path` in the Project tree, as a press with or without Control does.
///
/// Without Control the set becomes this one row; with it, the row joins the
/// set, or leaves it when it was already in. Two picked folders are what
/// Ctrl+D compares, which is the reference's way of comparing two
/// directories without a dialog: pick both, press the key.
pub fn pick_path(store: Store, path: String, additive: bool) {
    store.picked.update(|picked| {
        if !additive {
            picked.clear();
            picked.push(path);
            return;
        }
        if let Some(at) = picked.iter().position(|open| *open == path) {
            picked.remove(at);
        } else {
            picked.push(path);
        }
    });
}

/// Whether `path` is one of the Project tree's picked rows.
pub fn is_picked(store: Store, path: &str) -> bool {
    store
        .picked
        .with(|picked| picked.iter().any(|open| open == path))
}

/// Compares the two picked rows, as Ctrl+D does.
///
/// Two folders open a directory comparison; two files open the diff of the
/// two, which is what a press on "Compare with File…" does. Any other count
/// says what is missing rather than opening an empty tab.
pub fn compare_picked(store: Store) {
    let Some(paths) = store.paths() else {
        store.say("Open a repository to compare its folders");
        return;
    };
    let picked = store.picked.get_untracked();
    if picked.len() != 2 {
        store.say("Select two folders (or two files) to compare, then press Ctrl+D");
        return;
    }
    let one = paths.root.join(&picked[0]);
    let other = paths.root.join(&picked[1]);
    match (one.is_dir(), other.is_dir()) {
        (true, true) => open_dir_compare_tab(store, one, other),
        (false, false) => open_file_compare_tab(store, one, other),
        _ => store.say("A folder can only be compared with another folder"),
    }
}

/// Binds Ctrl+D: the two picked rows, compared.
///
/// It is the window's key rather than a row's, because the reference's
/// Ctrl+D acts on the selection the tree already has, not on what the
/// pointer is over.
pub fn bind_compare(store: Store) {
    on_key("mod+d", move |_: &str| compare_picked(store));
}

/// Asks for a folder and compares it against the folder `path` names.
///
/// The folder picked need not be in this repository, or in any: the
/// reference's reason for this command is two checkouts of one project on
/// different branches, which are two ordinary folders on disk.
pub fn compare_folder(store: Store, path: String) {
    let start = store
        .paths()
        .map(|paths| {
            let root = paths.root;
            if path.is_empty() {
                root
            } else {
                root.join(&path)
            }
        })
        .unwrap_or_default();
    spawn(async move {
        let answer = pick_paths(
            nsh_core::file_dialog::OpenDialog::folders()
                .title("Select Folder to Compare")
                .button_label("Compare")
                .directory(start),
        )
        .await;
        if let Ok(Some(mut folders)) = answer
            && let Some(picked) = folders.pop()
            && let Some(paths) = store.paths()
        {
            let here = if path.is_empty() {
                paths.root
            } else {
                paths.root.join(&path)
            };
            open_dir_compare_tab(store, here, picked);
        }
    });
}

/// Asks for a file and compares it against the file `path` names.
pub fn compare_file(store: Store, path: String) {
    let Some(paths) = store.paths() else { return };
    let start = if path.is_empty() {
        paths.root.clone()
    } else {
        paths.root.join(&path)
    };
    let directory = start
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| paths.root.clone());
    let here = start;
    spawn(async move {
        let answer = pick_paths(
            nsh_core::file_dialog::OpenDialog::files()
                .title("Select File to Compare")
                .button_label("Compare")
                .directory(directory),
        )
        .await;
        if let Ok(Some(mut files)) = answer
            && let Some(picked) = files.pop()
        {
            open_file_compare_tab(store, here, picked);
        }
    });
}

/// Opens the comparison of two folders on disk.
pub fn open_dir_compare_tab(store: Store, left: PathBuf, right: PathBuf) {
    let key = format!("compare-dirs:{}:{}", left.display(), right.display());
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let title = format!("{} - {}", folder_name(&left), folder_name(&right));
    let comparison = signal(None::<crate::compare::Comparison>);
    let mode = signal(crate::compare::CompareMode::default());
    let compared = signal(None::<String>);
    let (for_left, for_right) = (left.clone(), right.clone());
    let (shown, written) = (left.clone(), right.clone());
    off_thread(
        move || crate::compare::compare_dirs(&for_left, &for_right),
        move |result| {
            // The first file the two sides differ on is what the preview
            // shows while the reader has not picked a row: a comparison
            // opens on something to read, not on an empty pane.
            if let Some(first) = result
                .rows
                .iter()
                .find(|row| !row.holds_folder() && (row.differs || row.only().is_some()))
                .map(|row| row.path.clone())
            {
                compared.set(Some(first));
            }
            comparison.set(Some(result));
        },
    );
    open_tab(
        store,
        Tab {
            key,
            title,
            kind: TabKind::Compare,
            path: format!("{}|{}", shown.display(), written.display()),
            language: "",
            text: signal(String::new()),
            saved: signal(String::new()),
            commit: None,
            staged: None,
            revision: 0,
            baseline: String::new(),
            comparison: Some(comparison),
            compare_mode: Some(mode),
            compared: Some(compared),
        },
    );
}

/// Walks a folder comparison again with its tab's *Compare by:* value.
///
/// The mode is the tab's own signal, so the control that changed it is the
/// one that names the tab: the walk runs again and the rows the table shows
/// are replaced in place.
pub fn recompare(
    store: Store,
    comparison: Signal<Option<crate::compare::Comparison>>,
    mode: Signal<crate::compare::CompareMode>,
) {
    let _ = store;
    let Some(roots) = comparison.with(|comparison| {
        comparison
            .as_ref()
            .map(|comparison| (comparison.left.clone(), comparison.right.clone()))
    }) else {
        return;
    };
    let picked = mode.get_untracked();
    let (left, right) = roots;
    off_thread(
        move || crate::compare::compare_dirs_with(&left, &right, picked),
        move |result| comparison.set(Some(result)),
    );
}

/// Whether the folder comparison's selected row is `path`.
pub fn is_compared(compared: Signal<Option<String>>, path: &str) -> bool {
    compared.with(|picked| picked.as_deref() == Some(path))
}

/// Selects `path` in a folder comparison, and shows its diff below.
pub fn select_compared(compared: Signal<Option<String>>, path: String) {
    compared.set(Some(path));
}

/// Opens the comparison of two files on disk.
pub fn open_file_compare_tab(store: Store, left: PathBuf, right: PathBuf) {
    let key = format!("compare-files:{}:{}", left.display(), right.display());
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let name = file_name(&left);
    let title = format!("{} - {}", name, file_name(&right));
    let language = language_of(&name);
    let (for_left, for_right) = (left.clone(), right.clone());
    off_thread(
        move || {
            crate::compare::diff_patch(Some(&for_left), Some(&for_right), &file_name(&for_left))
        },
        move |patch| {
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Diff,
                    path: format!("{}|{}", left.display(), right.display()),
                    language,
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: None,
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// Opens the diff of one file of a folder comparison.
///
/// A row only one side has diffs against nothing, which is what shows the
/// file as wholly added or wholly deleted.
pub fn open_compared_file_tab(store: Store, left_root: PathBuf, right_root: PathBuf, path: String) {
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let key = format!(
        "compare-pair:{}:{}:{path}",
        left_root.display(),
        right_root.display()
    );
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let language = language_of(&name);
    let title = format!("{} - {}", folder_name(&left_root), folder_name(&right_root));
    let (for_left, for_right, for_name) = (left_root.clone(), right_root.clone(), name.clone());
    let shown_path = path.clone();
    off_thread(
        move || {
            let left = for_left.join(&shown_path);
            let right = for_right.join(&shown_path);
            crate::compare::diff_patch(
                left.exists().then_some(left.as_path()),
                right.exists().then_some(right.as_path()),
                &for_name,
            )
        },
        move |patch| {
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Diff,
                    path,
                    language,
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: None,
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// A path's last segment, which is what a tab calls it.
fn folder_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A path's file name, for a tab's title and the editor's language.
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Opens a diff tab of a working-tree file.
pub fn open_change_tab(store: Store, path: String, staged: bool) {
    let key = format!("change:{}:{path}", if staged { "staged" } else { "work" });
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let read = path.clone();
    let title = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let language = language_of(&path);
    off_thread(
        move || {
            paths
                .open()
                .ok()
                .and_then(|repo| repo.file_diff(&read, staged).ok())
                .unwrap_or_default()
        },
        move |patch| {
            open_tab(
                store,
                Tab {
                    key,
                    title,
                    kind: TabKind::Diff,
                    path: path.clone(),
                    language,
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: None,
                    staged: Some(staged),
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// Opens a diff tab of stash `index`.
pub fn open_stash_tab(store: Store, index: u32) {
    let key = format!("stash:{index}");
    if store.tabs.get_untracked().iter().any(|tab| tab.key == key) {
        store.active.set(Some(key));
        return;
    }
    let Some(paths) = store.paths() else { return };
    let message = store.snapshot.with(|snapshot| {
        snapshot
            .stashes
            .iter()
            .find(|stash| stash.index == index)
            .map(|stash| stash.message.clone())
            .unwrap_or_else(|| format!("stash@{{{index}}}"))
    });
    off_thread(
        move || model::load_stash_patch(&paths, index),
        move |patch| {
            open_tab(
                store,
                Tab {
                    key,
                    title: message,
                    kind: TabKind::Diff,
                    path: String::new(),
                    language: "",
                    text: signal(patch),
                    saved: signal(String::new()),
                    commit: None,
                    staged: None,
                    revision: 0,
                    baseline: String::new(),
                    comparison: None,
                    compare_mode: None,
                    compared: None,
                },
            );
        },
    );
}

/// The editor's language for `path`, from its extension.
fn language_of(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or(path).to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" | "scss" | "less" => "css",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "xml" => "xml",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Selections
// ---------------------------------------------------------------------------

/// Shows one commit: its message, its files and its patch.
pub fn select_commit(store: Store, hash: String) {
    let Some(paths) = store.paths() else { return };
    store.target.set(Target::Commit(hash.clone()));
    store.details.set(None);
    store.patch.set(String::new());
    let for_details = paths.clone();
    let details_hash = hash.clone();
    off_thread(
        move || model::load_details(&for_details, &details_hash),
        move |details| {
            store.details.set(details);
        },
    );
    off_thread(
        move || model::load_commit_patch(&paths, &hash),
        move |patch| {
            store.patch.set(patch);
        },
    );
}

/// Shows one working-tree file's diff in the Git area.
pub fn select_change(store: Store, path: String, staged: bool) {
    let Some(paths) = store.paths() else { return };
    store.target.set(Target::Change {
        path: path.clone(),
        staged,
    });
    store.details.set(None);
    store.patch.set(String::new());
    off_thread(
        move || model::load_file_patch(&paths, &path, staged),
        move |patch| {
            store.patch.set(patch);
        },
    );
}

/// Shows one stash's patch in the Git area.
pub fn select_stash(store: Store, index: u32) {
    let Some(paths) = store.paths() else { return };
    store.target.set(Target::Stash(index));
    store.details.set(None);
    store.patch.set(String::new());
    off_thread(
        move || model::load_stash_patch(&paths, index),
        move |patch| {
            store.patch.set(patch);
        },
    );
}

/// Puts `path` in the next commit, or leaves it out, as `wanted` says.
pub fn set_included(store: Store, path: String, wanted: bool) {
    store.included.update(|included| {
        if wanted {
            included.insert(path.clone());
        } else {
            included.remove(&path);
        }
    });
}

/// Puts every change in the next commit, or none of them.
pub fn set_all_included(store: Store, included: bool) {
    if included {
        let paths = store.snapshot.with(|snapshot| {
            snapshot
                .status
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>()
        });
        store.included.set(paths.into_iter().collect());
    } else {
        store.included.set(HashSet::new());
    }
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Runs a write, then reads the repository again.
///
/// Every mutation goes through `git`, so hooks, signing and filters behave as
/// they do on the command line; the refresh afterwards is what makes the
/// result visible.
pub fn mutate<F>(store: Store, what: &'static str, write: F)
where
    F: FnOnce(&git_core::Repo) -> git_core::Result<()> + Send + 'static,
{
    let Some(paths) = store.paths() else { return };
    store.busy.set(true);
    off_thread(
        move || {
            let repo = paths.open()?;
            write(&repo)
        },
        move |result| {
            store.busy.set(false);
            match result {
                Ok(()) => refresh(store),
                Err(error) => store.say(format!("{what}：{}", error.summary())),
            }
        },
    );
}

/// Stages `paths`.
pub fn stage(store: Store, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    mutate(store, "Stage", move |repo| repo.stage(&paths));
}

/// Takes every change under `folder` into the next commit, and shows the
/// Commit pane so the reader sees what went in.
pub fn include_under(store: Store, folder: String) {
    let needle = format!("{folder}/");
    store.included.update(|included| {
        let under: Vec<String> = store
            .snapshot
            .with(|snapshot| {
                snapshot
                    .status
                    .entries
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .filter(|path| path == &folder || path.starts_with(&needle))
            .collect();
        for path in under {
            included.insert(path);
        }
    });
}

/// Unstages `paths`.
pub fn unstage(store: Store, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let has_head = store.snapshot.get_untracked().branch.head_commit.is_some();
    mutate(store, "Unstage", move |repo| repo.unstage(&paths, has_head));
}

/// Stages one hunk of `path` from `patch`.
pub fn stage_hunk(store: Store, patch: String, path: String, hunk: usize) {
    mutate(store, "Stage hunk", move |repo| {
        repo.stage_hunk(&patch, &path, Some(hunk))
    });
}

/// Unstages one hunk of `path` from `patch`.
pub fn unstage_hunk(store: Store, patch: String, path: String, hunk: usize) {
    mutate(store, "Unstage hunk", move |repo| {
        repo.unstage_hunk(&patch, &path, Some(hunk))
    });
}

/// Discards one hunk of `path` from `patch`.
pub fn discard_hunk(store: Store, patch: String, path: String, hunk: usize) {
    mutate(store, "Discard hunk", move |repo| {
        repo.discard_hunk(&patch, &path, Some(hunk))
    });
}

/// Discards the working-tree changes of `paths`.
pub fn discard(store: Store, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    // A rollback throws the reader's work away, and git keeps no copy of it,
    // so it asks first, as the reference does.
    let named = match paths.len() {
        1 => format!("'{}'", paths[0].rsplit('/').next().unwrap_or(&paths[0])),
        count => format!("{count} paths"),
    };
    let for_discard = paths.clone();
    let accept: Rc<dyn Fn()> = Rc::new(move || {
        let paths = for_discard.clone();
        mutate(store, "Discard", move |repo| repo.discard(&paths));
    });
    ask_confirm(
        store,
        Confirm {
            title: format!("Roll back {named}?"),
            hint: "The working-tree changes are thrown away. Git keeps no copy of them.".to_owned(),
            confirm: "Rollback",
            accept,
        },
    );
}

/// Adds `paths` to `.gitignore`.
pub fn ignore(store: Store, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    mutate(store, "Ignore", move |repo| repo.ignore(&paths));
}

/// Deletes `paths` from disk and from the index.
pub fn delete(store: Store, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    // A delete takes the file off the disk as well as out of the index, so it
    // asks first: one mis-aimed press in a menu would otherwise be the only
    // thing between the reader and a file they still wanted.
    let named = match paths.len() {
        1 => format!("'{}'", paths[0].rsplit('/').next().unwrap_or(&paths[0])),
        count => format!("{count} paths"),
    };
    let for_delete = paths.clone();
    let confirm: Rc<dyn Fn()> = Rc::new(move || {
        let paths = for_delete.clone();
        mutate(store, "Delete", move |repo| repo.delete(&paths));
    });
    ask_confirm(
        store,
        Confirm {
            title: format!("Delete {named}?"),
            hint: "The paths leave the disk as well as the index. This cannot be undone from here."
                .to_owned(),
            confirm: "Delete",
            accept: confirm,
        },
    );
}

/// Moves `from` to `to`, on disk and in the index.
pub fn rename_file(store: Store, from: String, to: String) {
    if from == to || to.trim().is_empty() {
        return;
    }
    mutate(store, "Rename", move |repo| repo.rename(&from, &to));
}

/// Makes the file `path`, empty, and stages it.
pub fn create_file(store: Store, path: String) {
    if path.trim().is_empty() {
        return;
    }
    mutate(store, "New file", move |repo| repo.create_file(&path, ""));
}

/// Makes the folder `path`, with a `.gitkeep` inside.
pub fn create_folder(store: Store, path: String) {
    if path.trim().is_empty() {
        return;
    }
    mutate(store, "New folder", move |repo| repo.create_folder(&path));
}

/// Puts `path` aside, to be moved by a later paste (`cut`).
pub fn cut_path(store: Store, path: String, cut: bool) {
    if path.is_empty() {
        return;
    }
    store.file_clip.set(Some((path, cut)));
}

/// Pastes what Cut or Copy put aside into `folder`.
///
/// A cut is a `git mv` into the folder, keeping the file's name; a copy is
/// the same file written beside the original with a ` copy` suffix, as a
/// file manager names one. A name already taken gets a number.
pub fn paste_into(store: Store, folder: String) {
    let Some((from, cut)) = store.file_clip.get_untracked() else {
        return;
    };
    let name = from.rsplit('/').next().unwrap_or(&from).to_owned();
    if cut
        && from
            .rsplit('/')
            .next()
            .is_some_and(|_| dir_of(&from) == folder)
    {
        // Cut and pasted into the folder it already sits in: nothing moves.
        return;
    }
    let target = unique_path(store, &folder, &name, !cut);
    store.file_clip.set(None);
    mutate(store, if cut { "Move" } else { "Copy" }, move |repo| {
        if cut {
            repo.rename(&from, &target)
        } else {
            let text = repo.read_file(&from)?;
            repo.write_file(&target, &text)
        }
    });
}

/// The folder part of `path`, empty at the top of the repository.
fn dir_of(path: &str) -> String {
    match path.rfind('/') {
        Some(at) => path[..at].to_owned(),
        None => String::new(),
    }
}

/// `folder` and `name` as one path that no file holds yet.
///
/// A copy of `a.txt` becomes `a copy.txt`; if that is taken too, `a copy
/// 2.txt`, and so on.
fn unique_path(store: Store, folder: &str, name: &str, copy: bool) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    };
    let taken = |candidate: &str| {
        let full = if folder.is_empty() {
            candidate.to_owned()
        } else {
            format!("{folder}/{candidate}")
        };
        store
            .snapshot
            .get_untracked()
            .status
            .entries
            .iter()
            .any(|entry| entry.path == full)
    };
    let first = match copy {
        true => format!("{stem} copy{extension}"),
        false => name.to_owned(),
    };
    if !taken(&first) {
        return join_path(folder, &first);
    }
    for number in 2..1000 {
        let candidate = format!("{stem} copy {number}{extension}");
        if !taken(&candidate) {
            return join_path(folder, &candidate);
        }
    }
    join_path(folder, &first)
}

/// Commits exactly the changes whose boxes are ticked.
///
/// Git commits the index, so the ticked unstaged paths are staged and the
/// unticked staged ones are unstaged first; the commit then holds what the
/// list said it would.
pub fn commit(store: Store) {
    let message = store.message.get_untracked();
    if message.trim().is_empty() {
        store.say("Write a commit message first");
        return;
    }
    let amend = store.amend.get_untracked();
    let included = store.included.get_untracked();
    let snapshot = store.snapshot.get_untracked();
    let Some(paths) = snapshot.paths.clone() else {
        return;
    };

    let mut to_stage: Vec<String> = Vec::new();
    let mut to_unstage: Vec<String> = Vec::new();
    for entry in &snapshot.status.entries {
        let wanted = included.contains(&entry.path);
        if wanted && entry.unstaged.is_some() {
            to_stage.push(entry.path.clone());
        }
        if !wanted && entry.staged.is_some() {
            to_unstage.push(entry.path.clone());
        }
    }
    let has_head = snapshot.branch.head_commit.is_some();

    store.busy.set(true);
    off_thread(
        move || {
            let repo = paths.open()?;
            if !to_stage.is_empty() {
                repo.stage(&to_stage)?;
            }
            if !to_unstage.is_empty() {
                repo.unstage(&to_unstage, has_head)?;
            }
            repo.commit(&message, amend, false, &[])
        },
        move |result| {
            store.busy.set(false);
            match result {
                Ok(()) => {
                    store.message.set(String::new());
                    store.amend.set(false);
                    refresh(store);
                }
                Err(error) => store.say(format!("Commit：{}", error.summary())),
            }
        },
    );
}

/// Checks `name` out.
pub fn checkout(store: Store, name: String) {
    mutate(store, "Checkout", move |repo| repo.checkout(&name, false));
}

/// Checks out the commit `hash`, leaving `HEAD` on it.
pub fn checkout_detached(store: Store, hash: String) {
    mutate(store, "Checkout", move |repo| repo.checkout(&hash, true));
}

/// Fetches every remote.
pub fn fetch(store: Store) {
    mutate(store, "Fetch", |repo| repo.fetch(""));
}

/// Opens the update dialog, unless the window has stopped asking about the
/// choice and can update straight away.
pub fn open_update_dialog(store: Store) {
    if let Some(rebase) = model::update_mode() {
        pull_mode(store, rebase);
        return;
    }
    store.update_rebase.set(false);
    store.update_forget.set(false);
    store.update_open.set(true);
}

/// Closes the update dialog without updating.
pub fn dismiss_update(store: Store) {
    store.update_open.set(false);
}

/// Pulls, merging or rebasing as the dialog chose.
fn pull_mode(store: Store, rebase: bool) {
    mutate(store, "Update", move |repo| repo.pull(rebase));
}

/// Pulls the current branch, merging — the branch menu's direct command.
pub fn pull_merge(store: Store) {
    pull_mode(store, false);
}

/// Updates as the dialog chose, remembering it when it shall not be asked
/// about again.
pub fn confirm_update(store: Store) {
    let rebase = store.update_rebase.get_untracked();
    if store.update_forget.get_untracked() {
        model::remember_update_mode(rebase);
    }
    store.update_open.set(false);
    pull_mode(store, rebase);
}

/// Moves the current branch to its upstream, discarding everything the
/// update dialog's reset offers to give up.
pub fn reset_to_remote(store: Store) {
    mutate(store, "Reset to the Remote Branch", |repo| {
        repo.reset("@{u}", "hard")
    });
}

/// Pushes the current branch straight away, as the branch menu does.
pub fn push(store: Store) {
    let has_upstream = store.snapshot.get_untracked().branch.upstream.is_some();
    mutate(store, "Push", move |repo| {
        repo.push_with_tags(has_upstream, false, git_core::TagPush::None)
    });
}

/// Pushes one tag to `origin`, as the branch popup's tag menu does.
pub fn push_tag(store: Store, name: String) {
    mutate(store, "Push tag", move |repo| repo.push_tag(&name));
}

/// Opens the push dialog and reads what it would send.
pub fn open_push(store: Store) {
    let Some(paths) = store.paths() else { return };
    store.push_open.set(true);
    store.push_error.set(String::new());
    let limit = PAGE;
    off_thread(
        move || model::load_push(&paths, limit),
        move |data| {
            store.push_data.set(Arc::new(data));
            store.push_selected.set(None);
            store.push_collapsed.set(Default::default());
            store.push_diff.set(None);
            store.push_patch.set(String::new());
        },
    );
}

/// Closes the push dialog; not while its push is running.
pub fn dismiss_push(store: Store) {
    if store.push_running.get_untracked() {
        store.say("The push is still running");
        return;
    }
    store.push_open.set(false);
}

/// Picks the commit the push dialog's file column shows; `None` shows every
/// path of every commit together.
pub fn select_push_commit(store: Store, pick: Option<usize>) {
    store.push_selected.set(pick);
    store.push_diff.set(None);
    store.push_patch.set(String::new());
}

/// Shows the diff one commit introduced to one file, in the dialog.
pub fn open_push_diff(store: Store, hash: String, path: String) {
    let Some(paths) = store.paths() else { return };
    let parent = store
        .push_data
        .get_untracked()
        .commits
        .iter()
        .find(|commit| commit.hash == hash)
        .and_then(|commit| commit.parents.first().cloned());
    store.push_diff.set(Some((hash.clone(), path.clone())));
    store.push_patch.set(String::new());
    let for_then = (hash.clone(), path.clone());
    off_thread(
        move || model::load_commit_file_patch(&paths, &hash, parent.as_deref(), &path),
        move |patch| {
            // The dialog may have moved on while the file was read: the
            // patch belongs to the file it was asked for.
            if store
                .push_diff
                .get_untracked()
                .is_some_and(|shown| shown == for_then)
            {
                store.push_patch.set(patch);
            }
        },
    );
}

/// Closes the file's diff, back to the file list.
pub fn close_push_diff(store: Store) {
    store.push_diff.set(None);
    store.push_patch.set(String::new());
}

/// Pushes as the dialog says: the tags choice, and the force the menu asked.
pub fn confirm_push(store: Store, force: bool) {
    let Some(paths) = store.paths() else { return };
    if store.push_running.get_untracked() {
        return;
    }
    let has_upstream = store.snapshot.get_untracked().branch.upstream.is_some();
    let tags = match (
        store.push_tags.get_untracked(),
        store.push_tags_all.get_untracked(),
    ) {
        (false, _) => git_core::TagPush::None,
        (true, true) => git_core::TagPush::All,
        (true, false) => git_core::TagPush::Followed,
    };
    store.push_running.set(true);
    store.push_error.set(String::new());
    off_thread(
        move || {
            let repo = paths.open()?;
            repo.push_with_tags(has_upstream, force, tags)
        },
        move |result| {
            store.push_running.set(false);
            match result {
                Ok(()) => {
                    store.push_open.set(false);
                    store.say("Pushed");
                    refresh(store);
                }
                Err(error) => store.push_error.set(error.summary()),
            }
        },
    );
}

/// Pushes the current branch, setting its upstream on the first push.
/// Stashes the working tree, untracked files included.
pub fn stash(store: Store) {
    mutate(store, "Stash", |repo| repo.stash(None, true));
}

/// Pops stash `index`.
pub fn stash_pop(store: Store, index: u32) {
    mutate(store, "Pop stash", move |repo| repo.stash_pop(index));
}

/// Drops stash `index`.
pub fn stash_drop(store: Store, index: u32) {
    mutate(store, "Drop stash", move |repo| repo.stash_drop(index));
}

/// Creates `name` at `start` and checks it out.
pub fn create_branch(store: Store, name: String, start: String) {
    if name.trim().is_empty() {
        return;
    }
    mutate(store, "New branch", move |repo| {
        repo.create_branch(&name, &start)
    });
}

/// Deletes the local branch `name`.
pub fn delete_branch(store: Store, name: String) {
    mutate(store, "Delete branch", move |repo| {
        repo.delete_branch(&name, false)
    });
}

/// Renames a branch.
pub fn rename_branch(store: Store, from: String, to: String) {
    if to.trim().is_empty() {
        return;
    }
    mutate(store, "Rename branch", move |repo| {
        repo.rename_branch(&from, &to)
    });
}

/// Merges `name` into the current branch.
pub fn merge(store: Store, name: String) {
    mutate(store, "Merge", move |repo| repo.merge(&name, false));
}

/// Rebases the current branch onto `name`.
pub fn rebase(store: Store, name: String) {
    mutate(store, "Rebase", move |repo| repo.rebase(&name));
}

/// Cherry-picks `hash` onto the current branch.
pub fn cherry_pick(store: Store, hash: String) {
    mutate(store, "Cherry-pick", move |repo| repo.cherry_pick(&hash));
}

/// Reverts commit `hash` with a new commit.
pub fn revert_commit(store: Store, hash: String) {
    mutate(store, "Revert", move |repo| repo.revert(&hash));
}

/// Moves the current branch to `target` with `mode` (`soft`, `mixed`,
/// `hard`).
pub fn reset_to(store: Store, target: String, mode: &'static str) {
    mutate(store, "Reset", move |repo| repo.reset(&target, mode));
}

/// Creates tag `name` at `target`.
pub fn create_tag(store: Store, name: String, target: String) {
    if name.trim().is_empty() {
        return;
    }
    mutate(store, "New tag", move |repo| repo.tag(&name, &target, None));
}

/// Deletes tag `name`.
pub fn delete_tag(store: Store, name: String) {
    mutate(store, "Delete tag", move |repo| repo.delete_tag(&name));
}

/// Copies `text` and says so.
pub fn copy(store: Store, text: String, what: &'static str) {
    if text.is_empty() {
        return;
    }
    set_clipboard(text);
    store.say(format!("Copied {what}"));
}

/// The folder the window opens on: the first remembered one that still
/// exists.
pub fn remembered_folder() -> Option<PathBuf> {
    model::recent_paths().into_iter().find(|path| path.is_dir())
}
