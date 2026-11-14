# Grove

A Git client for the desktop, drawn with [NSH](https://nsh.dev) in Rust. The
window is the JetBrains *rebased* Git tool window, on the IntelliJ Islands
layout: a flat frame with rounded islands floating on it, a three-section
sidebar, a main area of tabs, and the Git area with the log, its graph and the
details of what is selected.

Reads go through libgit2 in `crates/git-core`; writes go through the `git`
executable, so hooks, signing, credential helpers and `.gitattributes` filters
behave as they do on the command line.

## Run it

```sh
cargo run                    # this machine's desktop backend
nsh run                      # the same root through NSH's runner
nsh package --target web     # or ios, android, macos, linux, windows
```

`cargo run` starts the root directly on the native backend (Win32 on Windows,
AppKit on macOS, GTK 4 on Linux) and installs the backend's development
libraries first.

## How the code is arranged

| Path | What lives there |
| --- | --- |
| `src/lib.rs` | `App`: the frame, the workspace, the main column |
| `src/state.rs` | every signal, in one `Copy` `Store` the tree reads; the open tabs and the actions |
| `src/model.rs` | the `Send` snapshot a worker builds off the engine thread |
| `src/theme_tokens.rs` | the metrics and the theme-token colours, nothing literal |
| `src/ui/frame.rs` | the title bar (project and branch widgets), the stripe, the log header, the status bar |
| `src/ui/sidebar.rs` | the Project / Commit / Git sections and what each shows |
| `src/ui/project.rs` | the Project tree: folders, files, and the Git actions on a file |
| `src/ui/changes.rs` | the Commit section's change list, with a checkbox on every row |
| `src/ui/commit_box.rs` | the message, Amend / All, and Commit |
| `src/ui/stashes.rs` | the Git section's stashes |
| `src/ui/tabs.rs`, `src/ui/editor_tab.rs` | the tab strip, the editor, and the dirty marker |
| `src/ui/diff.rs` | the patch, through `nsh_base`'s `DiffView`, with per-hunk actions |
| `src/ui/log.rs` | the log: the filter bar, the commit rows, the branches tree |
| `src/ui/details.rs` | the right pane: its bar, the changed-file tree, the patch |
| `src/ui/menus.rs` | every menu: branch, branch popup, commit, change, stash, file, commit box |
| `src/ui/prompt.rs` | the one modal a Git action asks a name in |
| `crates/git-core` | the Git model: reads by libgit2, writes by `git`, and `patch.rs` for hunk selection |

The UI is Grove's own layer: `nsh-core` gives the primitives, `nsh-base` the
unstyled behaviour, and every colour, size and press state comes from
`theme.json`'s tokens through `theme_tokens`.

## The frame

Top to bottom the window is a 40pt title bar, the workspace, and a 34pt status
bar. Islands sit 6pt apart on the frame colour and are rounded by 10pt.

- **The title bar** holds the project widget (a press picks a folder) and the
  branch widget, whose press opens the branch popup: new branch, new tag,
  checkout a revision, fetch, update, push, then every local branch, remote
  branch and tag, each with its own actions.
- **The stripe** down the left edge switches the sidebar between the two
  sections, and its Git button at the bottom shows and hides the Git area.
- **The sidebar** is one tool window with a single header (its icon, its name,
  the close cross, `…`), showing Project (the working tree, as an expandable
  tree) or Commit (the change list and the commit box).
- **The main island** is the tab strip along its top and the tab on screen
  below it.
- **The Git area** spans the full width under the sidebar and the main island,
  and is hidden until its stripe button is pressed: the log header, then the
  stashes above the branches tree, the log with its graph, and the details
  pane. Each divider is a resizable split.

## Partial commits

Every row of the change list starts with a checkbox, and each group header's
checkbox is tri-state. `Commit` stages the ticked paths and unstages the
unticked ones before it commits, so the commit holds exactly what the list
said. Inside a change's diff, each hunk carries **Stage hunk** and **Rollback
hunk**, which apply that hunk alone through `git apply --cached`.

## Appearance

`nsh.json` asks for `"appearance": "system"`, so the window follows the
system's light or dark setting and re-reads it when the setting changes. Every
colour is a token with a light and a dark value in `theme.json`; no screen
names a literal, so both appearances come from one set of rules.
