//! One cumulative byte budget for source and retained configuration text.

use super::filesystem::{self, ProjectFile, SourceFile};
use super::CodeScanError;
use std::path::Path;

pub(super) struct ScanTextBudget<'a> {
    max_bytes: u64,
    retained_bytes: u64,
    cancelled: &'a (dyn Fn() -> bool + Sync),
}

impl<'a> ScanTextBudget<'a> {
    pub(super) fn new(max_bytes: u64, cancelled: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self {
            max_bytes,
            retained_bytes: 0,
            cancelled,
        }
    }

    pub(super) fn account_sources(&mut self, files: &[SourceFile]) -> Result<(), CodeScanError> {
        for file in files {
            self.retain(&file.content)?;
        }
        Ok(())
    }

    pub(super) fn check_cancelled(&self) -> Result<(), CodeScanError> {
        if (self.cancelled)() {
            Err(CodeScanError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn retain(&mut self, content: &String) -> Result<(), CodeScanError> {
        self.check_cancelled()?;
        let bytes = content.capacity() as u64;
        if bytes > self.max_bytes.saturating_sub(self.retained_bytes) {
            return Err(CodeScanError::Failed(format!(
                "Code Scan stopped after reaching the {} byte source and configuration text budget. The audit is incomplete and no report was produced. Choose a smaller project root or exclude generated folders.",
                self.max_bytes
            )));
        }
        self.retained_bytes += bytes;
        Ok(())
    }

    pub(super) fn read_project_file(
        &mut self,
        file: &ProjectFile,
        max_bytes: u64,
    ) -> Result<Option<String>, CodeScanError> {
        self.check_cancelled()?;
        let content = filesystem::read_project_file(file, max_bytes)
            .filter(|bytes| !bytes.contains(&0))
            .and_then(|bytes| String::from_utf8(bytes).ok());
        self.account_read(content)
    }

    pub(super) fn read_text_under_root(
        &mut self,
        root: &Path,
        path: &Path,
    ) -> Result<Option<String>, CodeScanError> {
        self.check_cancelled()?;
        self.account_read(filesystem::read_text_under_root(root, path))
    }

    fn account_read(&mut self, content: Option<String>) -> Result<Option<String>, CodeScanError> {
        self.check_cancelled()?;
        if let Some(content) = &content {
            self.retain(content)?;
        }
        Ok(content)
    }
}

#[cfg(test)]
#[path = "text_budget_tests.rs"]
mod tests;
