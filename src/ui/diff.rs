//! A patch, through `nsh_base`'s `DiffView`.
//!
//! `DiffView` draws its rows in an absolutely positioned scroller, so it
//! needs a definite height: `flex_1` on its own leaves it at zero and the
//! rows paint outside the window. Every caller wraps it the same way.

use nsh_base::DiffLayout;
use nsh_base::diff_view;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui::empty_state;

/// The patch `patch` holds, or `title`/`hint` while it holds nothing.
pub fn diff_pane(
    patch: Signal<String>,
    language: &'static str,
    title: &'static str,
    hint: &'static str,
) -> impl IntoElement {
    view()
        .flex_1()
        .min_h(px(0.0))
        .min_w(px(0.0))
        .w_full()
        .flex_col()
        .overflow_hidden()
        .bg(move || theme().background)
        .child(
            when(
                move || !patch.get().is_empty(),
                move || {
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .min_w(px(0.0))
                        .w_full()
                        .overflow_hidden()
                        .child(
                            diff_view()
                                .patch(move || Some(SharedString::from(patch.get())))
                                .language(language)
                                // Side by side, as the reference shows a file's
                                // changes: the old text on the left, the new on
                                // the right, their lines aligned.
                                .layout(DiffLayout::Split)
                                .context_lines(3)
                                .font_size(px(12.0))
                                .line_height(px(19.0))
                                .w_full()
                                .h_full(),
                        )
                },
            )
            .otherwise(move || empty_state(title, hint)),
        )
}

/// The patch `patch` holds, with a pair of actions beside every hunk.
///
/// The actions are the reference's hunk arrows: a change of the working tree
/// can be staged or rolled back one hunk at a time, which is the other half
/// of "commit only some of what changed".
pub fn diff_pane_with_hunks(
    store: crate::state::Store,
    patch: Signal<String>,
    language: &'static str,
    path: String,
    staged: bool,
    title: &'static str,
    hint: &'static str,
) -> impl IntoElement {
    let for_patch = patch;
    let path = std::rc::Rc::new(path);
    let stage_path = path.clone();
    let discard_path = path.clone();
    view()
        .flex_1()
        .min_h(px(0.0))
        .min_w(px(0.0))
        .w_full()
        .flex_col()
        .overflow_hidden()
        .bg(move || theme().background)
        .child(
            when(
                move || !for_patch.get().is_empty(),
                move || {
                    // The branch is an `Fn`: each call clones the handles it
                    // hands to the hunk closure, which takes them by move.
                    let stage_path = stage_path.clone();
                    let discard_path = discard_path.clone();
                    view()
                        .flex_1()
                        .min_h(px(0.0))
                        .min_w(px(0.0))
                        .w_full()
                        .overflow_hidden()
                        .child(
                            diff_view()
                                .patch(move || Some(SharedString::from(for_patch.get())))
                                .language(language)
                                // Side by side, as the reference shows a
                                // file's changes: the old text on the left,
                                // the new on the right, their lines aligned.
                                .layout(DiffLayout::Split)
                                .context_lines(3)
                                .font_size(px(12.0))
                                .line_height(px(19.0))
                                .render_hunk_actions(move |hunk: &nsh_base::DiffHunk| {
                                    let patch = for_patch.get_untracked();
                                    let stage_path = stage_path.clone();
                                    let discard_path = discard_path.clone();
                                    let index = hunk.index as usize;
                                    view()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .px_1()
                                        .child(
                                            view()
                                                .h(px(18.0))
                                                .px_1p5()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(4.0))
                                                .cursor_pointer()
                                                .bg(move || theme().accent)
                                                .on_press({
                                                    let patch = patch.clone();
                                                    let path = stage_path.clone();
                                                    move |_: &()| {
                                                        let patch = patch.clone();
                                                        let path = path.as_ref().clone();
                                                        if staged {
                                                            crate::state::unstage_hunk(
                                                                store, patch, path, index,
                                                            );
                                                        } else {
                                                            crate::state::stage_hunk(
                                                                store, patch, path, index,
                                                            );
                                                        }
                                                    }
                                                })
                                                .child(
                                                    text(if staged {
                                                        "Unstage hunk"
                                                    } else {
                                                        "Stage hunk"
                                                    })
                                                    .text_size(px(10.5))
                                                    .text_color(move || theme().foreground),
                                                ),
                                        )
                                        .child(
                                            view()
                                                .h(px(18.0))
                                                .px_1p5()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(4.0))
                                                .cursor_pointer()
                                                .bg(move || theme().accent)
                                                .on_press({
                                                    let patch = patch.clone();
                                                    let path = discard_path.clone();
                                                    move |_: &()| {
                                                        crate::state::discard_hunk(
                                                            store,
                                                            patch.clone(),
                                                            path.as_ref().clone(),
                                                            index,
                                                        );
                                                    }
                                                })
                                                .child(
                                                    text("Rollback hunk")
                                                        .text_size(px(10.5))
                                                        .text_color(move || theme().foreground),
                                                ),
                                        )
                                        .into_element()
                                })
                                .w_full()
                                .h_full(),
                        )
                },
            )
            .otherwise(move || empty_state(title, hint)),
        )
}
