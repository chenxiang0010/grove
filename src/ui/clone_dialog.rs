//! The Clone Repository dialog: one repository URL panel, as the reference's
//! clone dialog has, and the fields that take a URL, a directory and a
//! shallow depth.
//!
//! The reference offers GitHub, GitHub Enterprise and GitLab beside the URL
//! panel; Grove has no accounts, so the list holds the URL panel alone. A
//! clone runs the `git` executable on a worker — hooks and filters behave as
//! on the command line — and opens what it cloned when it succeeds.

use nsh_base::{SelectItem, checkbox, input, progress, select, theme};
use nsh_core::prelude::*;

use crate::model;
use crate::state::{self, Store};
use crate::theme_tokens::{border, fg, fg_muted, fg_strong, icon_color};
use crate::ui::icons::{Icon, icon};

/// The width the field labels end at: every label is right-aligned to this
/// column, as the reference's form is.
const LABEL_WIDTH: f32 = 110.0;

/// The dialog, while it is open.
pub fn clone_dialog(store: Store) -> impl IntoElement {
    let open = store.clone_open;
    when(
        move || open.get(),
        move || {
            let running = store.clone_running;
            let url = store.clone_url;
            let into = store.clone_into;
            let shallow = store.clone_shallow;
            let depth = store.clone_depth;
            // Tracked reads: the Clone button's disabled state has to follow
            // the form and the running flag as they change.
            let enabled = move || {
                !running.get()
                    && model::validate_clone(&url.get(), &into.get(), shallow.get(), &depth.get())
                        .is_none()
            };
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(760.0))
                        .flex_col()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .overflow_hidden()
                        .child(clone_title(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(clone_body(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(
                            view()
                                .h(px(44.0))
                                .w_full()
                                .flex_none()
                                .flex_row()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .px_3()
                                .child(cancel_button(store))
                                .child(crate::ui::primary_button("Clone", enabled, move || {
                                    state::start_clone(store)
                                })),
                        ),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}

/// The dialog's title: the app mark, the dialog's name, and the cross that
/// closes it.
fn clone_title(store: Store) -> impl IntoElement {
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
            text("Clone Repository")
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
        .on_press(move |_: &()| state::dismiss_clone(store))
        .accessible(true)
        .label("Close")
        .child(icon(Icon::Close, 14.0).text_color(move || icon_color()))
}

/// The body: the one panel on the left, the fields on the right.
fn clone_body(store: Store) -> impl IntoElement {
    view()
        .h(px(240.0))
        .w_full()
        .flex_none()
        .flex_row()
        .child(
            // The reference lists GitHub and GitLab here as well; Grove has
            // no accounts, so the URL panel is the only one.
            view()
                .w(px(175.0))
                .flex_none()
                .flex_col()
                .p_2()
                .border_r_1()
                .border_color(move || border())
                .bg(move || theme().background)
                .accessible(true)
                .label("Clone source")
                .child(
                    view()
                        .h(px(38.0))
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .rounded(px(5.0))
                        .bg(move || theme().selection())
                        .child(icon(Icon::Branch, 16.0).text_color(move || fg_strong()))
                        .child(
                            text("Repository URL")
                                .text_size(px(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || fg_strong()),
                        ),
                ),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap_3()
                .p_4()
                .child(version_control_row(store))
                .child(url_row(store))
                .child(into_row(store))
                .child(shallow_row(store))
                .child(clone_status(store)),
        )
}

/// The panel's first row: the version control, and the placeholder for the
/// other providers.
fn version_control_row(store: Store) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            view().w(px(LABEL_WIDTH)).flex_none().justify_end().child(
                text("Version control:")
                    .text_size(px(12.0))
                    .text_color(move || fg()),
            ),
        )
        .child(
            select()
                .items(vec![SelectItem::new("Git", "Git")])
                .value(|| Some(SharedString::from("Git")))
                .on_change(|_: &Option<SharedString>| {})
                .accessibility_label("Version control")
                .offset(px(4.0))
                .popup_style(crate::ui::popup_style)
                .trigger(
                    view()
                        .h(px(24.0))
                        .px_2()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(move || theme().input)
                        .bg(move || theme().background)
                        .cursor_pointer()
                        .child(text("Git").text_size(px(11.5)).text_color(move || fg()))
                        .child(icon(Icon::ChevronDown, 11.0).text_color(move || icon_color())),
                ),
        )
        .child(
            // The reference's link to its other clone sources; Grove has
            // none, and says so rather than pretending.
            view()
                .cursor_pointer()
                .rounded(px(4.0))
                .px_1()
                .hover(|s| s.opacity(0.8))
                .on_press(move |_: &()| store.say("No other version control providers"))
                .child(
                    text("More via plugins…")
                        .text_size(px(12.0))
                        .text_color(move || theme().primary),
                ),
        )
}

/// One labelled row: the label right-aligned at the left end, its field
/// filling the rest.
fn field_row(label: &'static str, field: impl IntoElement) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            view()
                .w(px(LABEL_WIDTH))
                .flex_none()
                .justify_end()
                .child(text(label).text_size(px(12.0)).text_color(move || fg())),
        )
        .child(field)
}

/// The URL the dialog clones, with the field focused as the dialog opens and
/// the recent URLs behind a chevron at its end.
fn url_row(store: Store) -> impl IntoElement {
    let url = store.clone_url;
    field_row(
        "URL:",
        view()
            .h(px(30.0))
            .flex_1()
            .min_w(px(0.0))
            .flex_row()
            .items_center()
            .px_2()
            .rounded(px(5.0))
            .border_1()
            .border_color(move || theme().input)
            .bg(move || theme().background)
            .child(
                input()
                    .value(move || url.get())
                    .on_change(move |text: &str| state::set_clone_url(store, text))
                    .auto_focus(true)
                    .placeholder("https://github.com/...")
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .text_color(move || fg()),
            )
            .child(url_history(store)),
    )
}

/// The recent repository URLs, behind the URL field's chevron: what the
/// reference's URL combo offers. The chevron sits dark while the history is
/// empty.
fn url_history(store: Store) -> impl IntoElement {
    let recent = model::recent_clone_urls();
    if recent.is_empty() {
        return view()
            .size(px(24.0))
            .flex_none()
            .items_center()
            .justify_center()
            .child(icon(Icon::ChevronDown, 11.0).text_color(move || icon_color()))
            .opacity(0.4)
            .into_element();
    }
    select()
        .items(
            recent
                .iter()
                .map(|url| SelectItem::new(url.clone(), url.clone()))
                .collect::<Vec<_>>(),
        )
        .on_change(move |value: &Option<SharedString>| {
            if let Some(url) = value {
                state::set_clone_url(store, url);
            }
        })
        .accessibility_label("Recent repository URLs")
        .offset(px(4.0))
        .popup_style(crate::ui::popup_style)
        .trigger(
            view()
                .size(px(24.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(move || theme().accent))
                .child(icon(Icon::ChevronDown, 11.0).text_color(move || icon_color())),
        )
        .into_element()
}

/// The directory clones go into, and the cross it is browsed with.
fn into_row(store: Store) -> impl IntoElement {
    let into = store.clone_into;
    field_row(
        "Clone into:",
        view()
            .flex_1()
            .min_w(px(0.0))
            .flex_row()
            .items_center()
            .gap_1()
            .child(
                view()
                    .h(px(30.0))
                    .flex_1()
                    .min_w(px(0.0))
                    .flex_row()
                    .items_center()
                    .px_2()
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(move || theme().input)
                    .bg(move || theme().background)
                    .child(
                        input()
                            .value(move || into.get())
                            .on_change(move |text: &str| state::set_clone_into(store, text))
                            .flex_1()
                            .min_w(px(0.0))
                            .text_size(px(12.0))
                            .text_color(move || fg()),
                    ),
            )
            .child(
                view()
                    .size(px(30.0))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.0))
                    .border_1()
                    .border_color(move || theme().input)
                    .cursor_pointer()
                    .hover(|s| s.bg(move || theme().accent))
                    .on_press(move |_: &()| state::browse_clone_into(store))
                    .accessible(true)
                    .label("Choose a directory")
                    .child(icon(Icon::FolderOpen, 14.0).text_color(move || icon_color())),
            ),
    )
}

/// The shallow-clone row: a checkbox, its sentence, and the depth it
/// truncates the history to — the depth dark and deaf while the box is
/// unchecked, as the reference's is.
fn shallow_row(store: Store) -> impl IntoElement {
    let shallow = store.clone_shallow;
    let depth = store.clone_depth;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(view().w(px(LABEL_WIDTH + 8.0)).flex_none())
        .child(
            checkbox()
                .checked(move || shallow.get())
                .accessibility_label("Shallow clone")
                .size(px(14.0))
                .on_change(move |next: &bool| shallow.set(*next))
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
                            if shallow.get() {
                                theme().primary_foreground
                            } else {
                                AdaptiveColor::TRANSPARENT
                            }
                        })),
                )
                .bg(move || {
                    if shallow.get() {
                        theme().primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }),
        )
        .child(
            view()
                .cursor_pointer()
                .on_press(move |_: &()| shallow.set(!shallow.get_untracked()))
                .child(
                    text("Shallow clone with a history truncated to")
                        .flex_none()
                        .text_size(px(12.0))
                        .text_color(move || fg()),
                ),
        )
        .child(
            view()
                .w(px(40.0))
                .h(px(24.0))
                .flex_none()
                .flex_row()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .border_1()
                .border_color(move || theme().input)
                .bg(move || theme().background)
                .style_when(move || !shallow.get(), |s| s.opacity(0.45))
                .child(
                    input()
                        .value(move || depth.get())
                        .on_change(move |text: &str| {
                            // Unchecked, the depth is not the clone's: the
                            // field shows its value but takes no edits.
                            if shallow.get_untracked() {
                                depth.set(text.to_owned());
                            }
                        })
                        .flex_1()
                        .min_w(px(0.0))
                        .text_center()
                        .text_size(px(12.0))
                        .text_color(move || fg()),
                ),
        )
        .child(
            text("commits")
                .flex_none()
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
}

/// Under the fields: the clone's progress while it runs, why it failed once
/// it stopped, or — before it starts — the first problem with the form.
fn clone_status(store: Store) -> impl IntoElement {
    let status = store.clone_status;
    let running = store.clone_running;
    let url = store.clone_url;
    let into = store.clone_into;
    let shallow = store.clone_shallow;
    let depth = store.clone_depth;
    view()
        .flex_1()
        .min_h(px(0.0))
        .flex_col()
        .justify_start()
        .gap_1()
        .child(dynamic(move || {
            if running.get() {
                // The bar is indeterminate: git's progress lines are not
                // parsed into a fraction.
                return view()
                    .flex_col()
                    .gap_1()
                    .child(
                        progress()
                            .indeterminate(true)
                            .accessibility_label("Clone progress")
                            .track_style(move |s| {
                                s.h(px(3.0))
                                    .w_full()
                                    .rounded(px(1.5))
                                    .bg(move || theme().accent)
                            })
                            .indicator_style(move |s| {
                                s.h(px(3.0)).rounded(px(1.5)).bg(move || theme().primary)
                            }),
                    )
                    .child(
                        text(status.get())
                            .truncate()
                            .text_size(px(11.0))
                            .text_color(move || fg_muted()),
                    )
                    .into_element();
            }
            if !status.get().is_empty() {
                return text(status.get())
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(move || theme().destructive)
                    .into_element();
            }
            // The form's problems show once there is a URL to talk about,
            // as the reference's do.
            if !url.get().trim().is_empty()
                && let Some(problem) =
                    model::validate_clone(&url.get(), &into.get(), shallow.get(), &depth.get())
            {
                return text(problem)
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(move || fg_muted())
                    .into_element();
            }
            view().h(px(0.0)).into_element()
        }))
}

/// The dialog's Cancel.
fn cancel_button(store: Store) -> impl IntoElement {
    view()
        .h(px(26.0))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| state::dismiss_clone(store))
        .accessible(true)
        .label("Cancel")
        .child(
            text("Cancel")
                .text_size(px(12.0))
                .text_color(move || fg_muted()),
        )
}
