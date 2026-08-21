//! Desktop adapters for the portable scoring engine.

pub mod calculator;
pub mod live;

pub use sitecmd_engine::scoring::dedup;

#[cfg(test)]
#[path = "score_parity_tests.rs"]
mod score_parity_tests;
