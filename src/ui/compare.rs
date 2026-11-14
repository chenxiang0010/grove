//! A folder comparison, as the reference draws it: the two folders side by
//! side, one row per path, with the size and the write time each side has,
//! a *Compare by:* control over them, and the diff of the selected file
//! under them.
//!
//! A path only one side has is tinted the colour of an addition or a
//! deletion, and a path both have with different content the colour of a
//! modification, so "what is only here", "what is only there" and "what
//! changed" read at a glance without opening anything. A press selects a
//! row and shows its diff below; a double press opens it as a tab of its own.

use nsh_base::theme;
use nsh_base::{SelectItem, select};
use nsh_core::prelude::*;

use crate::compare::{CompareMode, Comparison, Row, Side};
use crate::state::{self, Store};
use crate::theme_tokens::{
    HEADER_H, ROW_H, border, change_color, fg, fg_muted, fg_strong, icon_color, island_bg,
    row_hover_bg, row_selected_bg,
};
use crate::ui::icons::{Icon, icon};

/// The body of a folder comparison tab.
///
/// `mode` is the tab's *Compare by:* value: changing it walks the two trees
/// again with the other rule, which is what the reference's control does.
pub fn compare_view(
    store: Store,
    comparison: Signal<Option<Comparison>>,
    mode: Signal<CompareMode>,
    compared: Signal<Option<String>>,
) -> impl IntoElement {
    let shown = computed(move || comparison.get());
    let left_root = move || shown.with(|shown| shown.as_ref().map(|c| c.left.clone()));
    let right_root = move || shown.with(|shown| shown.as_ref().map(|c| c.right.clone()));
    let summary = move || {
        shown.with(|shown| {
            shown
                .as_ref()
                .map(|comparison| {
                    format!(
                        "{} only here · {} only there · {} different",
                        comparison.only_left, comparison.only_right, comparison.different
                    )
                })
                .unwrap_or_default()
        })
    };

    view()
        .flex_1()
        .min_h(px(0.0))
        .min_w(px(0.0))
        .w_full()
        .flex_col()
        .bg(move || island_bg())
        .child(
            view()
                .h(px(HEADER_H))
                .w_full()
                .flex_none()
                .flex_row()
                .items_center()
                .gap_3()
                .px_3()
                .child(compare_by(store, comparison, mode))
                .child(
                    text(summary)
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(move || fg_muted()),
                )
                .child(view().flex_1().min_w(px(0.0)))
                .child(
                    text(move || {
                        left_root()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    })
                    .flex_none()
                    .max_w(px(320.0))
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(move || fg_strong()),
                )
                .child(
                    text(move || {
                        right_root()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    })
                    .flex_none()
                    .max_w(px(320.0))
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(move || fg_strong()),
                ),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(column_titles())
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(
            when(
                move || shown.with(|shown| shown.is_some()),
                move || compare_rows(store, comparison, compared),
            )
            .otherwise(|| {
                crate::ui::empty_state(
                    "Reading the folders",
                    "The two trees are being walked; the rows appear when both are read.",
                )
            }),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(compare_preview(comparison, compared))
}

/// The *Compare by:* label and its list, as the reference puts them over the
/// table's left half.
fn compare_by(
    store: Store,
    comparison: Signal<Option<Comparison>>,
    mode: Signal<CompareMode>,
) -> impl IntoElement {
    let items = CompareMode::ALL
        .iter()
        .map(|value| SelectItem::new(value.label(), value.label()))
        .collect::<Vec<_>>();
    view()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            text("Compare by:")
                .flex_none()
                .text_size(px(11.0))
                .text_color(move || fg_muted()),
        )
        .child(
            select()
                .items(items)
                .value(move || Some(SharedString::from(mode.get().label())))
                .on_change(move |next: &Option<SharedString>| {
                    let Some(label) = next.as_ref() else { return };
                    let Some(picked) = CompareMode::from_label(label) else {
                        return;
                    };
                    if picked == mode.get_untracked() {
                        return;
                    }
                    mode.set(picked);
                    state::recompare(store, comparison, mode);
                })
                .accessibility_label("Compare by")
                .offset(px(4.0))
                .popup_style(|s| {
                    s.p_1()
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().popover)
                        .shadow_md()
                })
                .item_class("rounded-md focus:bg-accent selected:font-medium")
                .render_item(|item: &nsh_base::SelectEntry| {
                    view().px_2().py_1p5().child(
                        text(item.title.clone())
                            .text_size(px(12.0))
                            .text_color(move || fg()),
                    )
                })
                .trigger(
                    view()
                        .h(px(22.0))
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
                            text(move || mode.get().label())
                                .flex_none()
                                .text_size(px(11.5))
                                .text_color(move || fg()),
                        )
                        .child(icon(Icon::ChevronDown, 11.0).text_color(move || icon_color())),
                ),
        )
}

/// The header above the rows: what each column holds.
fn column_titles() -> impl IntoElement {
    view()
        .h(px(24.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .child(
            text("Name")
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
        .child(
            text("Size")
                .w(px(72.0))
                .flex_none()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
        .child(
            text("Modified")
                .w(px(120.0))
                .flex_none()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
        .child(
            view()
                .w(px(1.0))
                .h(px(14.0))
                .flex_none()
                .bg(move || border()),
        )
        .child(
            text("Name")
                .flex_1()
                .min_w(px(0.0))
                .pl(px(8.0))
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
        .child(
            text("Size")
                .w(px(72.0))
                .flex_none()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
        .child(
            text("Modified")
                .w(px(120.0))
                .flex_none()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_muted()),
        )
}

/// The rows of the comparison, one per path of either side.
fn compare_rows(
    store: Store,
    comparison: Signal<Option<Comparison>>,
    compared: Signal<Option<String>>,
) -> impl IntoElement {
    let rows = computed(move || {
        comparison.with(|comparison| {
            comparison
                .as_ref()
                .map(|comparison| comparison.rows.clone())
                .unwrap_or_default()
        })
    });
    let roots = computed(move || {
        comparison.with(|comparison| {
            comparison
                .as_ref()
                .map(|comparison| (comparison.left.clone(), comparison.right.clone()))
        })
    });

    virtual_list(
        move || rows.with(Vec::len),
        move |at: Position| compare_row(store, rows, roots, compared, at),
    )
    .estimated_item_size(px(ROW_H))
    .keys(move || rows.with(|rows| rows.iter().map(|row| row.path.clone()).collect::<Vec<_>>()))
    .empty(crate::ui::empty_state(
        "Nothing to compare",
        "Both folders hold the same files, with the same content.",
    ))
    .flex_1()
    .min_h(px(0.0))
}

/// One path's row: its depth, its name, and what each side has at it.
fn compare_row(
    store: Store,
    rows: Memo<Vec<Row>>,
    roots: Memo<Option<(std::path::PathBuf, std::path::PathBuf)>>,
    compared: Signal<Option<String>>,
    at: Position,
) -> impl IntoElement {
    let row = move || rows.with(|rows| rows.get(at.index()).cloned());
    let path = move || row().map(|row| row.path).unwrap_or_default();
    let depth = move || path().matches('/').count() as f32;
    let folder = move || row().is_some_and(|row| row.holds_folder());
    let name = move || row().map(|row| row.name().to_owned()).unwrap_or_default();
    let tint = move || {
        row().and_then(|row| match row.only() {
            Some(Side::Left) => Some(change_color(git_core::ChangeKind::Deleted)),
            Some(Side::Right) => Some(change_color(git_core::ChangeKind::Added)),
            None if row.differs => Some(change_color(git_core::ChangeKind::Modified)),
            None => None,
        })
    };
    let left_size = move || size_of(row().and_then(|row| row.left));
    let left_date = move || date_of(row().and_then(|row| row.left));
    let right_size = move || size_of(row().and_then(|row| row.right));
    let right_date = move || date_of(row().and_then(|row| row.right));
    let has_left = move || row().is_some_and(|row| row.left.is_some());
    let has_right = move || row().is_some_and(|row| row.right.is_some());
    let selected = move || state::is_compared(compared, &path());

    let press_path = path;
    view()
        .h(px(ROW_H))
        .w_full()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .cursor_pointer()
        .hover(|s| s.bg(move || row_hover_bg()))
        .bg(move || {
            if selected() {
                row_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .on_press(move |press: &PressEvent| {
            let pressed = press_path();
            state::select_compared(compared, pressed.clone());
            // The second press of a double press opens the file as a tab of
            // its own; the first only selects it, which is what the pane
            // below the table reads.
            if press.clicks == 2 && !folder() {
                let Some((left, right)) = roots.get_untracked() else {
                    return;
                };
                state::open_compared_file_tab(store, left, right, pressed);
            }
        })
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap_1()
                .pl(px(14.0 * depth()))
                .child(if folder() {
                    icon(Icon::Folder, 14.0)
                        .text_color(move || icon_color())
                        .into_element()
                } else {
                    crate::ui::icons::file_icon(&name(), 14.0).into_element()
                })
                .child(
                    text(name)
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(12.0))
                        .text_color(move || tint().unwrap_or_else(fg)),
                ),
        )
        .child(side_cell(left_size, left_date, has_left, tint))
        .child(
            view()
                .w(px(1.0))
                .h(px(14.0))
                .flex_none()
                .bg(move || border()),
        )
        .child(
            view().flex_1().min_w(px(0.0)).pl(px(8.0)).child(
                text(move || if has_right() { name() } else { String::new() })
                    .truncate()
                    .text_size(px(12.0))
                    .text_color(move || tint().unwrap_or_else(fg)),
            ),
        )
        .child(side_cell(right_size, right_date, has_right, tint))
}

/// The size and the write time of one side of a row.
fn side_cell(
    size: impl Fn() -> String + 'static + Copy,
    date: impl Fn() -> String + 'static + Copy,
    present: impl Fn() -> bool + 'static + Copy,
    tint: impl Fn() -> Option<AdaptiveColor> + 'static + Copy,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .child(
            text(size)
                .w(px(72.0))
                .flex_none()
                .text_size(px(11.0))
                .text_color(move || tint().unwrap_or_else(fg_muted)),
        )
        .child(
            text(date)
                .w(px(120.0))
                .flex_none()
                .text_size(px(11.0))
                .text_color(move || tint().unwrap_or_else(fg_muted)),
        )
        .child(when(move || !present(), || view().w(px(0.0))))
}

/// The pane under the table: the diff of the row the reader picked, which is
/// how the reference shows "and here is what changed in it" without leaving
/// the two folders.
fn compare_preview(
    comparison: Signal<Option<Comparison>>,
    compared: Signal<Option<String>>,
) -> impl IntoElement {
    // The patch is read when the selection changes, not on every frame: the
    // two files are read from disk, which is not something to do while
    // drawing.
    let patch = signal(String::new());
    let shown = signal(Option::<String>::None);
    effect(move || {
        let Some(path) = compared.get() else {
            patch.set(String::new());
            shown.set(None);
            return;
        };
        let Some((left, right)) = comparison.with(|comparison| {
            comparison
                .as_ref()
                .map(|comparison| (comparison.left.clone(), comparison.right.clone()))
        }) else {
            return;
        };
        if shown.get_untracked().as_deref() == Some(path.as_str()) {
            return;
        }
        shown.set(Some(path.clone()));
        let (for_left, for_right, for_path) = (left, right, path.clone());
        crate::state::off_thread(
            move || {
                let one = for_left.join(&for_path);
                let other = for_right.join(&for_path);
                let name = for_path.rsplit('/').next().unwrap_or(&for_path).to_owned();
                crate::compare::diff_patch(
                    one.exists().then_some(one).as_deref(),
                    other.exists().then_some(other).as_deref(),
                    &name,
                )
            },
            move |text| patch.set(text),
        );
    });

    let name = move || compared.with(|path| path.clone()).unwrap_or_default();
    view()
        .flex_1()
        .min_h(px(120.0))
        .w_full()
        .flex_col()
        .child(
            view()
                .h(px(24.0))
                .w_full()
                .flex_none()
                .flex_row()
                .items_center()
                .gap_2()
                .px_3()
                .child(
                    text(move || {
                        if name().is_empty() {
                            "Select a file to see its changes".to_owned()
                        } else {
                            name()
                        }
                    })
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(move || fg_muted()),
                ),
        )
        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
        .child(
            view()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .overflow_hidden()
                .child(
                    nsh_base::diff_view()
                        .patch(move || Some(SharedString::from(patch.get())))
                        .language("")
                        .layout(nsh_base::DiffLayout::Split)
                        .context_lines(3)
                        .font_size(px(12.0))
                        .line_height(px(19.0))
                        .w_full()
                        .h_full(),
                ),
        )
}

/// A side's length, as a file manager shows it.
fn size_of(entry: Option<crate::compare::Entry>) -> String {
    match entry {
        None => String::new(),
        Some(entry) if entry.is_dir => "<dir>".to_owned(),
        Some(entry) => human_size(entry.size),
    }
}

/// A byte count in the units a file manager uses.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// A side's write time, as the comparison's columns show it.
fn date_of(entry: Option<crate::compare::Entry>) -> String {
    match entry {
        None => String::new(),
        Some(entry) => git_core::format_timestamp(entry.modified),
    }
}
