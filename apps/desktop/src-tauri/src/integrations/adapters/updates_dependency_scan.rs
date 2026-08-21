//! Project-scoped dependency-scan cache for the Updates adapter.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::db::Database;
use crate::updates::registry::RegistryScan;

/// Authority level of a project dependency scan.
pub(super) enum DependencyScanOutcome {
    /// Complete census; an empty result is authoritative.
    Scanned(RegistryScan),
    /// Partial census; findings are authoritative but absences are not.
    Degraded(RegistryScan),
    /// Project root unavailable; no authoritative observation was made.
    Unavailable,
}

/// Shares project dependency scans across environment polls in one cadence window.
#[derive(Default)]
pub(super) struct DependencyScanCache {
    entries: tokio::sync::Mutex<HashMap<i64, (Instant, RegistryScan, bool)>>,
}

impl DependencyScanCache {
    /// Serialize project scans so concurrent environment polls share one result.
    pub(super) async fn scan_for_project(
        &self,
        db: &Arc<Database>,
        cadence: Duration,
        project_id: i64,
    ) -> DependencyScanOutcome {
        let mut entries = self.entries.lock().await;
        if let Some((at, scan, partial)) = entries.get(&project_id) {
            if at.elapsed() < cadence {
                return if *partial {
                    DependencyScanOutcome::Degraded(scan.clone())
                } else {
                    DependencyScanOutcome::Scanned(scan.clone())
                };
            }
        }

        // Do not cache unavailable paths as authoritative empty results.
        let path = match db
            .get_project_path(project_id)
            .filter(|path| !path.is_empty())
        {
            Some(path) => std::path::PathBuf::from(path),
            None => return DependencyScanOutcome::Unavailable,
        };
        if !path.is_dir() {
            return DependencyScanOutcome::Unavailable;
        }

        // Keep bounded lockfile parsing off async workers.
        let detection = match tokio::task::spawn_blocking(move || {
            std::fs::read_dir(&path)?;
            Ok::<_, std::io::Error>(crate::updates::detect_dependencies(&path))
        })
        .await
        {
            Ok(Ok(detection)) => detection,
            Ok(Err(e)) => {
                tracing::warn!("updates_adapter: project folder unreadable: {}", e);
                return DependencyScanOutcome::Unavailable;
            }
            Err(e) => {
                tracing::warn!("updates_adapter: dependency detection failed: {}", e);
                return DependencyScanOutcome::Unavailable;
            }
        };

        // Cache partial observations so missing registry data cannot false-resolve issues.
        let scan = if detection.packages.is_empty() {
            RegistryScan::default()
        } else {
            crate::updates::registry::check_for_updates(&detection.packages).await
        };
        let partial = detection.partial || scan.partial;
        entries.insert(project_id, (Instant::now(), scan.clone(), partial));
        if partial {
            tracing::warn!(
                "updates_adapter: dependency scan partial for project {} (unreadable dependency file/directory or incomplete registry sweep); dependency resolution suppressed",
                project_id
            );
            DependencyScanOutcome::Degraded(scan)
        } else {
            DependencyScanOutcome::Scanned(scan)
        }
    }

    /// Return the scan and whether dependency findings require partial resolution.
    pub(super) async fn scan_or_partial(
        &self,
        db: &Arc<Database>,
        cadence: Duration,
        project_id: i64,
    ) -> (RegistryScan, bool) {
        match self.scan_for_project(db, cadence, project_id).await {
            DependencyScanOutcome::Scanned(scan) => (scan, false),
            DependencyScanOutcome::Degraded(scan) => (scan, true),
            DependencyScanOutcome::Unavailable => (RegistryScan::default(), true),
        }
    }
}
