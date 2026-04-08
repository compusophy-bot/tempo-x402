//! Cognitive Cartridge Orchestrator — route cognitive calls through WASM cartridges.
//!
//! Instead of calling compiled cognitive functions directly (brain::predict, genesis::evolve),
//! the orchestrator routes requests to hot-swappable WASM cartridges via the existing
//! cartridge engine. If a cognitive cartridge isn't loaded, falls back to compiled code.
//!
//! This enables runtime self-modification: the agent can rewrite a cognitive system,
//! compile it to WASM (20 seconds), and hot-swap it without restart or state loss.
//!
//! ## Architecture
//!
//! ```text
//! Thinking Loop
//!   → CognitiveOrchestrator::predict("brain", request)
//!     → Check: is "cognitive-brain" loaded in cartridge engine?
//!       → YES: engine.execute("cognitive-brain", json_request) → parse response
//!       → NO:  fall back to compiled brain::predict_step()
//! ```
//!
//! ## Cartridge Naming Convention
//!
//! Cognitive cartridges use the prefix `cognitive-`:
//! - `cognitive-brain` — step success prediction
//! - `cognitive-cortex` — world model and causal reasoning
//! - `cognitive-genesis` — plan template evolution
//! - `cognitive-hivemind` — pheromone trail coordination
//! - `cognitive-synthesis` — metacognitive unification
//! - `cognitive-unified` — the unified encoder-decoder model

use std::sync::Arc;

/// Orchestrator that routes cognitive calls to cartridges or compiled fallbacks.
pub struct CognitiveOrchestrator {
    /// Reference to the shared cartridge engine (if available).
    engine: Option<Arc<x402_cartridge::CartridgeEngine>>,
}

impl CognitiveOrchestrator {
    /// Create a new orchestrator. If no engine is provided, all calls fall back to compiled code.
    pub fn new(engine: Option<Arc<x402_cartridge::CartridgeEngine>>) -> Self {
        Self { engine }
    }

    /// Check if a cognitive cartridge is loaded and available.
    pub fn has_cartridge(&self, system: &str) -> bool {
        let slug = format!("cognitive-{system}");
        self.engine
            .as_ref()
            .map(|e| e.loaded_slugs().contains(&slug))
            .unwrap_or(false)
    }

    /// List all loaded cognitive cartridges.
    pub fn loaded_systems(&self) -> Vec<String> {
        self.engine
            .as_ref()
            .map(|e| {
                e.loaded_slugs()
                    .into_iter()
                    .filter(|s| s.starts_with("cognitive-"))
                    .map(|s| s.strip_prefix("cognitive-").unwrap_or(&s).to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Execute a cognitive request against a cartridge.
    /// Returns the response body as a JSON value, or None if the cartridge isn't loaded.
    pub fn execute(
        &self,
        system: &str,
        request: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let engine = self.engine.as_ref()?;
        let slug = format!("cognitive-{system}");

        if !engine.loaded_slugs().contains(&slug) {
            return None;
        }

        let body = serde_json::to_string(request).ok()?;
        let cart_request = x402_cartridge::CartridgeRequest {
            method: "POST".to_string(),
            path: "/".to_string(),
            body,
            headers: std::collections::HashMap::new(),
            payment: None,
        };

        match engine.execute(&slug, &cart_request, std::collections::HashMap::new(), 30) {
            Ok(result) if result.status == 200 => {
                serde_json::from_str(&result.body).ok()
            }
            Ok(result) => {
                tracing::debug!(
                    system,
                    status = result.status,
                    "Cognitive cartridge returned non-200"
                );
                None
            }
            Err(e) => {
                tracing::debug!(system, error = %e, "Cognitive cartridge execution failed");
                None
            }
        }
    }

    /// Hot-swap a cognitive cartridge: unload old, load new.
    /// Returns Ok(()) if successful, Err with reason if not.
    pub fn hot_swap(
        &self,
        system: &str,
        wasm_path: &std::path::Path,
    ) -> Result<(), String> {
        let engine = self.engine.as_ref().ok_or("no cartridge engine")?;
        let slug = format!("cognitive-{system}");

        // Unload existing (if any)
        engine.unload_module(&slug);

        // Load new
        engine
            .load_module(&slug, wasm_path)
            .map_err(|e| format!("failed to load {slug}: {e}"))?;

        tracing::info!(
            system,
            path = %wasm_path.display(),
            "Cognitive cartridge hot-swapped"
        );

        Ok(())
    }

    /// Rollback: unload a cognitive cartridge, forcing fallback to compiled code.
    pub fn rollback(&self, system: &str) {
        if let Some(engine) = &self.engine {
            let slug = format!("cognitive-{system}");
            engine.unload_module(&slug);
            tracing::info!(system, "Cognitive cartridge rolled back to compiled fallback");
        }
    }
}

/// Status info for the /soul/status API response.
pub fn status(orchestrator: &CognitiveOrchestrator) -> serde_json::Value {
    let loaded = orchestrator.loaded_systems();
    serde_json::json!({
        "cognitive_cartridges": loaded,
        "hot_swappable": true,
        "systems": [
            "brain", "cortex", "genesis", "hivemind",
            "synthesis", "unified"
        ],
        "active_cartridges": loaded.len(),
    })
}
