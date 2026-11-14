//! The window's frame: the title bar, the tool window stripes, the islands,
//! and the status bar.
//!
//! The reference IDE draws its window as *islands*: one flat frame colour
//! (`--bg-frame`) with rounded, slightly darker surfaces floating on it,
//! 6pt apart. Top to bottom the window is a 40pt title bar, a row holding the
//! activity stripe, the Project island and the Git tool window island, and a
//! status bar. The Git island has a tab strip of its own, and under it the
//! panes.
//!
//! Nothing here names a colour: every one comes from
//! [`crate::theme_tokens`], which reads the theme's tokens.

use nsh_base::{theme, tooltip};
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{
    BAR_H, HEADER_H, ISLAND_GAP, ISLAND_RADIUS, STATUS_H, STRIPE_BUTTON, TITLEBAR_H, border, fg,
    fg_muted, fg_strong, frame_bg, icon_color, island_bg, row_hover_bg, tab_selected_bg,
    tab_selected_border, toolbar_hover_bg, toolbar_selected_bg,
};
use crate::ui::icons::{Icon, icon};

/// An island: a rounded surface on the frame.
pub fn island() -> nsh_core::element::View {
    view()
        .flex_col()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .rounded(px(ISLAND_RADIUS))
        .bg(move || island_bg())
}

/// The gap between two islands stacked.
pub fn gap_v() -> impl IntoElement {
    view().h(px(ISLAND_GAP)).w_full().flex_none()
}

/// The title bar: the app mark, the main menu, the project and branch
/// widgets, the VCS actions, and the search and settings at the right, with
/// the system's caption buttons left their room.
pub fn title_bar(store: Store) -> impl IntoElement {
    view()
        .h(px(TITLEBAR_H))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .pl(px(10.0))
        .bg(move || frame_bg())
        .window_drag(true)
        .child(app_mark())
        .child(view().w(px(6.0)))
        .child(stripe_button(Icon::Menu, "Main menu", false, || {}))
        .child(view().w(px(6.0)))
        .child(project_widget(store))
        .child(view().w(px(4.0)))
        .child(branch_widget(store))
        .child(view().w(px(4.0)))
        .child(title_action(Icon::Pull, "Update project", move || {
            state::open_update_dialog(store)
        }))
        .child(title_action(Icon::Push, "Push", move || {
            state::open_push(store)
        }))
        .child(view().flex_1())
        .child(title_action(Icon::Search, "Search everywhere", || {}))
        .child(title_action(Icon::Settings, "Settings", || {}))
        // The platform draws the caption buttons here.
        .child(view().w(px(150.0)).flex_none())
}

/// The app's own mark at the window's left end, as the reference puts the
/// IDE's icon before the menu.
fn app_mark() -> impl IntoElement {
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
        )
}

/// The project widget: the project's avatar — its initials on their colour —
/// its name, and the chevron over the project menu it opens.
fn project_widget(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let name = move || {
        snapshot.with(|snapshot| {
            if snapshot.name.is_empty() {
                "Open a project".to_owned()
            } else {
                snapshot.name.clone()
            }
        })
    };
    let initials = move || crate::theme_tokens::project_initials(&name());
    let face = view()
        .h(px(30.0))
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .rounded(px(8.0))
        .cursor_pointer()
        .window_drag(false)
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .child(
            view()
                .size_5()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg(move || crate::theme_tokens::project_avatar_color(&name()))
                .child(
                    text(initials)
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(move || theme().primary_foreground),
                ),
        )
        .child(
            text(name)
                .max_w(px(220.0))
                .truncate()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || fg_strong()),
        )
        .child(small_chevron());

    crate::ui::menus::project_popup(store, face)
}

/// The branch widget: the current branch, its arrow, and the sync counts.
/// Pressing it opens the branch popup, which holds every branch action the
/// reference's branch button offers.
fn branch_widget(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let face = view()
        .h(px(30.0))
        .flex_row()
        .items_center()
        .gap_1p5()
        .px_2()
        .rounded(px(8.0))
        .cursor_pointer()
        .window_drag(false)
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .child(icon(Icon::Branch, 16.0).text_color(move || fg_muted()))
        .child(
            text(move || {
                snapshot.with(|snapshot| {
                    if snapshot.is_open() {
                        snapshot.branch.label()
                    } else {
                        "No repository".to_owned()
                    }
                })
            })
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(move || fg_strong()),
        )
        .child(
            text(move || {
                let branch = snapshot.with(|snapshot| snapshot.branch.clone());
                if branch.ahead == 0 && branch.behind == 0 {
                    String::new()
                } else {
                    format!("↑{} ↓{}", branch.ahead, branch.behind)
                }
            })
            .text_size(px(11.0))
            .text_color(move || fg_muted()),
        )
        .child(small_chevron());

    crate::ui::menus::branch_popup(store, face)
}

/// The little chevron a title bar widget ends with.
fn small_chevron() -> impl IntoElement {
    icon(Icon::ChevronDown, 12.0).text_color(move || fg_muted())
}

/// One icon button of the title bar's action group. Its hover text is the
/// label itself, as the reference's title bar buttons say what they do when
/// the pointer rests on them.
fn title_action(glyph: Icon, label: &'static str, press: impl Fn() + 'static) -> impl IntoElement {
    view()
        .size(px(30.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .cursor_pointer()
        .window_drag(false)
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .on_press(press)
        .accessible(true)
        .label(label)
        .child(icon(glyph, 16.0).text_color(move || icon_color()))
        .tooltip(tooltip().child(hover_bubble(label)))
}

/// The bubble a button's hover shows: the label on the popover's look.
pub(crate) fn hover_bubble(label: &'static str) -> impl IntoElement {
    view()
        .px_2()
        .py_1()
        .rounded(px(5.0))
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().popover)
        .shadow_md()
        .child(text(label).text_size(px(11.0)).text_color(move || fg()))
}

/// The activity stripe down the window's left edge: the sections of the
/// sidebar at the top, and the tools — the Git area's toggle, the terminal,
/// the settings — at the bottom.
pub fn stripe(store: Store) -> impl IntoElement {
    view()
        .w(px(STRIPE_BUTTON + 8.0))
        .h_full()
        .flex_none()
        .flex_col()
        .items_center()
        .gap_0p5()
        .pt(px(4.0))
        .pb(px(6.0))
        .bg(move || frame_bg())
        .child(section_stripe_button(
            store,
            Icon::Project,
            "Project",
            state::Side::Project,
        ))
        .child(section_stripe_button(
            store,
            Icon::Commit,
            "Commit",
            state::Side::Commit,
        ))
        .child(stripe_separator())
        .child(stripe_button(Icon::More, "More tool windows", false, || {}))
        .child(view().flex_1())
        .child(git_stripe_button(store))
        .child(stripe_button(Icon::Terminal, "Terminal", false, || {}))
        .child(stripe_button(Icon::Settings, "Settings", false, || {}))
        .child(
            view()
                .size(px(STRIPE_BUTTON))
                .flex_none()
                .items_center()
                .justify_center()
                .child(
                    text("YG")
                        .text_size(px(10.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || fg_muted()),
                ),
        )
}

/// The stripe's Git button: it shows and hides the Git area, and stays lit
/// while the area is up.
fn git_stripe_button(store: Store) -> impl IntoElement {
    let open = store.git_area;
    stripe_face(
        Icon::Git,
        "Git",
        move || open.get(),
        move || open.update(|open| *open = !*open),
    )
}

/// One stripe button that switches the sidebar's section.
fn section_stripe_button(
    store: Store,
    glyph: Icon,
    label: &'static str,
    which: state::Side,
) -> impl IntoElement {
    let side = store.side;
    let active = move || side.get() == which;
    stripe_face(glyph, label, active, move || side.set(which))
}

/// One button of the stripe: a rounded square that lights up when its tool
/// window is the active one.
fn stripe_button(
    glyph: Icon,
    label: &'static str,
    active: bool,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    stripe_face(glyph, label, move || active, press)
}

/// The look and the press of a stripe button.
fn stripe_face(
    glyph: Icon,
    label: &'static str,
    active: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    let lit = std::rc::Rc::new(active);
    let (for_bg, for_icon) = (lit.clone(), lit);
    view()
        .size(px(STRIPE_BUTTON))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .bg(move || {
            if for_bg() {
                toolbar_selected_bg()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .on_press(press)
        .accessible(true)
        .label(label)
        .child(icon(glyph, 16.0).text_color(move || {
            if for_icon() {
                theme().foreground
            } else {
                icon_color()
            }
        }))
}

/// The short rule between two groups of stripe buttons.
fn stripe_separator() -> impl IntoElement {
    view()
        .w(px(16.0))
        .h(px(1.0))
        .flex_none()
        .my(px(3.0))
        .bg(move || border())
}

/// A tool window island's header: the tabs it holds, and the actions at its
/// right end.
///
/// The Git area's header names the log and the branch it walks, which is what
/// the reference prints there: `Log: HEAD`.
pub fn log_header(store: Store) -> impl IntoElement {
    let reference = store.reference;
    let snapshot = store.snapshot;
    view()
        .h(px(HEADER_H))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .px(px(6.0))
        .child(
            view()
                .h(px(28.0))
                .flex_none()
                .flex_row()
                .items_center()
                .gap_1p5()
                .px(px(10.0))
                .rounded(px(6.0))
                .bg(move || tab_selected_bg())
                .border_1()
                .border_color(move || tab_selected_border())
                .child(icon(Icon::Branch, 14.0).text_color(move || fg()))
                .child(
                    text(move || {
                        let walked = reference.with(|name| {
                            name.as_ref().map(|full| {
                                full.strip_prefix("refs/heads/")
                                    .or_else(|| full.strip_prefix("refs/remotes/"))
                                    .unwrap_or(full)
                                    .to_owned()
                            })
                        });
                        let walked = walked
                            .unwrap_or_else(|| snapshot.with(|snapshot| snapshot.branch.label()));
                        format!("Log: {walked}")
                    })
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || fg()),
                ),
        )
        .child(view().flex_1())
        .child(header_action(Icon::More, "Tool window options", || {}))
        .child(header_action(
            Icon::Close,
            "Hide the tool window",
            move || store.git_area.set(false),
        ))
}

/// A small square button in a header or a toolbar.
pub fn header_action(
    glyph: Icon,
    label: &'static str,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    view()
        .size(px(26.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .on_press(press)
        .accessible(true)
        .label(label)
        .child(icon(glyph, 16.0).text_color(move || icon_color()))
}

/// A button of a pane's own toolbar: an icon, and a tooltip.
pub fn tool_icon(glyph: Icon, label: &'static str, press: impl Fn() + 'static) -> impl IntoElement {
    header_action(glyph, label, press)
}

/// The row a list pane puts above its rows: a search field, filters, actions.
pub fn bar() -> nsh_core::element::View {
    view()
        .h(px(BAR_H))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .px(px(6.0))
}

/// A search field as the reference draws it: a rounded input with the
/// magnifier inside it.
pub fn search_field(
    placeholder: &'static str,
    value: impl Fn() -> String + 'static,
    change: impl Fn(&str) + 'static,
) -> impl IntoElement {
    view()
        .h(px(26.0))
        .flex_1()
        .min_w(px(60.0))
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
                .placeholder(placeholder)
                .value(value)
                .on_change(change)
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// The status bar along the bottom.
pub fn status_bar(store: Store) -> impl IntoElement {
    let snapshot = store.snapshot;
    let toast = store.toast;
    let busy = store.busy;

    view()
        .h(px(STATUS_H))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_1()
        .pl(px(12.0))
        .pr(px(10.0))
        .bg(move || frame_bg())
        .child(
            view()
                .h(px(22.0))
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_1()
                .rounded(px(4.0))
                .child(icon(Icon::Branch, 12.0).text_color(move || icon_color()))
                .child(
                    text(move || {
                        snapshot.with(|snapshot| {
                            if snapshot.is_open() {
                                snapshot.branch.label()
                            } else {
                                "No repository".to_owned()
                            }
                        })
                    })
                    .text_size(px(12.0))
                    .text_color(move || fg_muted()),
                )
                .child(
                    text(move || {
                        let branch = snapshot.with(|snapshot| snapshot.branch.clone());
                        if branch.ahead == 0 && branch.behind == 0 {
                            String::new()
                        } else {
                            format!("↑{} ↓{}", branch.ahead, branch.behind)
                        }
                    })
                    .text_size(px(11.0))
                    .text_color(move || fg_muted()),
                ),
        )
        .child(view().flex_1())
        .child(
            when(
                move || busy.get(),
                move || {
                    view()
                        .size_2()
                        .rounded(px(4.0))
                        .bg(move || theme().primary)
                        .animate_pulse()
                },
            )
            .otherwise(|| view().size_2()),
        )
        .child(
            text(move || {
                snapshot.with(|snapshot| {
                    let (staged, unstaged) = (
                        snapshot.status.staged_count(),
                        snapshot.status.unstaged_count(),
                    );
                    if staged + unstaged == 0 {
                        String::new()
                    } else {
                        format!("{staged} staged · {unstaged} changed")
                    }
                })
            })
            .text_size(px(12.0))
            .text_color(move || fg_muted()),
        )
        .child(
            when(
                move || toast.get().is_some(),
                move || {
                    view()
                        .h(px(20.0))
                        .px_2()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .bg(move || theme().accent)
                        .child(
                            text(move || toast.get().unwrap_or_default())
                                .text_size(px(11.0))
                                .text_color(move || fg()),
                        )
                },
            )
            .otherwise(|| view().h(px(0.0))),
        )
}

/// The chevron a tree row starts with: a triangle pointing at an expanded
/// node, or at a collapsed one.
pub fn tree_chevron(expanded: bool, show: bool) -> impl IntoElement {
    if !show {
        return view().w(px(12.0)).flex_none();
    }
    view()
        .w(px(12.0))
        .flex_none()
        .items_center()
        .justify_center()
        .child(
            icon(
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                12.0,
            )
            .text_color(move || icon_color()),
        )
}

/// The pane a screen with no repository shows: a way in, and the folders the
/// window remembers.
pub fn welcome_screen(store: Store) -> impl IntoElement {
    let recents = crate::model::recent_paths();
    let any_recent = !recents.is_empty();
    view()
        .flex_1()
        .w_full()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .p_8()
        .bg(move || frame_bg())
        .child(
            text("Grove")
                .text_size(px(28.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || fg_strong()),
        )
        .child(
            text("A Git client that reads fast and writes through git.")
                .text_size(px(13.0))
                .text_color(move || fg_muted()),
        )
        .child(crate::ui::primary_button(
            "Open a repository",
            || true,
            move || state::pick_folder(store),
        ))
        .child(when(
            move || any_recent,
            move || {
                view()
                    .flex_col()
                    .w(px(460.0))
                    .gap_0p5()
                    .child(
                        text("Recent")
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(move || fg_muted()),
                    )
                    .children(recents.iter().cloned().map(move |path| {
                        let shown = path.display().to_string();
                        let target = path.clone();
                        view()
                            .h(px(28.0))
                            .w_full()
                            .flex_row()
                            .items_center()
                            .px_2()
                            .rounded(px(5.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(move || row_hover_bg()))
                            .on_press(move |_: &()| state::open_folder(store, target.clone()))
                            .child(
                                text(shown)
                                    .text_size(px(12.0))
                                    .truncate()
                                    .text_color(move || fg()),
                            )
                    }))
            },
        ))
}
