//! The Update Project dialog: how the pull lands, and the ways out.
//!
//! The reference asks before every update: merge the incoming commits in, or
//! rebase the branch on top of them. Its footer also offers to stop asking —
//! once taken, the choice repeats without the dialog.

use nsh_base::{checkbox, radio, theme};
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{border, fg, fg_muted, fg_strong, icon_color};
use crate::ui::icons::{Icon, icon};

/// The dialog, while it is open.
pub fn update_dialog(store: Store) -> impl IntoElement {
    let open = store.update_open;
    when(
        move || open.get(),
        move || {
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(500.0))
                        .flex_col()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .overflow_hidden()
                        .child(update_title(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(update_body(store))
                        .child(view().h(px(1.0)).w_full().flex_none().bg(move || border()))
                        .child(update_footer(store)),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}

/// The dialog's title: the app mark, the dialog's name, and the cross that
/// closes it.
fn update_title(store: Store) -> impl IntoElement {
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
            text("Update Project")
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
        .on_press(move |_: &()| state::dismiss_update(store))
        .accessible(true)
        .label("Close")
        .child(icon(Icon::Close, 14.0).text_color(move || icon_color()))
}

/// The body: the two ways the incoming commits can land.
fn update_body(store: Store) -> impl IntoElement {
    view()
        .w_full()
        .flex_none()
        .flex_col()
        .gap_2()
        .p_4()
        .child(update_mode_radio(
            store,
            false,
            "Merge incoming changes into the current branch",
        ))
        .child(update_mode_radio(
            store,
            true,
            "Rebase the current branch on top of incoming changes",
        ))
}

/// One way to update, with the circle the reference draws for it.
fn update_mode_radio(store: Store, rebase: bool, label: &'static str) -> impl IntoElement {
    let checked = store.update_rebase;
    radio()
        .checked(move || checked.get() == rebase)
        .on_change(move |_: &bool| checked.set(rebase))
        .accessibility_label(label)
        .child(
            view()
                .h(px(28.0))
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    view()
                        .size(px(16.0))
                        .rounded_full()
                        .border_1()
                        .border_color(move || {
                            if checked.get() == rebase {
                                theme().primary
                            } else {
                                // The theme's border sinks into the popover;
                                // the muted text tone keeps the circle seen.
                                fg_muted()
                            }
                        })
                        .items_center()
                        .justify_center()
                        // The dot shows only on the chosen one: an empty
                        // circle with a dot in it reads as chosen anyway.
                        .child(view().size(px(7.0)).rounded_full().bg(move || {
                            if checked.get() == rebase {
                                theme().primary
                            } else {
                                AdaptiveColor::TRANSPARENT
                            }
                        })),
                )
                .child(text(label).text_size(px(12.5)).text_color(move || fg())),
        )
}

/// The footer: the ways out, and the offer to stop being asked.
fn update_footer(store: Store) -> impl IntoElement {
    let forget = store.update_forget;
    view()
        .h(px(52.0))
        .w_full()
        .flex_none()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .child(
            view()
                .size(px(22.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(move || border())
                .child(text("?").text_size(px(11.0)).text_color(move || fg_muted())),
        )
        .child(
            checkbox()
                .checked(move || forget.get())
                .accessibility_label("Don't show again")
                .size(px(14.0))
                .on_change(move |next: &bool| forget.set(*next))
                .indicator(
                    view()
                        .size(px(14.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(move || theme().border)
                        .items_center()
                        .justify_center()
                        .child(icon(Icon::Check, 10.0).text_color(move || {
                            if forget.get() {
                                theme().primary_foreground
                            } else {
                                AdaptiveColor::TRANSPARENT
                            }
                        })),
                )
                .bg(move || {
                    if forget.get() {
                        theme().primary
                    } else {
                        AdaptiveColor::TRANSPARENT
                    }
                }),
        )
        .child(
            text("Don't show again")
                .text_size(px(12.0))
                .text_color(move || fg()),
        )
        .child(view().flex_1())
        .child(secondary_button("Reset to the Remote Branch", move || {
            state::reset_to_remote(store)
        }))
        .child(crate::ui::primary_button(
            "OK",
            || true,
            move || state::confirm_update(store),
        ))
        .child(cancel_button(store))
}

/// A framed push-button of the footer, lighter than the primary one.
fn secondary_button(label: &'static str, press: impl Fn() + 'static) -> impl IntoElement {
    view()
        .h(px(28.0))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(move || theme().input)
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(press)
        .accessible(true)
        .label(label)
        .child(text(label).text_size(px(12.0)).text_color(move || fg()))
}

/// The dialog's Cancel.
fn cancel_button(store: Store) -> impl IntoElement {
    view()
        .h(px(28.0))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || theme().accent))
        .on_press(move |_: &()| state::dismiss_update(store))
        .accessible(true)
        .label("Cancel")
        .child(
            text("Cancel")
                .text_size(px(12.0))
                .text_color(move || fg_muted()),
        )
}
