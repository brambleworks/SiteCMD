//! Scoring - aggregates findings into per-category and overall health scores.
//! The desktop re-exports these modules at `crate::scoring::{calculator, dedup}`.

pub mod calculator;
pub mod dedup;
