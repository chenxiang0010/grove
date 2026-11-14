//! Grove's own look.
//!
//! `nsh_base` owns behaviour and draws nothing; every colour, size and press
//! feedback in Grove comes from here, so the whole window reads as one
//! surface. Nothing below a screen names a literal colour: it names a theme
//! token, which is what keeps light and dark in step.

use std::rc::Rc;

pub mod changes;
pub mod clone_dialog;
pub mod commit_box;
pub mod compare;
pub mod details;
pub mod diff;
pub mod editor_tab;
pub mod frame;
pub mod graph_gutter;
pub mod icons;
pub mod log;
pub mod menus;
pub mod project;
pub mod prompt;
pub mod push_dialog;
pub mod sidebar;
pub mod stashes;
pub mod tabs;
pub mod update_dialog;

pub use frame::welcome_screen;

use nsh_base::{button, theme};
use nsh_core::element::StyleRefinement;
use nsh_core::prelude::*;

use crate::theme_tokens::{
    MOTION_FAST, border, fg_muted, icon_color, island_bg, row_hover_bg, toolbar_hover_bg,
};
use crate::ui::icons::{Icon, icon};

/// A press target with a label: a toolbar action, a row action.
pub fn tool_button(
    label: &'static str,
    glyph: Icon,
    press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    button()
        .accessibility_label(label)
        .flex_row()
        .items_center()
        .gap_1()
        .h(px(24.0))
        .px_1p5()
        .rounded(px(5.0))
        .cursor_pointer()
        .hover(|s| s.bg(move || toolbar_hover_bg()))
        .active(|s| s.bg(move || row_hover_bg()))
        .on_press(press)
        .child(icon(glyph, 14.0).text_color(move || icon_color()))
        .child(
            text(label)
                .text_size(px(12.0))
                .text_color(move || theme().foreground),
        )
}

/// The one action a screen is for: commit, push, open.
///
/// `enabled` is a closure so the button follows the state that decides it
/// without the screen rebuilding: a component runs once in NSH.
pub fn primary_button(
    label: &'static str,
    enabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let enabled: Rc<dyn Fn() -> bool> = Rc::new(enabled);
    let (off, bg, fg) = (enabled.clone(), enabled.clone(), enabled);
    button()
        .accessibility_label(label)
        .disabled(move || !off())
        .h(px(26.0))
        .px_3()
        .rounded(px(6.0))
        .cursor_pointer()
        .bg(move || if bg() { theme().primary } else { theme().muted })
        .hover(|s| s.opacity(0.9))
        .active(|s| s.opacity(0.75))
        .items_center()
        .justify_center()
        .on_press(press)
        .child(
            text(label)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || {
                    if fg() {
                        theme().primary_foreground
                    } else {
                        theme().muted_foreground
                    }
                }),
        )
}

/// A chip that stays lit while it is on: amend, include-untracked.
pub fn toggle_chip(
    label: &'static str,
    active: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let active: Rc<dyn Fn() -> bool> = Rc::new(active);
    let (border, ground, fg) = (active.clone(), active.clone(), active);
    button()
        .accessibility_label(label)
        .h(px(24.0))
        .px_2()
        .rounded(px(6.0))
        .cursor_pointer()
        .border_1()
        .border_color(move || {
            if border() {
                theme().primary
            } else {
                theme().border
            }
        })
        .bg(move || {
            if ground() {
                theme().primary.opacity(0.14)
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .hover(|s| s.bg(move || theme().accent))
        .items_center()
        .justify_center()
        .transition_all()
        .duration(MOTION_FAST)
        .on_press(press)
        .child(text(label).text_size(px(11.0)).text_color(move || {
            if fg() {
                theme().primary
            } else {
                theme().muted_foreground
            }
        }))
}

/// A short count or label on a tinted ground.
pub fn badge(label: impl Fn() -> String + 'static) -> impl IntoElement {
    view()
        .h(px(16.0))
        .px_1p5()
        .rounded(px(4.0))
        .items_center()
        .justify_center()
        .bg(move || theme().muted)
        .child(
            text(label)
                .text_size(px(10.0))
                .text_color(move || theme().muted_foreground),
        )
}

/// A branch, tag or `HEAD` marker beside a commit subject.
pub fn ref_badge(label: impl Into<SharedString>, color: AdaptiveColor) -> impl IntoElement {
    let label: SharedString = label.into();
    view()
        .h(px(17.0))
        .px_1p5()
        .rounded(px(4.0))
        .flex_none()
        .items_center()
        .justify_center()
        .bg(move || color.opacity(0.16))
        .child(
            text(label)
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || color),
        )
}

/// A thin rule between two regions.
pub fn divider() -> impl IntoElement {
    view().h(px(1.0)).w_full().flex_none().bg(move || border())
}

/// Centred text for a list or a pane with nothing in it.
pub fn empty_state(title: &'static str, hint: &'static str) -> impl IntoElement {
    view()
        .flex_1()
        .w_full()
        .min_h(px(120.0))
        .items_center()
        .justify_center()
        .gap_1()
        .p_6()
        .bg(move || island_bg())
        .child(
            text(title)
                .text_size(px(13.0))
                .text_color(move || fg_muted()),
        )
        .child(
            text(hint)
                .text_size(px(11.0))
                .text_color(move || fg_muted().opacity(0.75)),
        )
}

/// The popup look every menu and select in Grove shares.
pub fn popup_style(s: StyleRefinement) -> StyleRefinement {
    s.p_1()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().popover)
        .shadow_md()
}
