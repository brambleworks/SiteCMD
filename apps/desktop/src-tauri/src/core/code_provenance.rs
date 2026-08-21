//! Scan-time checkout identity for Code Scan evidence.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeCheckoutProvenance {
    pub commit_sha: Option<String>,
    pub tree_clean: Option<bool>,
}

impl CodeCheckoutProvenance {
    /// Read the checkout identity at the code collector boundary. A non-Git
    /// project carries no invented commit and cannot claim an exact basis.
    pub fn capture(project_path: &str) -> Self {
        let Some((commit_sha, tree_clean)) = super::git::checkout_head_and_clean(project_path)
        else {
            return Self::default();
        };
        Self {
            commit_sha: Some(commit_sha),
            tree_clean: Some(tree_clean),
        }
    }

    /// Exactness requires the same clean HEAD before and after the audit.
    pub fn confirm_unchanged(mut self, after: Self) -> Self {
        if self.commit_sha.is_none() && after.commit_sha.is_none() {
            return Self::default();
        }
        self.tree_clean = Some(
            self.commit_sha == after.commit_sha
                && self.tree_clean == Some(true)
                && after.tree_clean == Some(true),
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactness_requires_the_same_clean_checkout_after_the_audit() {
        let clean = CodeCheckoutProvenance {
            commit_sha: Some("abc1234".into()),
            tree_clean: Some(true),
        };
        assert_eq!(clean.clone().confirm_unchanged(clean.clone()), clean);
        for after in [
            CodeCheckoutProvenance {
                commit_sha: Some("abc1234".into()),
                tree_clean: Some(false),
            },
            CodeCheckoutProvenance {
                commit_sha: Some("def5678".into()),
                tree_clean: Some(true),
            },
        ] {
            assert_eq!(
                clean.clone().confirm_unchanged(after),
                CodeCheckoutProvenance {
                    commit_sha: Some("abc1234".into()),
                    tree_clean: Some(false),
                },
            );
        }
    }
}
