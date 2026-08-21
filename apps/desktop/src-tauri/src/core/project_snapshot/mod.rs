//! Desktop-only project snapshot builders over Tauri and database surfaces.

pub mod action_items;

pub(crate) use action_items::{build_project_work_queue, build_project_work_summary};
