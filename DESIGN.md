# Design notes

Grove copies the JetBrains *rebased* Git tool window's structure — the
IntelliJ "Islands" layout — rather than inventing a look. This file records
the numbers and the motion that came from it, so a change can be checked
against the original instead of guessed.

## Metrics

At 100% zoom, from `theme_tokens`:

| Thing | Points |
| --- | --- |
| Title bar | 40 |
| The tab strip's island | 36 |
| A tool window header (the log header) | 40 |
| A bar inside a pane (search, filters, the details toolbar) | 32 |
| A stripe button | 30 |
| Status bar | 34 |
| A list row (commit, file, tree) | 26 |
| Island gap | 6 |
| Island radius | 10 |
| Commit dot radius | 3.6 |

The window is a flat frame with islands floating on it: the sidebar island,
the main island (the tab strip along its top, the tab below it) and the Git
island, with the same 6pt gap between them and the bars above and below. The
sidebar and the Git area are `h_resizable` splits; the main area and the Git
area are a `v_resizable` split, and the Git area spans the window's full
width beneath both.

## The sections

The stripe switches one sidebar between two tool windows — Project and
Commit — and its Git button at the bottom toggles the Git area below the
tabs. The sidebar island carries one tool window header — the section's icon
and name, the close cross and the `…` — and the pane below it:

- **Project** — the working tree as an expandable tree; a file press opens an
  editor tab, a right click offers Open, Show Diff, Add to Index, Unstage,
  Rollback, Add to .gitignore, Show History, Copy Path and Reveal.
- **Commit** — the change list (Staged above Unstaged, a checkbox on every
  row and a tri-state one on each header) and the commit box.

The Git area under them is hidden until the stripe's Git button is pressed
and hidden again by its close cross: the stashes above the branches tree, the
log with its graph, and the details of what is selected.

The main area holds one tab per opened file, change, commit file or stash; a
change's diff carries per-hunk Stage and Rollback actions.

## Colour

Every colour is a theme token with a light and a dark value in `theme.json`,
read through `theme_tokens`:

- `frame` — the ground the islands float on
- `island` — an island's own surface
- `island-hover`, `island-selected` — a row
- `toolbar-hover`, `toolbar-selected` — a button, and the active stripe button
- `tab-selected`, `tab-selected-border` — the lit tab pill
- `graph-1…8` — the commit graph's lanes, one colour per lane
- `change-*` — a file's status letter

No screen names a literal colour, so light and dark come from one set of
rules and a new screen inherits both.

## Motion

Two durations, and nothing else moves:

- `MOTION_FAST` (80ms) — a hover or press settling. Every button, row and
  tab uses it, through `transition_all` and `duration`.
- Panels and tabs switch without animation: the reference's tool window
  swaps its pane at once, and an animated swap reads as lag in a client whose
  whole point is that a click is answered in the same frame.

The one looping animation is the status bar's busy dot, which pulses while a
read or a write is in flight.

## Type

13pt medium for a title, a pill and a subject; 12.5pt for a row's main text;
12pt for a diff and a body; 11.5pt for a secondary line; 11pt for a count and
a hint; 10.5pt for a badge. The mono face comes from the theme's `font-mono`
and is used by the diff only.
