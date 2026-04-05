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

