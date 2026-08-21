//! LemonSqueezy activation, validation, persistence, and desktop commands.

pub mod access;
pub mod activation_errors;
pub mod api;
#[cfg(feature = "desktop")]
pub mod commands;
pub mod config;
pub mod manifest;
pub mod store;
