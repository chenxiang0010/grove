//! The commit graph's gutter, drawn with a canvas.
//!
//! One row of the log draws one row of the graph: the lanes arriving from the
//! row above, the commit's dot, and the edges leaving toward its parents. The
//! arithmetic is `git_core::graph`; this file only turns a lane into an x and
//! a colour into a stroke.
//!
//! Drawing per row rather than as one tall canvas is what lets the list
//! virtualize: a cell that scrolls away keeps nothing.

use std::sync::Arc;

use git_core::GraphLayout;
use git_core::graph::{LANE_PAD, LANE_STEP, gutter_width};
use nsh_base::theme;
use nsh_core::canvas::LineCap;
use nsh_core::prelude::*;

use crate::theme_tokens::{DOT_R, ROW_H, graph_color};

/// The x of `lane` inside a gutter.
fn lane_x(lane: usize) -> f32 {
    LANE_PAD + LANE_STEP * lane as f32
}

/// The whole gutter's width for a layout.
pub fn width(layout: &GraphLayout) -> f32 {
    gutter_width(layout.max_lanes.max(1))
}

/// One row of the graph: `layout`'s row `index`.
pub fn graph_cell(layout: Arc<GraphLayout>, index: usize) -> impl IntoElement {
    let gutter = width(&layout);
    let row = layout.rows.get(index).cloned().unwrap_or_default();
    let incoming = layout.incoming(index);
    let mid = ROW_H / 2.0;

    canvas(move || {
        let mut painter = Painter::new().line_width(1.6).line_cap(LineCap::Round);

        // The lanes coming down from the row above. The commit's own lane
        // stops at the dot; every other one runs on.
        for (lane, lane_color) in &incoming {
            let x = lane_x(*lane);
            let bottom = if *lane == row.lane { mid } else { ROW_H };
            painter = painter
                .stroke_color(color(graph_color(*lane_color)))
                .begin_path()
                .move_to(x, 0.0)
                .line_to(x, bottom)
                .stroke();
        }

        // The lanes that pass this row without a dot.
        for (lane, lane_color) in &row.through {
            if incoming.iter().any(|(seen, _)| seen == lane) {
                continue;
            }
            let x = lane_x(*lane);
            painter = painter
                .stroke_color(color(graph_color(*lane_color)))
                .begin_path()
                .move_to(x, 0.0)
                .line_to(x, ROW_H)
                .stroke();
        }

        // The edges leaving the dot toward each parent.
        for edge in &row.edges {
            let (from, to) = (lane_x(edge.from), lane_x(edge.to));
            let stroke = color(graph_color(edge.color));
            painter = if from == to {
                painter
                    .stroke_color(stroke)
                    .begin_path()
                    .move_to(from, mid)
                    .line_to(to, ROW_H)
                    .stroke()
            } else {
                // A branch leaving its line curves into the next lane, which
                // is what makes a merge read as a merge.
                painter
                    .stroke_color(stroke)
                    .begin_path()
                    .move_to(from, mid)
                    .bezier_curve_to(
                        point(px(from), px(mid + (ROW_H - mid) * 0.55)),
                        point(px(to), px(mid + (ROW_H - mid) * 0.45)),
                        point(px(to), px(ROW_H)),
                    )
                    .stroke()
            };
        }

        // The dot. A merge is drawn hollow, so a branch point is visible at a
        // glance.
        let cx = lane_x(row.lane);
        let dot = color(graph_color(row.color));
        painter = if row.merge {
            painter
                .fill_color(color(theme().background))
                .begin_path()
                .arc(cx, mid, DOT_R, 0.0, std::f32::consts::TAU)
                .fill()
                .stroke_color(dot)
                .line_width(1.6)
                .begin_path()
                .arc(cx, mid, DOT_R, 0.0, std::f32::consts::TAU)
                .stroke()
        } else {
            painter
                .fill_color(dot)
                .begin_path()
                .arc(cx, mid, DOT_R, 0.0, std::f32::consts::TAU)
                .fill()
        };

        painter
    })
    .w(px(gutter))
    .h(px(ROW_H))
    .flex_none()
}
