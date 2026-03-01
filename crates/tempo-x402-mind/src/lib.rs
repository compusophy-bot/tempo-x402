//! x402-mind: cognitive architecture for autonomous nodes.
//!
//! # Current State (v1.0)
//!
//! Implements a dual-soul architecture: two `ThinkingLoop` instances with different
//! configurations connected by a `Callosum` integration bus. Left hemisphere gets
//! coding tools (System 1, fast), right gets observe-only (System 2, slow).
//!
//! When `MIND_ENABLED=false` (default), the node falls back to a single soul.
//!
//! # Architectural Direction
//!
//! The current forced lateralization (labeling hemispheres "left" and "right" with
//! hardcoded roles) is **imposed structure, not emergent behavior**. Within a single
//! entity, specialization should arise from experience and environment, not labels.
//!
//! The vision for v2:
//!
//! - **Single soul per node** with the capacity for multiple modes of thought.
//!   The soul naturally alternates between focused/analytical and broad/holistic
//!   processing based on what it encounters — not because we tell it which to be.
//!
//! - **Emergent specialization across nodes**, not within them. When multiple nodes
//!   exist in different environments with different experiences, they naturally
//!   develop different capabilities and perspectives. That's real lateralization.
//!
//! - **Multi-stream thinking**: instead of one linear loop (observe → think → act),
//!   the soul could spawn concurrent thought processes that diverge (explore
//!   different hypotheses) and converge (synthesize findings). These would be
//!   dynamic, not predetermined — born from the soul's own decisions about what
//!   needs deeper investigation.
//!
//! - **The callosum becomes cross-node integration**: sharing insights between
//!   separate autonomous entities, not between forced roles within one.
//!
//! The pieces that ARE worth keeping: `WorkingMemory` (ephemeral ring buffer),
//! memory consolidation, the `Notify`-based cross-wake mechanism, and the
//! thought-type epistemology (Observation, Reasoning, Decision, etc.).

pub mod callosum;
pub mod config;
pub mod consolidation;
pub mod hemisphere;
pub mod memory;

pub use config::MindConfig;
pub use hemisphere::{HemisphereConfig, HemisphereRole};

use std::sync::Arc;
use tokio::task::JoinHandle;

use x402_soul::{NodeObserver, Soul, SoulDatabase, SoulError};

use crate::callosum::Callosum;
use crate::memory::WorkingMemory;

/// Handle to the running mind — wraps join handles for both hemispheres + callosum.
pub struct MindHandle {
    pub left_handle: JoinHandle<()>,
    pub right_handle: JoinHandle<()>,
    pub callosum_handle: JoinHandle<()>,
}

/// The Mind: owns two souls and the integration bus between them.
pub struct Mind {
    left: Soul,
    right: Soul,
    left_working_memory: Arc<WorkingMemory>,
    right_working_memory: Arc<WorkingMemory>,
    callosum: Callosum,
    /// The primary database (left's DB, or shared DB).
    primary_db: Arc<SoulDatabase>,
    /// The right hemisphere's DB (same as primary if shared).
    right_db: Arc<SoulDatabase>,
}

impl Mind {
    /// Create a new Mind from config. Opens soul databases for both hemispheres.
    pub fn new(config: MindConfig) -> Result<Self, SoulError> {
        let left = Soul::new(config.left.soul_config.clone())?;
        let right = Soul::new(config.right.soul_config.clone())?;

        let primary_db = left.database().clone();
        let right_db = if config.shared_db {
            primary_db.clone()
        } else {
            right.database().clone()
        };

        let left_working_memory = Arc::new(WorkingMemory::new(20));
        let right_working_memory = Arc::new(WorkingMemory::new(20));

        let callosum = Callosum::new(
            primary_db.clone(),
            right_db.clone(),
            config.integration_interval_secs,
            config.escalation_threshold,
        );

        Ok(Self {
            left,
            right,
            left_working_memory,
            right_working_memory,
            callosum,
            primary_db,
            right_db,
        })
    }

    /// Spawn both hemispheres and the callosum as background tokio tasks.
    pub fn spawn(self, observer: Arc<dyn NodeObserver>) -> MindHandle {
        let left_observer = observer.clone();
        let right_observer = observer;

        let left_handle = self.left.spawn(left_observer);
        let right_handle = self.right.spawn(right_observer);
        let callosum_handle = self.callosum.spawn();

        tracing::info!("Mind spawned: left + right hemispheres + callosum");

        MindHandle {
            left_handle,
            right_handle,
            callosum_handle,
        }
    }

    /// Get the primary (left/shared) database for external queries.
    pub fn database(&self) -> &Arc<SoulDatabase> {
        &self.primary_db
    }

    /// Get the right hemisphere's database.
    pub fn right_database(&self) -> &Arc<SoulDatabase> {
        &self.right_db
    }

    /// Get the left hemisphere's working memory.
    pub fn left_working_memory(&self) -> &Arc<WorkingMemory> {
        &self.left_working_memory
    }

    /// Get the right hemisphere's working memory.
    pub fn right_working_memory(&self) -> &Arc<WorkingMemory> {
        &self.right_working_memory
    }
}
