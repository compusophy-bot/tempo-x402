//! Housekeeping helpers extracted from the thinking loop.
use super::*;

impl ThinkingLoop {
    pub(super) fn housekeeping(&self, fired_ops: &[String]) {
        crate::housekeeping::housekeeping(
            &self.db,
            self.config.prune_threshold,
            &self.config.workspace_root,
            fired_ops,
        );
    }
}

/// Extract the first JSON array from text, returning (json_str, text_before, text_after).
pub(super) fn extract_json_array(text: &str) -> Option<(String, String, String)> {
    crate::normalize::extract_json_array(text)
}
