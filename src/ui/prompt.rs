//! The one modal Grove asks a name in: a new branch, a new tag, a rename.
//!
//! A Git action that needs a name goes through this instead of a dialog of
//! its own, so the window has one place that asks, one place that takes
//! Return, and one place that Escape closes.
//!
//! The overlay exists only while a prompt is open: a full-window transparent
//! layer would take every press the rest of the window wants, menus and rows
//! included.

use nsh_base::{button, input, theme};
use nsh_core::prelude::*;

use crate::state::{self, Store};
use crate::theme_tokens::{border, fg, fg_muted, fg_strong};

/// The dialog, while one is open.
pub fn prompt_dialog(store: Store) -> impl IntoElement {
    let prompt = store.prompt;
    when(
        move || prompt.get().is_some(),
        move || {
            let shown = prompt.get().unwrap_or_else(|| unreachable!());
            let value = shown.value;
            let title = shown.title;
            let hint = shown.hint;
            let confirm = shown.confirm;
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(420.0))
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .child(
                            text(title)
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || fg_strong()),
                        )
                        .child(
                            view()
                                .h(px(28.0))
                                .w_full()
                                .flex_row()
                                .items_center()
                                .px_2()
                                .rounded(px(5.0))
                                .border_1()
                                .border_color(move || theme().input)
                                .bg(move || theme().background)
                                .child(
                                    input()
                                        .value(move || value.get())
                                        .on_change(move |text: &str| value.set(text.to_owned()))
                                        .auto_focus(true)
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_size(px(12.5))
                                        .text_color(move || fg()),
                                ),
                        )
                        .child(
                            text(hint)
                                .text_size(px(11.0))
                                .text_color(move || fg_muted()),
                        )
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(
                                    button()
                                        .accessibility_label("Cancel")
                                        .h(px(26.0))
                                        .px_3()
                                        .rounded(px(6.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(move || theme().accent))
                                        .on_press(move |_: &()| state::dismiss_prompt(store))
                                        .child(
                                            text("Cancel")
                                                .text_size(px(12.0))
                                                .text_color(move || fg_muted()),
                                        ),
                                )
                                .child(
                                    button()
                                        .accessibility_label(confirm)
                                        .h(px(26.0))
                                        .px_3()
                                        .rounded(px(6.0))
                                        .cursor_pointer()
                                        .bg(move || theme().primary)
                                        .hover(|s| s.opacity(0.9))
                                        .on_press(move |_: &()| state::accept_prompt(store))
                                        .child(
                                            text(confirm)
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(move || theme().primary_foreground),
                                        ),
                                ),
                        ),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}

/// The confirmation, while one is open.
///
/// It shares the name prompt's frame — one modal look, one overlay, one pair
/// of buttons — so a destructive action reads as the same kind of moment as
/// a rename, and the button that does it is the one tinted as the primary.
pub fn confirm_dialog(store: Store) -> impl IntoElement {
    let confirm = store.confirm;
    when(
        move || confirm.get().is_some(),
        move || {
            let shown = confirm.get().unwrap_or_else(|| unreachable!());
            let title = shown.title.clone();
            let hint = shown.hint.clone();
            let label = shown.confirm;
            view()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .bg(move || theme().background.opacity(0.55))
                .child(
                    view()
                        .w(px(420.0))
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(move || border())
                        .bg(move || theme().popover)
                        .shadow_md()
                        .child(
                            text(title)
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || fg_strong()),
                        )
                        .child(
                            text(hint)
                                .text_size(px(11.0))
                                .text_color(move || fg_muted()),
                        )
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(
                                    button()
                                        .accessibility_label("Cancel")
                                        .h(px(26.0))
                                        .px_3()
                                        .rounded(px(6.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(move || theme().accent))
                                        .on_press(move |_: &()| state::dismiss_confirm(store))
                                        .child(
                                            text("Cancel")
                                                .text_size(px(12.0))
                                                .text_color(move || fg_muted()),
                                        ),
                                )
                                .child(
                                    button()
                                        .accessibility_label(label)
                                        .h(px(26.0))
                                        .px_3()
                                        .rounded(px(6.0))
                                        .cursor_pointer()
                                        .bg(move || theme().primary)
                                        .hover(|s| s.opacity(0.9))
                                        .on_press(move |_: &()| state::accept_confirm(store))
                                        .child(
                                            text(label)
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(move || theme().primary_foreground),
                                        ),
                                ),
                        ),
                )
        },
    )
    .otherwise(|| view().size(px(0.0)))
}
