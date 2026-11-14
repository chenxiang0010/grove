//! The Git model behind GitDesk: reading a repository and changing it.
//!
//! This crate knows nothing about a user interface, a window or a signal, so
//! every rule here is exercised by `cargo test` against real repositories
//! rather than through a screen.
//!
//! # Why two halves
//!
//! Reads run in-process through libgit2 ([`repo`]): a status, a log page and a
//! diff cost milliseconds on an ordinary repository, which is what makes the
//! change list and the log feel immediate.
//!
//! Writes run through the `git` executable ([`ops`]): hooks, credential
//! helpers, commit signing and `.gitattributes` filters are the executable's
//! job, and reimplementing them in libgit2 is how a client ends up committing
//! something the command line would have committed differently.
//!
//! # The graph
//!
//! [`graph`] lays the commit graph out as lanes. It is pure arithmetic over
//! `(hash, parents)` pairs, so the layout is testable without a repository.

pub mod cli;
pub mod error;
pub mod graph;
pub mod ops;
pub mod patch;
pub mod repo;

pub use error::{GitError, Result};
pub use graph::{
    Edge, GraphLayout, GraphRow, LANE_PAD, LANE_STEP, Lane, PALETTE_LEN, gutter_width,
};
pub use ops::TagPush;
pub use repo::{
    BlameLine, BranchState, ChangeKind, Commit, CommitDetails, CommitFile, RefEntry, RefKind,
    Remote, Repo, RepoPaths, StashEntry, Status, StatusEntry, format_timestamp, now_seconds,
    relative_timestamp,
};
