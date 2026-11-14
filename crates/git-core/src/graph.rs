//! Laying out the commit graph.
//!
//! The log lists commits newest first, one row each. The graph beside them is
//! a set of lanes: a lane is a line carried down the list toward a commit that
//! has not been placed yet. A commit takes the lane already carrying it; a
//! commit nothing carries takes the lane already heading to its first parent
//! (its branch continues upward); failing both, it takes the leftmost free
//! lane. Its first parent then continues in that lane and the rest take lanes
//! to the right, which is what a merge looks like.
//!
//! This is the same model IntelliJ's log uses, reduced to what a reader needs:
//! a dot per commit, an edge per parent link, and a colour that stays with a
//! lane so a branch reads as one line.

/// One line carried down the list.
pub type Lane = usize;

/// A segment of an edge inside one row: it enters at `from` on the row's own
/// line and leaves at `to` on the next row's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// The lane at the row's centre line.
    pub from: Lane,
    /// The lane at the row's bottom edge.
    pub to: Lane,
    /// The colour index, stable for the branch the edge belongs to.
    pub color: u8,
    /// The parent's position in the commit's parent list.
    pub parent: usize,
}

/// A commit's place in the graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphRow {
    /// The lane the commit's dot sits in.
    pub lane: Lane,
    /// The colour of the commit's dot.
    pub color: u8,
    /// The edges leaving this row downward.
    pub edges: Vec<Edge>,
    /// Lanes that pass through this row without a dot.
    pub through: Vec<(Lane, u8)>,
    /// How many lanes this row needs, so the gutter can be sized.
    pub lanes: usize,
    /// True when the commit has more than one parent.
    pub merge: bool,
}

/// The whole graph, one row per commit in the order they were given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphLayout {
    pub rows: Vec<GraphRow>,
    /// The widest row, in lanes: the gutter's width.
    pub max_lanes: usize,
}

/// How many colours the graph cycles through.
pub const PALETTE_LEN: u8 = 8;

impl GraphLayout {
    /// Lays out `commits` as `(hash, parents)`, newest first.
    ///
    /// A parent that never appears in the list (history truncated by the log's
    /// limit, or a shallow clone) still gets its lane, which stays open to the
    /// bottom edge — that open line is what tells a reader the history
    /// continues.
    pub fn build(commits: &[(String, Vec<String>)]) -> Self {
        // A lane holds the commit it is heading toward, and its colour.
        let mut lanes: Vec<Option<(String, u8)>> = Vec::new();
        let mut next_color: u8 = 0;
        let mut rows = Vec::with_capacity(commits.len());

        for (hash, parents) in commits {
            let carrying: Vec<Lane> = lanes
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.as_ref().is_some_and(|(target, _)| target == hash))
                .map(|(index, _)| index)
                .collect();

            // Where the dot goes, and the colour it inherits.
            let (lane, color) = match carrying.first() {
                Some(first) => {
                    let color = lanes[*first].as_ref().map(|(_, color)| *color).unwrap_or(0);
                    (*first, color)
                }
                None => {
                    // Nothing carries this commit: continue the lane already
                    // heading to its first parent, so a branch reads as one
                    // line, else take the leftmost free lane.
                    let by_parent = parents.first().and_then(|parent| {
                        lanes
                            .iter()
                            .position(|slot| slot.as_ref().is_some_and(|(t, _)| t == parent))
                    });
                    match by_parent {
                        Some(index) => {
                            let color = lanes[index].as_ref().map(|(_, color)| *color).unwrap_or(0);
                            (index, color)
                        }
                        None => {
                            let index = free_lane(&mut lanes);
                            let color = next_color % PALETTE_LEN;
                            next_color = next_color.wrapping_add(1);
                            (index, color)
                        }
                    }
                }
            };

            // Lanes that converge here are released; the dot's lane is
            // re-assigned below by the first parent.
            for index in &carrying {
                lanes[*index] = None;
            }
            let _ = &carrying;
            lanes[lane] = None;

            let mut edges = Vec::with_capacity(parents.len());
            let mut used: Vec<Lane> = Vec::with_capacity(parents.len());
            for (position, parent) in parents.iter().enumerate() {
                let target_lane = if position == 0 {
                    lane
                } else {
                    let existing = lanes
                        .iter()
                        .position(|slot| slot.as_ref().is_some_and(|(t, _)| t == parent));
                    existing.unwrap_or_else(|| free_lane(&mut lanes))
                };
                let edge_color = if target_lane == lane {
                    color
                } else {
                    // A lane reused from another branch keeps that branch's
                    // colour, so the line does not change colour mid-run.
                    let inherited = lanes[target_lane].as_ref().map(|(_, c)| *c);
                    inherited.unwrap_or_else(|| {
                        let c = next_color % PALETTE_LEN;
                        next_color = next_color.wrapping_add(1);
                        c
                    })
                };
                lanes[target_lane] = Some((parent.clone(), edge_color));
                used.push(target_lane);
                edges.push(Edge {
                    from: lane,
                    to: target_lane,
                    color: edge_color,
                    parent: position,
                });
            }

            // Lanes nothing heads toward any more are dropped from the right,
            // so the gutter does not keep growing after a branch ends.
            while matches!(lanes.last(), Some(None)) {
                lanes.pop();
            }

            let through: Vec<(Lane, u8)> = lanes
                .iter()
                .enumerate()
                .filter(|(index, slot)| *index != lane && slot.is_some())
                .filter_map(|(index, slot)| slot.as_ref().map(|(_, color)| (index, *color)))
                .collect();

            let width = (lane + 1)
                .max(edges.iter().map(|e| e.to + 1).max().unwrap_or(0))
                .max(lanes.len());

            rows.push(GraphRow {
                lane,
                color,
                edges,
                through,
                lanes: width,
                merge: parents.len() > 1,
            });
        }

        let max_lanes = rows.iter().map(|row| row.lanes).max().unwrap_or(1);
        Self { rows, max_lanes }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The lanes crossing the top edge of row `index`: the previous row's
    /// edges and the lanes that ran through it.
    ///
    /// A row draws the half of a lane above its dot from this, so a line is
    /// never cut between two rows.
    pub fn incoming(&self, index: usize) -> Vec<(Lane, u8)> {
        let Some(previous) = index.checked_sub(1).and_then(|at| self.rows.get(at)) else {
            return Vec::new();
        };
        let mut lanes: Vec<(Lane, u8)> = previous
            .edges
            .iter()
            .map(|edge| (edge.to, edge.color))
            .collect();
        for (lane, color) in &previous.through {
            if !lanes.iter().any(|(seen, _)| seen == lane) {
                lanes.push((*lane, *color));
            }
        }
        lanes.sort_unstable();
        lanes.dedup_by_key(|(lane, _)| *lane);
        lanes
    }

    /// How wide the gutter must be, in points, for `max_lanes` lanes.
    pub fn gutter_width(&self) -> f32 {
        crate::graph::gutter_width(self.max_lanes)
    }
}

/// The width a gutter needs for `lanes` lanes: a margin, then one step per
/// lane.
pub fn gutter_width(lanes: usize) -> f32 {
    LANE_PAD * 2.0 + LANE_STEP * lanes.saturating_sub(1) as f32
}

/// The distance between two lanes.
pub const LANE_STEP: f32 = 14.0;
/// The margin before the first lane and after the last.
pub const LANE_PAD: f32 = 12.0;

/// The leftmost lane nothing is heading toward, opening a new one when every
/// lane is taken.
fn free_lane(lanes: &mut Vec<Option<(String, u8)>>) -> Lane {
    match lanes.iter().position(Option::is_none) {
        Some(index) => index,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, parents: &[&str]) -> (String, Vec<String>) {
        (
            hash.to_owned(),
            parents.iter().map(|p| (*p).to_owned()).collect(),
        )
    }

    #[test]
    fn a_linear_history_stays_in_one_lane() {
        let layout =
            GraphLayout::build(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
        assert_eq!(layout.len(), 3);
        assert_eq!(layout.max_lanes, 1);
        assert!(layout.rows.iter().all(|row| row.lane == 0));
        assert_eq!(layout.rows[0].edges.len(), 1);
        assert_eq!(layout.rows[0].edges[0].to, 0);
        assert!(layout.rows[2].edges.is_empty());
    }

    #[test]
    fn a_merge_opens_a_second_lane() {
        // m merges b and c; c sits on its own line until a.
        let layout = GraphLayout::build(&[
            commit("m", &["b", "c"]),
            commit("b", &["a"]),
            commit("c", &["a"]),
            commit("a", &[]),
        ]);
        assert_eq!(layout.rows[0].lane, 0);
        assert!(layout.rows[0].merge);
        assert_eq!(layout.rows[0].edges.len(), 2);
        assert_eq!(layout.rows[0].edges[0].to, 0);
        assert_eq!(layout.rows[0].edges[1].to, 1);
        assert_eq!(layout.max_lanes, 2);
        // b continues lane 0, c takes lane 1.
        assert_eq!(layout.rows[1].lane, 0);
        assert_eq!(layout.rows[2].lane, 1);
    }

    #[test]
    fn lanes_close_when_their_branch_ends() {
        let layout = GraphLayout::build(&[
            commit("m", &["b", "c"]),
            commit("c", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        let last = layout.rows.last().unwrap();
        assert_eq!(last.lane, 0);
        assert!(last.through.is_empty());
    }

    #[test]
    fn two_merges_into_one_commit_share_its_lane() {
        let layout = GraphLayout::build(&[
            commit("m1", &["a", "x"]),
            commit("m2", &["a", "y"]),
            commit("x", &["a"]),
            commit("y", &["a"]),
            commit("a", &[]),
        ]);
        // m2's first parent is `a`, so it joins the lane already heading there
        // instead of opening a fourth.
        assert!(layout.max_lanes <= 3, "{}", layout.max_lanes);
        assert_eq!(layout.rows.last().unwrap().lane, 0);
    }

    #[test]
    fn an_unknown_parent_keeps_its_lane_open() {
        let layout = GraphLayout::build(&[commit("a", &["not-in-the-list"])]);
        assert_eq!(layout.rows[0].edges.len(), 1);
        assert_eq!(layout.rows[0].edges[0].to, 0);
    }

    #[test]
    fn colors_come_from_the_palette_and_stay_with_a_lane() {
        let layout = GraphLayout::build(&[
            commit("m", &["b", "c"]),
            commit("b", &["a"]),
            commit("c", &["a"]),
            commit("a", &[]),
        ]);
        for row in &layout.rows {
            assert!(row.color < PALETTE_LEN);
        }
        // The straight-down first-parent line keeps the merge commit's colour.
        assert_eq!(layout.rows[0].edges[0].color, layout.rows[0].color);
        // The side branch keeps its own colour down to the join.
        let side = layout.rows[0].edges[1].color;
        assert_eq!(layout.rows[2].color, side);
    }
}
