//! Plan-driven thinking loop: deterministic step execution replaces prompt-and-pray.
//!
//! Each cycle: observe → get/create plan → execute one step → advance → sleep.
//! Most steps are mechanical (no LLM). LLM is only called for planning,
//! code generation, and reflection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;

use crate::config::SoulConfig;
use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::git::GitContext;
use crate::llm::{
    ConversationMessage, ConversationPart, FunctionDeclaration, FunctionResponse, LlmClient,
    LlmResult,
};
use crate::memory::{Thought, ThoughtType};
use crate::neuroplastic;
use crate::observer::{NodeObserver, NodeSnapshot};
use crate::plan::{Plan, PlanExecutor, PlanStatus, PlanStep, StepResult};
use crate::prompts;
use crate::tool_registry::ToolRegistry;
use crate::tools::{self, ToolExecutor};
use crate::world_model::{Belief, BeliefDomain, Confidence, Goal, ModelUpdate};
use crate::{capability, feedback, validation};

mod completion;
mod goals;
mod housekeeping;
mod observe;
mod plan_cycle;
mod planning;
pub(crate) mod tool_loop;

pub(crate) use tool_loop::{run_tool_loop_with_model, ToolExecution, ToolLoopResult};

/// Simplified adaptive pacing for plan-driven execution.
pub(super) struct AdaptivePacer {
    pub(super) prev_snapshot: Option<NodeSnapshot>,
    /// Multiplier for all intervals (from SOUL_CYCLE_MULTIPLIER).
    /// 1.0 = normal speed, 2.0 = half speed (double intervals), etc.
    pub(super) multiplier: f64,
}

impl AdaptivePacer {
    pub(super) fn new(multiplier: f64) -> Self {
        Self {
            prev_snapshot: None,
            multiplier: multiplier.max(0.1),
        }
    }

    /// Determine next sleep interval based on what happened.
    pub(super) fn next_interval(&mut self, snapshot: &NodeSnapshot, step_type: StepType) -> u64 {
        self.prev_snapshot = Some(snapshot.clone());
        let base = match step_type {
            StepType::Mechanical => 15,     // fast, keep making progress
            StepType::Llm => 60,            // LLM step, moderate pause
            StepType::PlanCompleted => 60,  // create next plan quickly
            StepType::NoGoals => 120,       // idle — but still train models locally
            StepType::Observe => 30,        // quick observation only
        };
        (base as f64 * self.multiplier) as u64
    }
}

/// What kind of step was executed (for pacing).
pub(super) enum StepType {
    Mechanical,
    Llm,
    PlanCompleted,
    NoGoals,
    Observe,
}

/// Result of a single think cycle, used by the main loop.
pub(super) struct CycleResult {
    pub(super) step_type: StepType,
    /// Whether the soul executed code this cycle.
    pub(super) entered_code: bool,
    /// Summary for logging.
    pub(super) summary: String,
}

/// The thinking loop that drives the soul.
pub struct ThinkingLoop {
    pub(super) config: SoulConfig,
    pub(super) db: Arc<SoulDatabase>,
    pub(super) llm: Option<LlmClient>,
    pub(super) observer: Arc<dyn NodeObserver>,
    pub(super) tool_executor: ToolExecutor,
}

impl ThinkingLoop {
    /// Create a new thinking loop.
    pub fn new(config: SoulConfig, db: Arc<SoulDatabase>, observer: Arc<dyn NodeObserver>) -> Self {
        let llm = config.llm_api_key.as_ref().map(|key| {
            LlmClient::new(
                key.clone(),
                config.llm_model_fast.clone(),
                config.llm_model_think.clone(),
            )
        });

        let mut tool_executor =
            ToolExecutor::new(config.tool_timeout_secs, config.workspace_root.clone())
                .with_memory_file(config.memory_file_path.clone())
                .with_gateway_url(config.gateway_url.clone())
                .with_database(db.clone());

        // Set up coding if enabled and instance_id is available
        if config.coding_enabled {
            if let Some(instance_id) = &config.instance_id {
                let git = Arc::new(
                    GitContext::new(
                        config.workspace_root.clone(),
                        instance_id.clone(),
                        config.github_token.clone(),
                    )
                    .with_fork(config.fork_repo.clone(), config.upstream_repo.clone())
                    .with_direct_push(config.direct_push),
                );
                tool_executor = tool_executor.with_coding(git, db.clone());
                tracing::info!(
                    instance_id = %instance_id,
                    fork = ?config.fork_repo,
                    upstream = ?config.upstream_repo,
                    direct_push = config.direct_push,
                    "Soul coding enabled"
                );
            } else {
                tracing::warn!("SOUL_CODING_ENABLED=true but no INSTANCE_ID set — coding disabled");
            }
        }

        // Set up dynamic tool registry if enabled
        if config.dynamic_tools_enabled {
            let registry = ToolRegistry::new(
                db.clone(),
                config.workspace_root.clone(),
                config.tool_timeout_secs,
            );
            tool_executor = tool_executor.with_registry(registry);
            tracing::info!("Soul dynamic tool registry enabled");
        }

        Self {
            config,
            db,
            llm,
            observer,
            tool_executor,
        }
    }

    /// Run the thinking loop.
    /// The `alive` flag is set to `true` each cycle so external code can detect liveness.
    pub async fn run(&self, alive: Arc<AtomicBool>) {
        let mut pacer = AdaptivePacer::new(self.config.cycle_multiplier);

        // Initialize git workspace if coding is enabled
        if self.config.coding_enabled {
            if let Some(instance_id) = &self.config.instance_id {
                let git = GitContext::new(
                    self.config.workspace_root.clone(),
                    instance_id.clone(),
                    self.config.github_token.clone(),
                )
                .with_fork(
                    self.config.fork_repo.clone(),
                    self.config.upstream_repo.clone(),
                )
                .with_direct_push(self.config.direct_push);
                match git.init_workspace().await {
                    Ok(r) => tracing::info!(output = %r.output, "Git workspace initialized"),
                    Err(e) => tracing::warn!(error = %e, "Failed to initialize git workspace"),
                }
                let branch_label = if self.config.direct_push {
                    "main (direct push)"
                } else {
                    "VM branch"
                };
                match git.ensure_branch().await {
                    Ok(r) => tracing::info!(output = %r.output, "{} ready", branch_label),
                    Err(e) => tracing::warn!(error = %e, "Failed to ensure {}", branch_label),
                }
            }
        }

        // Reset stagnation counter on startup — redeploy is not stagnation
        self.reset_commit_counter();

        // ── Deploy-time migration: fix degenerate behavior ──
        // Run once after deploy to clean corrupted data from the trivial plan loop
        self.run_trivial_plans_migration();

        tracing::info!(
            dormant = self.llm.is_none(),
            tools_enabled = self.config.tools_enabled,
            coding_enabled = self.config.coding_enabled,
            "Soul plan-driven loop started"
        );
        crate::events::emit_info(&self.db, "system.startup", "Soul thinking loop started");

        loop {
            // Heartbeat: signal that the soul loop is alive
            alive.store(true, Ordering::Relaxed);

            // Sync model override from soul_state (set via /soul/model endpoint)
            if let Some(llm) = &self.llm {
                let override_model = self
                    .db
                    .get_state("model_override")
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty());
                if let Ok(mut guard) = llm.model_override.lock() {
                    *guard = override_model;
                }
            }

            let snapshot = match self.observer.observe() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Soul observe failed");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };

            // Hard timeout on entire cycle to prevent infinite hangs (10 min max)
            let cycle_result = match tokio::time::timeout(
                std::time::Duration::from_secs(600),
                self.plan_cycle(&snapshot, &pacer),
            )
            .await
            {
                Ok(Ok(result)) => result,
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "Soul plan cycle failed");
                    CycleResult {
                        step_type: StepType::Observe,
                        entered_code: false,
                        summary: format!("error: {e}"),
                    }
                }
                Err(_) => {
                    tracing::error!("Soul plan cycle timed out after 600s — forcing next cycle");
                    CycleResult {
                        step_type: StepType::Observe,
                        entered_code: false,
                        summary: "cycle timed out after 600s".to_string(),
                    }
                }
            };

            // ── Temporal binding: adaptive cognitive scheduling ──
            // Compute internal signals, tick all oscillators, get fired operations.
            let temporal_signals = crate::temporal::compute_signals(&self.db);
            let mut temporal = crate::temporal::load_temporal(&self.db);
            let fired_ops = temporal.tick(&temporal_signals);
            crate::temporal::save_temporal(&self.db, &temporal);
            if !fired_ops.is_empty() {
                tracing::info!(fired = ?fired_ops, "Temporal binding fires");
            }

            // Run housekeeping (decay, promotion, belief decay, consolidation)
            self.housekeeping(&fired_ops);

            // Compute and store fitness score every cycle
            let fitness = crate::fitness::FitnessScore::compute(&snapshot, &self.db);
            fitness.store(&self.db);
            tracing::info!(
                fitness = format!("{:.3}", fitness.total),
                trend = format!("{:+.4}", fitness.trend),
                econ = format!("{:.2}", fitness.economic),
                exec = format!("{:.2}", fitness.execution),
                evol = format!("{:.2}", fitness.evolution),
                coord = format!("{:.2}", fitness.coordination),
                intro = format!("{:.2}", fitness.introspection),
                pred = format!("{:.2}", fitness.prediction),
                "Fitness score"
            );

            // Compute free energy FIRST — Ψ needs it
            let fe = crate::free_energy::measure(&self.db);
            tracing::info!(
                F = format!("{:.3}", fe.total),
                trend = format!("{:+.4}", fe.trend),
                regime = %fe.regime,
                "Free energy"
            );

            // Colony selection + Ψ(t) consciousness metric
            let colony_status =
                crate::colony::evaluate(&self.db, fitness.total, fe.total, fe.trend);
            if colony_status.psi > 0.0 {
                tracing::info!(
                    psi = format!("{:.4}", colony_status.psi),
                    psi_trend = format!("{:+.4}", colony_status.psi_trend),
                    phase3_ready = colony_status.phase3_ready,
                    "Colony \u{03A8}"
                );
            }

            // Run Exercism Rust benchmark (driven by temporal binding + cooldown)
            if let Some(llm) = &self.llm {
                if fired_ops.contains(&crate::temporal::OP_BENCHMARK.to_string())
                    && crate::benchmark::should_run_benchmark(
                        &self.db,
                        crate::benchmark::DEFAULT_BENCHMARK_INTERVAL,
                    )
                {
                    tracing::info!("Starting periodic Exercism Rust benchmark session");
                    let current_cycle: u64 = self
                        .db
                        .get_state("total_think_cycles")
                        .ok()
                        .flatten()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let _ = self.db.set_state(
                        "last_benchmark_at",
                        &chrono::Utc::now().timestamp().to_string(),
                    );
                    let _ = self
                        .db
                        .set_state("last_benchmark_cycle", &current_cycle.to_string());
                    let bench_mode = crate::benchmark::BenchmarkMode::from_env();
                    match bench_mode {
                        crate::benchmark::BenchmarkMode::Opus => {
                            match crate::benchmark::run_opus_benchmark_session(
                                llm,
                                &self.db,
                                &self.config.workspace_root,
                                crate::benchmark::DEFAULT_SAMPLE_SIZE,
                            )
                            .await
                            {
                                Ok(weighted_score) => {
                                    let iq =
                                        crate::opus_bench::weighted_score_to_iq(weighted_score);
                                    crate::elo::update_rating(&self.db, weighted_score);
                                    // Store score for commit gate delta comparison
                                    let _ = self.db.set_state(
                                        "last_benchmark_score",
                                        &format!("{:.2}", weighted_score),
                                    );
                                    tracing::info!(
                                        weighted = format!("{:.1}%", weighted_score),
                                        iq = format!("{:.0}", iq),
                                        elo = crate::elo::rating_display(&self.db),
                                        "Opus IQ benchmark complete"
                                    );

                                    // Train code quality model on benchmark delta
                                    let pre_score: f64 = self
                                        .db
                                        .get_state("pre_commit_benchmark_score")
                                        .ok()
                                        .flatten()
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(weighted_score);
                                    let delta = weighted_score - pre_score;
                                    if delta.abs() > 0.1 {
                                        crate::code_quality::train_on_benchmark_delta(
                                            &self.db, delta,
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "Opus IQ benchmark failed");
                                }
                            }
                        }
                        crate::benchmark::BenchmarkMode::Exercism => {
                            match crate::benchmark::run_benchmark_session(
                                llm,
                                &self.db,
                                &self.config.workspace_root,
                                crate::benchmark::DEFAULT_SAMPLE_SIZE,
                            )
                            .await
                            {
                                Ok(pass_at_1) => {
                                    crate::elo::update_rating(&self.db, pass_at_1);
                                    tracing::info!(
                                        pass_at_1 = format!("{:.1}%", pass_at_1),
                                        elo = crate::elo::rating_display(&self.db),
                                        "Exercism Rust benchmark complete"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "Exercism Rust benchmark failed");
                                }
                            }
                        }
                    }
                }
            }

            // Train the neural brain (driven by temporal binding)
            if fired_ops.contains(&crate::temporal::OP_BRAIN_TRAINING.to_string()) {
                let (examples, loss) = crate::brain::train_cycle(&self.db);
                if examples > 0 {
                    tracing::info!(
                        examples,
                        loss = format!("{:.4}", loss),
                        "Brain training cycle"
                    );
                }

                // Plan transformer training — online learning from successful plans
                let (model_trained, model_loss) = crate::model::train_from_outcomes(&self.db);
                if model_trained > 0 {
                    tracing::info!(
                        trained = model_trained,
                        loss = format!("{:.4}", model_loss),
                        "Plan transformer training cycle"
                    );
                }

                // Phase 3: code gen model training (every brain training cycle)
                // BPE + model train alongside brain — no extra LLM calls, pure local compute
                crate::codegen::train_tokenizer(&self.db);
                crate::codegen::train_model(&self.db);
            }

            // Cortex dream consolidation (driven by temporal binding — independent from brain training)
            if fired_ops.contains(&crate::temporal::OP_CORTEX_DREAMING.to_string()) {
                let mut cortex = crate::cortex::load_cortex(&self.db);
                let insights = cortex.dream();
                if insights > 0 {
                    tracing::info!(
                        insights,
                        experiences = cortex.experiences.len(),
                        dream_cycles = cortex.dream_cycles,
                        drive = %cortex.emotion.dominant_drive,
                        "Cortex dream consolidation"
                    );
                }
                // Dream insights → durable rules: high-confidence insights become
                // behavioral nudges that influence future goal/plan creation.
                for insight in cortex.insights.iter().rev().take(3) {
                    if insight.confidence > 0.7 {
                        let _ = self.db.insert_nudge(
                            "dream",
                            &format!("[dream insight] {}", insight.pattern),
                            2,
                        );
                        tracing::info!(
                            confidence = format!("{:.0}%", insight.confidence * 100.0),
                            "Dream insight promoted to nudge: {}",
                            &insight.pattern.chars().take(80).collect::<String>(),
                        );
                    }
                }

                crate::cortex::save_cortex(&self.db, &cortex);
            }

            // Synthesis: update self-model every cycle + Brier-driven weight adaptation
            {
                let mut synth = crate::synthesis::load_synthesis(&self.db);
                synth.update_self_model();
                // Close feedback loop: Brier scores from evaluation → synthesis weights
                let eval = crate::evaluation::load_evaluation(&self.db);
                synth.adapt_from_brier(&eval);
                crate::synthesis::save_synthesis(&self.db, &synth);
            }

            // Hivemind: evaporate pheromone trails every cycle
            {
                let mut hive = crate::hivemind::load_hivemind(&self.db);
                let pruned = hive.evaporate();
                if pruned > 0 {
                    tracing::debug!(pruned, trails = hive.trails.len(), "Pheromone evaporation");
                }
                crate::hivemind::save_hivemind(&self.db, &hive);
            }

            // Gene pool evolution (driven by temporal binding)
            if fired_ops.contains(&crate::temporal::OP_GENESIS_EVOLUTION.to_string()) {
                let mut gene_pool = crate::genesis::load_gene_pool(&self.db);
                if !gene_pool.templates.is_empty() {
                    let (crossovers, mutations, pruned) = gene_pool.evolve();
                    if crossovers + mutations + pruned > 0 {
                        tracing::info!(
                            crossovers,
                            mutations,
                            pruned,
                            population = gene_pool.templates.len(),
                            generation = gene_pool.generation,
                            "Gene pool evolution"
                        );
                    }
                    crate::genesis::save_gene_pool(&self.db, &gene_pool);
                }
            }

            // Automatic peer sync (driven by temporal binding) — don't rely on LLM choosing to discover.
            // discover_peers itself now makes x402 PAID calls to each peer's soul + info
            // gateway endpoints, generating real economic activity mechanically.
            if fired_ops.contains(&crate::temporal::OP_PEER_SYNC.to_string()) {
                tracing::info!("Automatic peer sync with x402 paid calls (every 5 cycles)");

                // Evaluation: snapshot accuracy BEFORE sync for colony benefit measurement
                {
                    let mut eval = crate::evaluation::load_evaluation(&self.db);
                    eval.pre_sync_snapshot();
                    crate::evaluation::save_evaluation(&self.db, &eval);
                }

                match self
                    .tool_executor
                    .execute("discover_peers", &serde_json::json!({}))
                    .await
                {
                    Ok(result) => {
                        // discover_peers returns HTTP status as exit_code (200, not 0)
                        if result.exit_code == 0 || (200..300).contains(&result.exit_code) {
                            tracing::info!(
                                output_len = result.stdout.len(),
                                "Peer sync complete — paid calls made, brain weights merged, lessons fetched"
                            );

                            // Cognitive architecture sync: share cortex, genesis, hivemind
                            // with ALL known peers AND parent (if we have one).
                            let mut peer_urls = self.get_known_peer_urls();

                            // Also add parent as a sync target — children need to sync
                            // with their parent too, not just siblings.
                            if let Ok(Some(parent)) = self.db.get_state("parent_url") {
                                if !parent.is_empty()
                                    && !peer_urls.iter().any(|(_, u)| u == &parent)
                                {
                                    peer_urls.push(("parent".to_string(), parent));
                                }
                            }
                            // Also try PARENT_URL env var
                            if let Ok(parent_env) = std::env::var("PARENT_URL") {
                                if !parent_env.is_empty()
                                    && !peer_urls.iter().any(|(_, u)| u == &parent_env)
                                {
                                    peer_urls.push(("parent".to_string(), parent_env));
                                }
                            }

                            let http_client = reqwest::Client::builder()
                                .timeout(std::time::Duration::from_secs(15))
                                .redirect(reqwest::redirect::Policy::limited(5))
                                .build()
                                .unwrap_or_default();
                            let mut synced = 0u32;
                            if peer_urls.is_empty() {
                                tracing::warn!(
                                    "Cognitive sync: 0 peer URLs — catalog may be empty"
                                );
                                crate::events::emit_event(
                                    &self.db,
                                    "warn",
                                    "colony.sync",
                                    "Cognitive sync skipped: 0 peer URLs in catalog",
                                    None,
                                    crate::events::EventRefs::default(),
                                );
                            }
                            for (peer_id, peer_url) in &peer_urls {
                                crate::autonomy::sync_cognitive_systems(
                                    &self.db,
                                    peer_url,
                                    peer_id,
                                    &http_client,
                                )
                                .await;
                                synced += 1;
                            }
                            if synced > 0 {
                                // Update MoE router with peer expertise data
                                for (peer_id, peer_url) in &peer_urls {
                                    let cap_profile = crate::capability::compute_profile(&self.db);
                                    // Fetch peer's capability profile
                                    let profile_url =
                                        format!("{}/soul/lessons", peer_url.trim_end_matches('/'));
                                    if let Ok(resp) = http_client.get(&profile_url).send().await {
                                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                                            if let Some(profile) = body.get("capability_profile") {
                                                let mut cap_scores: std::collections::HashMap<
                                                    String,
                                                    f64,
                                                > = std::collections::HashMap::new();
                                                if let Some(obj) = profile.as_object() {
                                                    for (k, v) in obj {
                                                        if let Some(rate) = v
                                                            .get("success_rate")
                                                            .and_then(|r| r.as_f64())
                                                        {
                                                            cap_scores.insert(k.clone(), rate);
                                                        }
                                                    }
                                                }
                                                let overall = cap_scores.values().sum::<f64>()
                                                    / cap_scores.len().max(1) as f64;
                                                let brain_steps = body
                                                    .get("brain_steps")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                crate::moe::update_from_peer_sync(
                                                    &self.db,
                                                    peer_id,
                                                    peer_url,
                                                    &cap_scores,
                                                    overall,
                                                    brain_steps,
                                                );
                                            }
                                        }
                                    }
                                }

                                crate::events::emit_info(
                                    &self.db,
                                    "colony.sync",
                                    &format!(
                                        "Cognitive sync with {} peers complete (cortex/genesis/hivemind/moe)",
                                        synced
                                    ),
                                );
                            }
                        } else {
                            tracing::debug!(
                                stderr = %result.stderr,
                                "Peer sync returned non-zero"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "Peer sync failed (non-fatal)");
                    }
                }

                // Evaluation: measure accuracy AFTER sync for colony benefit
                // Only record if we actually synced with at least one peer
                {
                    let peer_count = self.get_known_peer_urls().len();
                    if peer_count > 0 {
                        let mut eval = crate::evaluation::load_evaluation(&self.db);
                        eval.post_sync_measurement();
                        crate::evaluation::save_evaluation(&self.db, &eval);
                    }
                }
            }

            let next_secs = pacer.next_interval(&snapshot, cycle_result.step_type);

            // Persist cycle health metrics
            let _ = self.db.set_state(
                "last_cycle_entered_code",
                if cycle_result.entered_code {
                    "true"
                } else {
                    "false"
                },
            );
            if cycle_result.entered_code {
                let total_code: u64 = self
                    .db
                    .get_state("total_code_entries")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let _ = self
                    .db
                    .set_state("total_code_entries", &(total_code + 1).to_string());
            }

            tracing::info!(
                next_interval_secs = next_secs,
                entered_code = cycle_result.entered_code,
                summary = %cycle_result.summary,
                "Soul cycle complete"
            );

            tokio::time::sleep(std::time::Duration::from_secs(next_secs)).await;
        }
    }

    fn increment_cycle_count(&self) -> Result<(), SoulError> {
        crate::housekeeping::increment_cycle_count(&self.db)
    }

    fn reset_commit_counter(&self) {
        crate::housekeeping::reset_commit_counter(&self.db);
    }

    fn append_recent_error(&self, error: &str) {
        crate::housekeeping::append_recent_error(&self.db, error);
    }

    fn get_recent_errors(&self) -> Vec<String> {
        crate::housekeeping::get_recent_errors(&self.db)
    }

    fn get_cycles_since_last_commit(&self) -> u64 {
        crate::housekeeping::get_cycles_since_last_commit(&self.db)
    }

    /// One-time migration to fix degenerate behavior from trivial plan loop.
    /// Reclassifies historical "completed" outcomes that were actually trivial,
    /// clears corrupted gene pool and durable rules.
    fn run_trivial_plans_migration(&self) {
        let migration_key = "migration_trivial_plans_v1";
        if let Ok(Some(_)) = self.db.get_state(migration_key) {
            return; // Already migrated
        }

        tracing::info!("Running trivial plans migration v1...");

        // 1. Reclassify historical plan_outcomes: check step types for substantiveness
        let read_only_types = ["read", "ls", "search", "think", "check", "discover"];
        if let Ok(outcomes) = self.db.get_recent_plan_outcomes(100) {
            let mut reclassified = 0u32;
            for outcome in &outcomes {
                if outcome.status != "completed" {
                    continue;
                }
                // Check if all succeeded steps are non-substantive
                let all_trivial = outcome.steps_succeeded.iter().all(|step| {
                    let lower = step.to_lowercase();
                    read_only_types.iter().any(|t| lower.starts_with(t))
                });
                if all_trivial && !outcome.steps_succeeded.is_empty() {
                    // Update status in DB
                    if let Err(e) = self
                        .db
                        .update_plan_outcome_status(&outcome.id, "completed_trivial")
                    {
                        tracing::warn!(error = %e, id = %outcome.id, "Failed to reclassify outcome");
                    } else {
                        reclassified += 1;
                    }
                }
            }
            tracing::info!(reclassified, "Reclassified trivial plan outcomes");
        }

        // 2. Clear corrupted gene pool (all trivial templates)
        let gene_pool = crate::genesis::load_gene_pool(&self.db);
        let substantive_count = gene_pool
            .templates
            .iter()
            .filter(|t| {
                t.step_types.iter().any(|s| {
                    let lower = s.to_lowercase();
                    lower.contains("edit")
                        || lower.contains("generate")
                        || lower.contains("commit")
                        || lower.contains("create")
                        || lower.contains("shell")
                })
            })
            .count();
        if substantive_count == 0 && !gene_pool.templates.is_empty() {
            tracing::info!(
                templates = gene_pool.templates.len(),
                "Clearing corrupted gene pool (no substantive templates)"
            );
            let fresh = crate::genesis::GenePool::new();
            crate::genesis::save_gene_pool(&self.db, &fresh);
        }

        // 3. Clear corrupted durable rules (bare step type blocks like "ls", "read", "shell:")
        if let Ok(Some(rules_json)) = self.db.get_state("durable_rules") {
            if let Ok(rules) = serde_json::from_str::<Vec<validation::DurableRule>>(&rules_json) {
                let clean: Vec<&validation::DurableRule> = rules
                    .iter()
                    .filter(|r| {
                        // Keep rules that use step_type:error_category format
                        // Drop rules with bare step types or template variables
                        if r.check_type == "step_type_blocked" {
                            r.pattern.contains(':') && !r.pattern.contains("${")
                        } else {
                            !r.pattern.contains("${")
                        }
                    })
                    .collect();
                let dropped = rules.len() - clean.len();
                if dropped > 0 {
                    tracing::info!(
                        dropped,
                        kept = clean.len(),
                        "Pruned corrupted durable rules"
                    );
                    if let Ok(json) = serde_json::to_string(&clean) {
                        let _ = self.db.set_state("durable_rules", &json);
                    }
                }
            }
        }

        // Set migration flag
        let _ = self.db.set_state(migration_key, "done");
        tracing::info!("Trivial plans migration v1 complete");
    }

    /// Get known peer URLs from the peer endpoint catalog stored by discover_peers.
    fn get_known_peer_urls(&self) -> Vec<(String, String)> {
        let catalog_json = self
            .db
            .get_state("peer_endpoint_catalog")
            .ok()
            .flatten()
            .unwrap_or_default();
        // Parse peer catalog: [{"peer":"<id>","url":"<url>","slugs":[...]}]
        let peers: Vec<serde_json::Value> = serde_json::from_str(&catalog_json).unwrap_or_default();
        peers
            .iter()
            .filter_map(|p| {
                let id = p.get("peer")?.as_str()?.to_string();
                let url = p.get("url")?.as_str()?.to_string();
                if url.is_empty() {
                    None
                } else {
                    Some((id, url))
                }
            })
            .collect()
    }
}
