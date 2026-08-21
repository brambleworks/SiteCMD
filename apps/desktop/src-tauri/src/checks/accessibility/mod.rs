//! Accessibility checks: alt text, form labels, ARIA, color contrast, landmarks, language.

pub use sitecmd_engine::checks::accessibility::extra_checks;
pub use sitecmd_engine::checks::accessibility::form_labels;
pub use sitecmd_engine::checks::accessibility::html_checks;
pub use sitecmd_engine::checks::accessibility::markup_checks;

use super::Check;

pub fn sync_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(html_checks::LangAttributeCheck),
        Box::new(html_checks::ImageAltAccessibilityCheck),
        Box::new(html_checks::HeadingOrderCheck),
        Box::new(form_labels::FormLabelsCheck),
        Box::new(html_checks::AriaLandmarksCheck),
        Box::new(html_checks::LinkTextCheck),
        Box::new(html_checks::SkipNavCheck),
        Box::new(html_checks::AutoplayCheck),
        Box::new(extra_checks::ColorContrastHintsCheck),
        Box::new(extra_checks::FocusIndicatorCheck),
        Box::new(extra_checks::AriaUsageCheck),
        Box::new(extra_checks::TabindexCheck),
        Box::new(markup_checks::ViewportZoomCheck),
        Box::new(markup_checks::EmptyHeadingsCheck),
        Box::new(markup_checks::IframeTitleCheck),
        Box::new(markup_checks::RedundantAltTextCheck),
    ]
}
