# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Grove — a desktop Git client written in Rust with NSH 0.5.1, replicating the JetBrains *rebased* Git tool window (the IntelliJ "Islands" layout). Workspace with two crates: `grove` (the UI, `src/`) and `git-core` (`crates/git-core`, the Git model). `README.md` maps every file; `DESIGN.md` records the metrics, colours and motion copied from the reference.

## The reference rule

The prototype must follow Rebased. Every screen, popup, menu and dialog is modelled on the JetBrains *rebased* client, whose full source lives at `D:\code\grove\sources\rebased` (intellij-community, including `plugins/git4idea`). Priority when looking something up:

1. `sources/rebased` / intellij-community (e.g. `plugins/git4idea/shared/src/com/intellij/vcs/git/branch/popup/`, the action-group XML under `shared/resources/`) — the wording, action order, grouping and defaults come from here.
2. `sources/openidea` — another NSH-ecosystem replica, for NSH technique only when rebased doesn't answer the question.

Never invent a label, an action order or a dialog structure: find it in git4idea's sources first, and state what was left out and why when Grove lacks the capability (no fake menu items).

## NSH API references

When a builder method, prop or element behaviour is in doubt, check the official docs before guessing (note: `nsh.dev` resolves to a local server through the hosts file):

- Docs: https://nsh.dev/zh-CN/docs/
- Elements: https://nsh.dev/zh-CN/elements/
- API (Rust modules): https://nsh.dev/zh-CN/modules/

The crate sources in the cargo registry cache (`nsh-base-0.5.1`, `nsh-core-0.5.1` under `%USERPROFILE%\scoop\persist\rustup\.cargo\registry\src\rsproxy.cn-e3de039b2554c837\`) remain the ground truth when docs and code disagree.

## Commands

```sh
cargo build                          # build (runs the whole workspace)
cargo run                            # launch the desktop app
cargo clippy                         # lint — use clippy, never cargo check
cargo fmt                            # format
cargo nextest run                    # all Rust tests — nextest, never cargo test
cargo nextest run model::tests::an_empty_path_list_means_everything   # one test
nsh test                             # the TSX end-to-end test (App.test.tsx, headless runner)
```

Rust unit tests live in `#[cfg(test)]` modules next to the code they test. There is no separate test directory.

## Architecture

**One-way data flow; the UI never holds a `Repo`.** libgit2's handle is not shareable across threads, so every read runs on a worker: `state::refresh` spawns a read through `state::off_thread`, builds a `Send` `Snapshot` (`model.rs`), and replaces the signal's value whole. Every UI binding that read the old snapshot updates at once. Components run once in NSH — state is signals in one `Copy` `Store` (`state.rs`), shared by context, not props passed down.

**Reads through libgit2, writes through the `git` executable.** `git-core` reads (status, log, refs, diff, blame) in-process; every mutation goes through `state::mutate(store, "What", |repo| ...)`, which shells out to `git` so hooks, signing and `.gitattributes` filters behave as on the command line. A mutate is invisible until its `refresh` returns — never mutate without that trailing refresh, and never build a git command line in UI code.

**Menus are driven by closures, not strings.** A menu built inside a list cell outlives the row that cell was showing, so `menus.rs` takes `impl Fn() -> String` for the ref a menu acts on. The branch popup's own state (open, query, folded groups, show-tags) lives in the `Store`, and the popup is a controlled `nsh_base::menu` (`.open(...)` + `.on_open_change(...)` with `MenuEvent::OpenChange`); a menu's `children` are rebuilt on each opening, so a `dynamic` inside them re-reads the signals.

**Colours and metrics are tokens.** No screen names a literal colour or size: everything goes through `theme_tokens.rs` (`theme.json` holds a light and a dark value per token), so both appearances come from one set of rules. The metrics in `DESIGN.md` (40pt title bar, 26pt rows, 6pt island gap, …) are constants there, not inline numbers.

## NSH pitfalls (each of these cost a compile error or a wrong behaviour once)

- Rust edition 2024 captures closure fields precisely: two closures over the same `Rc<dyn Fn()>` each need their own `let name = rc.clone();` before the `move` — the label closure moving the `Rc` breaks the press closure.
- Inside a `move` closure, force a by-value copy of a `Store` field with a leading `let query = store.branch_popup_query;` when the borrow checker reads a method call through `&self`.
- A signal read inside a `dynamic` must be tracked (`get()` / `with(...)`); `get_untracked` builds no subscription and the UI goes stale.
- The API has no `scrollable`, no `.visible()`, no `into_any_element` on views — use the `scroll()` component, `style_when(cond, |s| s.hidden())`, and `into_element()` when pushing into `Vec<AnyElement>`.
- `state::off_thread` is detached on purpose: a task spawned by a view dies with that view (NSH cancels on unmount), and a snapshot read would come back to nobody. Keep using it for anything that outlives the asking view.
- `libgit2`'s `find_branch` takes the short branch name (`main`), not the full ref name (`refs/heads/main`).

## Worth knowing

- `theme.json` + `nsh.json` (`"appearance": "system"`) drive light/dark; nothing else may branch on appearance.
- `App.test.tsx` is the root's E2E test run by `nsh test`; the window opens on the last repository that still exists, or the welcome screen.
- `cargo clippy` reports ~260 pre-existing `redundant_closure` warnings across the codebase; match the surrounding style (`move || fg_muted()`) rather than mass-fixing them.
