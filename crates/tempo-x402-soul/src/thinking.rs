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

/// Simplified adaptive pacing for plan-driven execution.
struct AdaptivePacer {
    prev_snapshot: Option<NodeSnapshot>,
    /// Multiplier for all intervals (from SOUL_CYCLE_MULTIPLIER).
    /// 1.0 = normal speed, 2.0 = half speed (double intervals), etc.
    multiplier: f64,
}

impl AdaptivePacer {
    fn new(multiplier: f64) -> Self {
        Self {
            prev_snapshot: None,
            multiplier: multiplier.max(0.1),
        }
    }

    /// Determine next sleep interval based on what happened.
    fn next_interval(&mut self, snapshot: &NodeSnapshot, step_type: StepType) -> u64 {
        self.prev_snapshot = Some(snapshot.clone());
        let base = match step_type {
            StepType::Mechanical => 30,     // fast, keep making progress
            StepType::Llm => 120,           // LLM step, moderate pause
            StepType::PlanCompleted => 300, // time to create next plan
            StepType::NoGoals => 600,       // idle
            StepType::Observe => 60,        // quick observation only
        };
        (base as f64 * self.multiplier) as u64
    }
}

/// What kind of step was executed (for pacing).
enum StepType {
    Mechanical,
    Llm,
    PlanCompleted,
    NoGoals,
    Observe,
}

/// Result of a single think cycle, used by the main loop.
struct CycleResult {
    step_type: StepType,
    /// Whether the soul executed code this cycle.
    entered_code: bool,
    /// Summary for logging.
    summary: String,
}

/// The thinking loop that drives the soul.
pub struct ThinkingLoop {
    config: SoulConfig,
    db: Arc<SoulDatabase>,
    llm: Option<LlmClient>,
    observer: Arc<dyn NodeObserver>,
    tool_executor: ToolExecutor,
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

            // Run housekeeping (decay, promotion, belief decay, consolidation)
            self.housekeeping();

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
                "Fitness score"
            );

            // Run Exercism Rust benchmark periodically (every 100 cycles)
            if let Some(llm) = &self.llm {
                if crate::benchmark::should_run_benchmark(
                    &self.db,
                    crate::benchmark::DEFAULT_BENCHMARK_INTERVAL,
                ) {
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

            // Train the neural brain every 10 cycles
            let cycle_count: u64 = self
                .db
                .get_state("total_think_cycles")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if cycle_count % 10 == 0 {
                let (examples, loss) = crate::brain::train_cycle(&self.db);
                if examples > 0 {
                    tracing::info!(
                        examples,
                        loss = format!("{:.4}", loss),
                        "Brain training cycle"
                    );
                }
            }

            // Automatic peer sync every 5 cycles — don't rely on LLM choosing to discover.
            // discover_peers itself now makes x402 PAID calls to each peer's soul + info
            // gateway endpoints, generating real economic activity mechanically.
            if cycle_count > 0 && cycle_count % 5 == 0 {
                tracing::info!("Automatic peer sync with x402 paid calls (every 5 cycles)");
                match self
                    .tool_executor
                    .execute("discover_peers", &serde_json::json!({}))
                    .await
                {
                    Ok(result) => {
                        if result.exit_code == 0 {
                            tracing::info!(
                                output_len = result.stdout.len(),
                                "Peer sync complete — paid calls made, brain weights merged, lessons fetched"
                            );
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

    /// Execute one plan-driven cycle.
    async fn plan_cycle(
        &self,
        snapshot: &NodeSnapshot,
        pacer: &AdaptivePacer,
    ) -> Result<CycleResult, SoulError> {
        // ── Step 1: Observe — record snapshot, sync auto-beliefs ──
        self.observe(snapshot, pacer)?;

        // If dormant (no API key), stop here
        let llm = match &self.llm {
            Some(g) => g,
            None => {
                tracing::debug!("Soul dormant — observation recorded");
                self.increment_cycle_count()?;
                return Ok(CycleResult {
                    step_type: StepType::Observe,
                    entered_code: false,
                    summary: "dormant".to_string(),
                });
            }
        };

        // ── Read nudges (external signals) ──
        let nudges = self.db.get_unprocessed_nudges(5).unwrap_or_default();
        if !nudges.is_empty() {
            tracing::info!(count = nudges.len(), "Processing nudges");
        }

        // ── Check for pending-approval plan first ──
        if let Ok(Some(pending)) = self.db.get_pending_approval_plan() {
            // Check if approval has timed out
            let age_mins = (chrono::Utc::now().timestamp() - pending.created_at).max(0) as u64 / 60;
            if age_mins >= self.config.plan_approval_timeout_mins {
                tracing::info!(
                    plan_id = %pending.id,
                    age_mins,
                    "Plan approval timed out — auto-approving"
                );
                let _ = self.db.approve_plan(&pending.id);
                // Fall through to pick it up as active
            } else {
                tracing::debug!(
                    plan_id = %pending.id,
                    age_mins,
                    "Plan awaiting approval — skipping execution"
                );
                self.increment_cycle_count()?;
                return Ok(CycleResult {
                    step_type: StepType::Observe,
                    entered_code: false,
                    summary: format!("plan {} awaiting approval ({age_mins}m)", pending.id),
                });
            }
        }

        // ── Step 2: Get or create plan ──
        let mut plan = match self.db.get_active_plan()? {
            Some(plan) => plan,
            None => {
                // No active plan — try to create one
                match self.create_plan(llm, snapshot, &nudges).await? {
                    Some(plan) => {
                        // Mark nudges as processed after they've influenced plan creation
                        for nudge in &nudges {
                            let _ = self.db.mark_nudge_processed(&nudge.id);
                        }
                        // If plan requires approval, don't execute it yet
                        if plan.status == PlanStatus::PendingApproval {
                            tracing::info!(
                                plan_id = %plan.id,
                                "Plan created — awaiting approval"
                            );
                            self.increment_cycle_count()?;
                            return Ok(CycleResult {
                                step_type: StepType::Observe,
                                entered_code: false,
                                summary: format!("plan {} created, awaiting approval", plan.id),
                            });
                        }
                        plan
                    }
                    None => {
                        // No goals either — create goals
                        self.create_goals(llm, snapshot, &nudges).await?;
                        // Mark nudges as processed after they've influenced goal creation
                        for nudge in &nudges {
                            let _ = self.db.mark_nudge_processed(&nudge.id);
                        }
                        self.increment_cycle_count()?;
                        return Ok(CycleResult {
                            step_type: StepType::NoGoals,
                            entered_code: false,
                            summary: "created goals, will plan next cycle".to_string(),
                        });
                    }
                }
            }
        };

        // ── Stagnation checks ──

        // Circuit breaker 1: global stagnation — 50+ cycles without a commit or plan completion
        // Also count completed plans and peer reviews as "progress" — not just commits.
        // Agents doing peer review, coordination, and research are productive even without commits.
        let cycles_since_commit = self.get_cycles_since_last_commit();
        let has_recent_completions = {
            let recent_outcomes = self.db.get_recent_plan_outcomes(5).unwrap_or_default();
            recent_outcomes.iter().any(|o| {
                o.status == "completed" && o.created_at > chrono::Utc::now().timestamp() - 3600
            })
        };
        let effective_stagnation_threshold = if has_recent_completions { 80 } else { 50 };
        if cycles_since_commit > effective_stagnation_threshold {
            tracing::warn!(
                cycles_since_commit,
                "Global stagnation — abandoning all goals"
            );
            let abandoned = self.db.abandon_all_active_goals().unwrap_or(0);
            plan.status = PlanStatus::Failed;
            let _ = self.db.update_plan(&plan);
            let _ = self.db.set_state("active_plan_id", "");
            // Record outcome so agents learn from stagnation
            let goal_desc = self
                .db
                .get_goal(&plan.goal_id)
                .ok()
                .flatten()
                .map(|g| g.description.clone())
                .unwrap_or_else(|| "unknown goal".to_string());
            let stag_err =
                format!("{cycles_since_commit} cycles without commit — global stagnation");
            feedback::record_outcome(&self.db, &plan, &goal_desc, Some(&stag_err));
            crate::events::emit_event(
                &self.db,
                "error",
                "system.stagnation",
                &format!("Global stagnation: {stag_err}. {abandoned} goals abandoned."),
                Some(serde_json::json!({
                    "cycles_since_commit": cycles_since_commit,
                    "goals_abandoned": abandoned,
                })),
                crate::events::EventRefs {
                    plan_id: Some(plan.id.clone()),
                    goal_id: Some(plan.goal_id.clone()),
                    ..Default::default()
                },
            );
            let _ = self.db.insert_nudge(
                "stagnation",
                &format!("{cycles_since_commit} cycles without progress. All {abandoned} goals reset. Try a completely different approach."),
                4,
            );
            self.reset_commit_counter();
            self.increment_cycle_count()?;
            return Ok(CycleResult {
                step_type: StepType::Observe,
                entered_code: false,
                summary: format!("stagnation reset after {cycles_since_commit} idle cycles"),
            });
        }

        // Circuit breaker 2: goal has failed too many times
        if let Ok(Some(goal)) = self.db.get_goal(&plan.goal_id) {
            if goal.retry_count >= 2 {
                tracing::warn!(
                    goal_id = %plan.goal_id,
                    retry_count = goal.retry_count,
                    "Goal failed too many times — abandoning"
                );
                let _ = self.db.update_goal(
                    &plan.goal_id,
                    Some("abandoned"),
                    None,
                    Some(chrono::Utc::now().timestamp()),
                );
                plan.status = PlanStatus::Failed;
                let _ = self.db.update_plan(&plan);
                let _ = self.db.set_state("active_plan_id", "");
                // Record outcome so agents learn from repeated goal failures
                let retry_err = format!("Goal failed {} times — abandoned", goal.retry_count);
                feedback::record_outcome(&self.db, &plan, &goal.description, Some(&retry_err));
                crate::events::emit_event(
                    &self.db,
                    "warn",
                    "goal.abandoned",
                    &format!(
                        "Goal abandoned after {} retries: {}",
                        goal.retry_count, goal.description
                    ),
                    Some(serde_json::json!({"retry_count": goal.retry_count})),
                    crate::events::EventRefs {
                        plan_id: Some(plan.id.clone()),
                        goal_id: Some(plan.goal_id.clone()),
                        ..Default::default()
                    },
                );
                let desc_preview: String = goal.description.chars().take(80).collect();
                let _ = self.db.insert_nudge(
                    "stagnation",
                    &format!(
                        "Goal '{}' failed {} times. Try a different approach.",
                        desc_preview, goal.retry_count
                    ),
                    3,
                );
                self.increment_cycle_count()?;
                return Ok(CycleResult {
                    step_type: StepType::Observe,
                    entered_code: false,
                    summary: format!("abandoned goal after {} retries", goal.retry_count),
                });
            }
        }

        // ── Step 3: Execute steps ──
        // Batch consecutive mechanical steps in a single cycle (no 30s gap between reads).
        // Stop when: plan done, LLM step executed, or a step fails.
        if plan.current_step >= plan.steps.len() {
            return self.complete_plan(llm, &mut plan).await;
        }

        let executor = PlanExecutor::new(&self.tool_executor, llm, &self.config, &self.db);
        let mut steps_executed = 0u32;
        let mut last_step_summary = String::new();
        let mut last_was_llm = false;
        let mut entered_code = false;
        let mut consecutive_failures: u32 = 0;
        const MAX_BATCH: u32 = 10; // cap mechanical batch to avoid runaway

        loop {
            if plan.current_step >= plan.steps.len() {
                // All steps done — reflect and complete
                if steps_executed > 0 {
                    // Record batch progress before completing
                    self.record_step_progress(&plan, &format!("{steps_executed} steps batched"));
                }
                return self.complete_plan(llm, &mut plan).await;
            }

            let step = plan.steps[plan.current_step].clone();
            let step_summary = step.summary();
            let is_llm = step.needs_llm();
            let is_code = matches!(
                step,
                PlanStep::GenerateCode { .. }
                    | PlanStep::EditCode { .. }
                    | PlanStep::Commit { .. }
                    | PlanStep::CargoCheck { .. }
            );

            // If we already ran an LLM step, stop — don't batch LLM steps
            if last_was_llm {
                break;
            }
            // If we've batched enough mechanical steps, stop
            if steps_executed >= MAX_BATCH && !is_llm {
                break;
            }

            // Brain prediction: estimate success probability before execution
            let cycle_count: u64 = self
                .db
                .get_state("total_think_cycles")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let brain_ctx = crate::brain::StepContext {
                plan_progress: if plan.steps.is_empty() {
                    0.0
                } else {
                    plan.current_step as f32 / plan.steps.len() as f32
                },
                replan_count: plan.replan_count,
                overall_success_rate: crate::capability::compute_profile(&self.db)
                    .overall_success_rate as f32,
                capability_success_rate: {
                    let cap = crate::capability::Capability::from_step(&step);
                    let profile = crate::capability::compute_profile(&self.db);
                    profile
                        .capabilities
                        .iter()
                        .find(|s| s.capability == cap.as_str())
                        .map(|s| s.success_rate as f32)
                        .unwrap_or(0.5)
                },
                consecutive_failures,
                cycle_count: cycle_count,
                ..Default::default()
            };
            let prediction = crate::brain::predict_step(&self.db, &step, &brain_ctx);

            // ── Brain-gated execution ──
            // Instead of just logging brain predictions, actually gate execution.
            // If brain has enough training data and predicts very low success,
            // skip the step and force a replan.
            let (should_execute, gate_reason) =
                validation::brain_gate_step(&self.db, &step, &prediction);

            if let Some(ref reason) = gate_reason {
                if should_execute {
                    tracing::warn!(
                        plan_id = %plan.id,
                        step = plan.current_step,
                        step_type = %step_summary,
                        "{reason}"
                    );
                } else {
                    tracing::warn!(
                        plan_id = %plan.id,
                        step = plan.current_step,
                        step_type = %step_summary,
                        "Brain GATED step — forcing replan: {reason}"
                    );
                }
            }

            if !should_execute {
                crate::events::emit_event(
                    &self.db,
                    "warn",
                    "brain.gate.blocked",
                    &format!(
                        "Brain gated step: {}",
                        gate_reason.as_deref().unwrap_or("low confidence")
                    ),
                    Some(serde_json::json!({"step": step_summary})),
                    crate::events::EventRefs {
                        plan_id: Some(plan.id.clone()),
                        goal_id: Some(plan.goal_id.clone()),
                        step_index: Some(plan.current_step as i32),
                        ..Default::default()
                    },
                );
                // Record the gate event as a capability failure
                capability::record_step_result(
                    &self.db,
                    &step,
                    false,
                    &format!(
                        "brain-gated: {}",
                        gate_reason.as_deref().unwrap_or("low confidence")
                    ),
                );
                // Force replan
                return self
                    .handle_step_failure(
                        llm,
                        &mut plan,
                        &step_summary,
                        &format!("Brain-gated: {}", gate_reason.unwrap_or_default()),
                    )
                    .await;
            }

            tracing::info!(
                plan_id = %plan.id,
                step = plan.current_step,
                total_steps = plan.steps.len(),
                step_type = %step_summary,
                batch_pos = steps_executed,
                brain_success_prob = format!("{:.1}%", prediction.success_prob * 100.0),
                "Executing plan step"
            );

            let result = executor.execute_step(&step, &plan.context).await;

            // ── Handle result ──
            match result {
                StepResult::Success(output) => {
                    // Track capability success
                    capability::record_step_result(&self.db, &step, true, &step_summary);

                    if let Some(key) = step.store_key() {
                        let truncated = if output.len() > 8000 {
                            let mut end = 8000;
                            while end > 0 && !output.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...(truncated)", &output[..end])
                        } else {
                            output.clone()
                        };
                        plan.context.insert(key.to_string(), truncated);
                    }
                    plan.current_step += 1;
                    self.db.update_plan(&plan)?;

                    if matches!(step, PlanStep::Commit { .. }) {
                        self.reset_commit_counter();
                    }

                    tracing::info!(
                        step = %step_summary,
                        output_len = output.len(),
                        "Step succeeded"
                    );

                    // Online brain learning — immediate feedback, not just batch
                    crate::brain::train_on_step(&self.db, &step, true, None, &brain_ctx);

                    consecutive_failures = 0; // reset on success
                    last_step_summary = step_summary;
                    last_was_llm = is_llm;
                    if is_code {
                        entered_code = true;
                    }
                    steps_executed += 1;

                    // After an LLM step, always stop (give it a pause)
                    if is_llm {
                        break;
                    }
                    // Mechanical step succeeded — continue to next step in same cycle
                }
                StepResult::Failed(error) => {
                    consecutive_failures += 1;
                    tracing::warn!(step = %step_summary, error = %error, consecutive_failures, "Step failed");
                    // Track capability failure
                    capability::record_step_result(&self.db, &step, false, &error);
                    // Online brain learning — learn from failures immediately
                    let err_cat = crate::feedback::classify_error(&error);
                    crate::brain::train_on_step(&self.db, &step, false, Some(err_cat), &brain_ctx);

                    // Record failure chain for causal reasoning
                    let goal_desc = self
                        .db
                        .get_goal(&plan.goal_id)
                        .ok()
                        .flatten()
                        .map(|g| g.description.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    validation::record_failure_chain(
                        &self.db,
                        &goal_desc,
                        &step,
                        &error,
                        plan.replan_count,
                    );

                    return self
                        .handle_step_failure(llm, &mut plan, &step_summary, &error)
                        .await;
                }
                StepResult::NeedsReplan(reason) => {
                    tracing::info!(step = %step_summary, reason = %reason, "Step needs replan");
                    // Track capability failure
                    capability::record_step_result(&self.db, &step, false, &reason);
                    // Online brain learning
                    let err_cat = crate::feedback::classify_error(&reason);
                    crate::brain::train_on_step(&self.db, &step, false, Some(err_cat), &brain_ctx);

                    // Record failure chain for causal reasoning
                    let goal_desc = self
                        .db
                        .get_goal(&plan.goal_id)
                        .ok()
                        .flatten()
                        .map(|g| g.description.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    validation::record_failure_chain(
                        &self.db,
                        &goal_desc,
                        &step,
                        &reason,
                        plan.replan_count,
                    );

                    return self
                        .handle_step_failure(llm, &mut plan, &step_summary, &reason)
                        .await;
                }
            }
        }

        // Record batch progress as a single thought
        let goal_desc = self
            .db
            .get_goal(&plan.goal_id)
            .ok()
            .flatten()
            .map(|g| g.description.clone());
        let progress_content = if steps_executed > 1 {
            format!(
                "[steps ..{}/{}] {} — batched {} mechanical steps, last: {}",
                plan.current_step,
                plan.steps.len(),
                goal_desc.as_deref().unwrap_or("plan"),
                steps_executed,
                last_step_summary,
            )
        } else {
            format!(
                "[step {}/{}] {} — {}",
                plan.current_step,
                plan.steps.len(),
                goal_desc.as_deref().unwrap_or("plan"),
                last_step_summary,
            )
        };
        let _ = self.db.insert_thought(&Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Reasoning,
            content: progress_content,
            context: None,
            created_at: chrono::Utc::now().timestamp(),
            salience: None,
            memory_tier: None,
            strength: None,
        });

        self.increment_cycle_count()?;
        Ok(CycleResult {
            step_type: if last_was_llm {
                StepType::Llm
            } else {
                StepType::Mechanical
            },
            entered_code,
            summary: format!(
                "steps {}/{} ({} executed): {}",
                plan.current_step,
                plan.steps.len(),
                steps_executed,
                last_step_summary,
            ),
        })
    }

    /// Record step progress as a thought (for dashboard visibility).
    fn record_step_progress(&self, plan: &Plan, summary: &str) {
        let goal_desc = self
            .db
            .get_goal(&plan.goal_id)
            .ok()
            .flatten()
            .map(|g| g.description.clone());
        let content = format!(
            "[step {}/{}] {} — {}",
            plan.current_step,
            plan.steps.len(),
            goal_desc.as_deref().unwrap_or("plan"),
            summary,
        );
        let _ = self.db.insert_thought(&Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Reasoning,
            content,
            context: None,
            created_at: chrono::Utc::now().timestamp(),
            salience: None,
            memory_tier: None,
            strength: None,
        });
    }

    /// Record observation and sync auto-beliefs.
    /// Only records a thought when state actually changes (delta detection).
    fn observe(&self, snapshot: &NodeSnapshot, pacer: &AdaptivePacer) -> Result<(), SoulError> {
        let snapshot_json = serde_json::to_string(snapshot)?;
        let neuroplastic = self.config.neuroplastic_enabled;

        // Delta detection — skip recording identical observations
        let state_changed = match &pacer.prev_snapshot {
            Some(prev) => {
                prev.total_payments != snapshot.total_payments
                    || prev.endpoint_count != snapshot.endpoint_count
                    || prev.children_count != snapshot.children_count
                    || prev.total_revenue != snapshot.total_revenue
            }
            None => true, // First observation always recorded
        };

        if state_changed {
            // Emit events for specific state changes
            if let Some(prev) = &pacer.prev_snapshot {
                if snapshot.total_payments > prev.total_payments {
                    let delta = snapshot.total_payments - prev.total_payments;
                    crate::events::emit_event(
                        &self.db,
                        "info",
                        "payment.received",
                        &format!(
                            "{} new payment(s) received (total: {})",
                            delta, snapshot.total_payments
                        ),
                        Some(serde_json::json!({
                            "delta": delta,
                            "total": snapshot.total_payments,
                            "revenue": snapshot.total_revenue,
                        })),
                        crate::events::EventRefs::default(),
                    );
                }
                if snapshot.endpoint_count != prev.endpoint_count {
                    crate::events::emit_event(
                        &self.db,
                        "info",
                        "endpoint.changed",
                        &format!(
                            "Endpoint count changed: {} → {}",
                            prev.endpoint_count, snapshot.endpoint_count
                        ),
                        None,
                        crate::events::EventRefs::default(),
                    );
                }
            }

            // Record observation
            let obs_content = format!(
                "Node state captured (uptime {}h, {} endpoints, {} payments)",
                snapshot.uptime_secs / 3600,
                snapshot.endpoint_count,
                snapshot.total_payments,
            );

            let obs_thought = Thought {
                id: uuid::Uuid::new_v4().to_string(),
                thought_type: ThoughtType::Observation,
                content: obs_content.clone(),
                context: Some(snapshot_json),
                created_at: chrono::Utc::now().timestamp(),
                salience: None,
                memory_tier: None,
                strength: None,
            };

            if neuroplastic {
                let fp = neuroplastic::content_fingerprint(&obs_content);
                let _ = self.db.increment_pattern(&fp);
                let pattern_counts = self.db.get_pattern_counts(&[fp]).unwrap_or_default();
                let (salience, factors) = neuroplastic::compute_salience(
                    &ThoughtType::Observation,
                    &obs_content,
                    snapshot,
                    pacer.prev_snapshot.as_ref(),
                    &pattern_counts,
                );
                let tier = neuroplastic::initial_tier(&ThoughtType::Observation, salience);
                let factors_json = serde_json::to_string(&factors).unwrap_or_default();
                self.db.insert_thought_with_salience(
                    &obs_thought,
                    salience,
                    &factors_json,
                    tier.as_str(),
                    1.0,
                )?;
            } else {
                self.db.insert_thought(&obs_thought)?;
            }
        }

        // Sync auto-beliefs from snapshot (ground truth) — always
        self.sync_auto_beliefs(snapshot);

        Ok(())
    }

    /// Create a plan for the highest-priority active goal.
    /// Returns None if there are no goals.
    async fn create_plan(
        &self,
        llm: &LlmClient,
        _snapshot: &NodeSnapshot,
        nudges: &[crate::db::Nudge],
    ) -> Result<Option<Plan>, SoulError> {
        let goals = self.db.get_active_goals()?;
        let goal = match goals.first() {
            Some(g) => g,
            None => return Ok(None),
        };

        // Check if there's already a plan for this goal
        if let Some(existing) = self.db.get_plan_for_goal(&goal.id)? {
            if existing.status == PlanStatus::Active {
                return Ok(Some(existing));
            }
        }

        tracing::info!(
            goal_id = %goal.id,
            description = %goal.description,
            "Creating plan for goal"
        );

        // Get deep workspace listing for context — show routes dir where endpoints live
        let top_listing = match self
            .tool_executor
            .execute(
                "list_directory",
                &serde_json::json!({ "path": self.config.workspace_root }),
            )
            .await
        {
            Ok(r) => r.stdout,
            Err(_) => "workspace listing unavailable".to_string(),
        };

        // Also list the routes directory (where the soul adds endpoints)
        let routes_listing = match self
            .tool_executor
            .execute(
                "list_directory",
                &serde_json::json!({ "path": format!("{}/crates/tempo-x402-node/src/routes", self.config.workspace_root) }),
            )
            .await
        {
            Ok(r) => format!("\n\ncrates/tempo-x402-node/src/routes/:\n{}", r.stdout),
            Err(_) => String::new(),
        };

        // Read routes/mod.rs to show what modules exist
        let routes_mod = match self
            .tool_executor
            .execute(
                "read_file",
                &serde_json::json!({ "path": format!("{}/crates/tempo-x402-node/src/routes/mod.rs", self.config.workspace_root) }),
            )
            .await
        {
            Ok(r) => format!("\n\ncrates/tempo-x402-node/src/routes/mod.rs:\n{}", r.stdout),
            Err(_) => String::new(),
        };

        let workspace_listing = format!("{}{}{}", top_listing, routes_listing, routes_mod);

        let recent_errors = self.get_recent_errors();
        let own_experience = feedback::consult_experience(&self.db, &goal.description);
        let peer_lessons = feedback::collect_peer_lessons(&self.db);
        let failure_chains = validation::failure_chain_summary(&self.db);
        let experience = {
            let mut exp = own_experience;
            if !peer_lessons.is_empty() {
                exp = format!("{exp}\n\n{peer_lessons}");
            }
            if !failure_chains.is_empty() {
                exp = format!("{exp}\n\n{failure_chains}");
            }
            exp
        };
        let cap_guidance = {
            let mut cg = capability::capability_guidance(&self.db);
            let brain_intel = crate::brain::brain_summary(&self.db);
            if !brain_intel.is_empty() {
                cg = format!("{cg}\n\n{brain_intel}");
            }
            cg
        };
        let role_guide = capability::role_guidance(&self.db);
        let peer_catalog = self
            .db
            .get_state("peer_endpoint_catalog")
            .ok()
            .flatten()
            .unwrap_or_default();
        let peer_prs = self
            .db
            .get_state("peer_open_prs")
            .ok()
            .flatten()
            .unwrap_or_default();
        let health_section = crate::events::format_health_for_prompt(&self.db);
        let prompt = prompts::planning_prompt(
            goal,
            &workspace_listing,
            nudges,
            &recent_errors,
            &experience,
            &cap_guidance,
            &peer_catalog,
            &peer_prs,
            &role_guide,
            &health_section,
        );
        let system =
            "You are a software engineering planner. Output ONLY a JSON array of plan steps.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let steps =
                    crate::normalize::parse_plan_steps(&response, self.config.max_plan_steps)?;
                let steps = crate::normalize::sanitize_plan_steps(steps);
                if steps.is_empty() {
                    tracing::warn!("Plan sanitization removed all steps — skipping plan creation");
                    return Ok(None);
                }

                // ── Mechanical plan validation ──
                // Hard checks that reject bad plans BEFORE execution.
                // This is server-side enforcement, not prompt injection.
                let validation = validation::validate_plan(&steps, &self.db, &goal.description);
                if !validation.is_valid() {
                    let rejection = validation.rejection_reason();
                    tracing::warn!(
                        goal_id = %goal.id,
                        violations = validation.violations.len(),
                        "Plan REJECTED by mechanical validation"
                    );
                    for v in &validation.violations {
                        tracing::info!(
                            rule = v.rule,
                            severity = ?v.severity,
                            step = ?v.step_index,
                            detail = %v.detail,
                            "Validation violation"
                        );
                    }
                    // Record the rejection as a thought so it's visible
                    let _ = self.db.insert_thought(&Thought {
                        id: uuid::Uuid::new_v4().to_string(),
                        thought_type: ThoughtType::Reasoning,
                        content: format!("Plan rejected by validation:\n{}", rejection,),
                        context: None,
                        created_at: chrono::Utc::now().timestamp(),
                        salience: None,
                        memory_tier: None,
                        strength: None,
                    });
                    // Inject rejection reason as a nudge so the next plan creation
                    // sees what went wrong and fixes it
                    let _ = self.db.insert_nudge(
                        "system",
                        &format!("Previous plan was rejected: {}", rejection),
                        3,
                    );
                    return Ok(None);
                }

                // Log soft warnings
                for v in &validation.violations {
                    if v.severity == validation::Severity::Soft {
                        tracing::info!(
                            rule = v.rule,
                            detail = %v.detail,
                            "Plan validation warning (soft)"
                        );
                    }
                }

                let now = chrono::Utc::now().timestamp();
                let initial_status = if self.config.require_plan_approval {
                    PlanStatus::PendingApproval
                } else {
                    PlanStatus::Active
                };
                let plan = Plan {
                    id: uuid::Uuid::new_v4().to_string(),
                    goal_id: goal.id.clone(),
                    steps,
                    current_step: 0,
                    status: initial_status,
                    context: std::collections::HashMap::new(),
                    replan_count: 0,
                    created_at: now,
                    updated_at: now,
                };
                self.db.insert_plan(&plan)?;

                // Store plan info for dashboard
                let _ = self.db.set_state("active_plan_id", &plan.id);
                let _ = self
                    .db
                    .set_state("active_plan_steps", &plan.steps.len().to_string());

                tracing::info!(
                    plan_id = %plan.id,
                    steps = plan.steps.len(),
                    status = plan.status.as_str(),
                    "Plan created"
                );

                // Record plan creation as a visible thought
                let step_summaries: Vec<String> = plan
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("  {}. {}", i + 1, s.summary()))
                    .collect();
                let _ = self.db.insert_thought(&Thought {
                    id: uuid::Uuid::new_v4().to_string(),
                    thought_type: ThoughtType::Decision,
                    content: format!(
                        "New plan for: {}\n{}",
                        goal.description,
                        step_summaries.join("\n")
                    ),
                    context: None,
                    created_at: chrono::Utc::now().timestamp(),
                    salience: None,
                    memory_tier: None,
                    strength: None,
                });

                Ok(Some(plan))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create plan");
                Ok(None)
            }
        }
    }

    /// Create goals when there are none.
    async fn create_goals(
        &self,
        llm: &LlmClient,
        snapshot: &NodeSnapshot,
        nudges: &[crate::db::Nudge],
    ) -> Result<(), SoulError> {
        // ── First-boot seed: concrete goals, don't ask LLM to hallucinate ──
        let total_goals_ever = self.db.count_all_goals().unwrap_or(0);
        if total_goals_ever == 0 {
            let now = chrono::Utc::now().timestamp();

            // If this is a specialist with an initial goal, seed that instead of defaults
            let mut seed_goals: Vec<(&str, &str, u32)> = Vec::new();
            let initial_goal_storage;
            if let Some(ref goal) = self.config.initial_goal {
                initial_goal_storage = goal.clone();
                seed_goals.push((
                    &initial_goal_storage,
                    "Task completed successfully as described",
                    5,
                ));
            }

            // Always include codebase research as the first or second goal
            seed_goals.push((
                "Research your own codebase: read the main thinking loop \
                 (crates/tempo-x402-soul/src/thinking.rs), the prompt system \
                 (crates/tempo-x402-soul/src/prompts.rs), and the tool executor \
                 (crates/tempo-x402-soul/src/tools.rs). Understand how you think, \
                 plan, and act. Record what you learn as beliefs — what are your \
                 strengths, weaknesses, and opportunities for self-improvement?",
                "At least 3 beliefs recorded about own architecture, capabilities, and limitations",
                if self.config.initial_goal.is_some() {
                    4
                } else {
                    5
                },
            ));

            if self.config.initial_goal.is_none() {
                seed_goals.push((
                    "Discover sibling agents using discover_peers and call one of their paid \
                     endpoints using call_peer to verify the agent-to-agent payment flow. \
                     Check what endpoints they offer, pick one, and make a real paid request. \
                     Record the result as a belief about inter-agent commerce.",
                    "discover_peers returns at least one peer with endpoints, call_peer succeeds on one of them",
                    4,
                ));
            }

            for (desc, criteria, priority) in &seed_goals {
                let goal = Goal {
                    id: uuid::Uuid::new_v4().to_string(),
                    description: desc.to_string(),
                    status: crate::world_model::GoalStatus::Active,
                    priority: *priority,
                    success_criteria: criteria.to_string(),
                    progress_notes: String::new(),
                    parent_goal_id: None,
                    retry_count: 0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };
                let _ = self.db.insert_goal(&goal);
            }
            tracing::info!(
                count = seed_goals.len(),
                specialist = ?self.config.specialization,
                "First boot — seeded starter goals"
            );
            return Ok(());
        }

        // No hardcoded goal injection — let the LLM decide what to do.
        // The goal creation prompt gives the LLM all the data (endpoint count, payments,
        // peers, fitness) and lets it decide what goals make sense.
        // Previous hardcoded demand/prune seeds caused infinite loops when combined
        // with validation rules that blocked retries of those same goals.

        tracing::info!("No goals — asking LLM to create goals");

        let beliefs = self.db.get_all_active_beliefs().unwrap_or_default();
        let recent_errors = self.get_recent_errors();
        let cycles_since_commit = self.get_cycles_since_last_commit();
        let total_cycles: u64 = self
            .db
            .get_state("total_think_cycles")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let failed_plans = self.db.count_plans_by_status("failed").unwrap_or(0);
        let recently_abandoned = self.db.get_recently_abandoned_goals(5).unwrap_or_default();
        let failed_descriptions: Vec<String> = recently_abandoned
            .iter()
            .map(|g| g.description.clone())
            .collect();
        let fitness = crate::fitness::FitnessScore::load_current(&self.db);
        let own_experience = feedback::consult_experience(&self.db, "");
        let peer_lessons = feedback::collect_peer_lessons(&self.db);
        let failure_chains = validation::failure_chain_summary(&self.db);
        let experience = {
            let mut exp = own_experience;
            if !peer_lessons.is_empty() {
                exp = format!("{exp}\n\n{peer_lessons}");
            }
            if !failure_chains.is_empty() {
                exp = format!("{exp}\n\n{failure_chains}");
            }
            exp
        };
        let cap_guidance = capability::capability_guidance(&self.db);
        let benchmark_summary = crate::benchmark::benchmark_summary_for_prompt(&self.db);
        let brain_summary = crate::brain::brain_summary(&self.db);
        let cap_with_benchmark = {
            let mut s = cap_guidance;
            if !benchmark_summary.is_empty() {
                s = format!("{s}\n\n{benchmark_summary}");
            }
            if !brain_summary.is_empty() {
                s = format!("{s}\n\n{brain_summary}");
            }
            s
        };
        let role_guide = capability::role_guidance(&self.db);
        let peer_prs = self
            .db
            .get_state("peer_open_prs")
            .ok()
            .flatten()
            .unwrap_or_default();
        let health_section = crate::events::format_health_for_prompt(&self.db);
        let prompt = prompts::goal_creation_prompt(
            snapshot,
            &beliefs,
            nudges,
            cycles_since_commit,
            failed_plans,
            total_cycles,
            &recent_errors,
            &failed_descriptions,
            fitness.as_ref(),
            &experience,
            &cap_with_benchmark,
            &peer_prs,
            &role_guide,
            &health_section,
        );
        let system = "You are an autonomous agent. Output ONLY a JSON array of goal operations.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let (applied, _) = self.apply_model_updates(&response);
                tracing::info!(goals_created = applied, "Goals created");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create goals");
            }
        }
        Ok(())
    }

    /// Complete a plan — reflect and mark done.
    async fn complete_plan(
        &self,
        llm: &LlmClient,
        plan: &mut Plan,
    ) -> Result<CycleResult, SoulError> {
        tracing::info!(plan_id = %plan.id, "Plan complete — reflecting");

        let goal = self.db.get_goal(&plan.goal_id)?;
        let goal = goal.unwrap_or_else(|| Goal {
            id: plan.goal_id.clone(),
            description: "unknown goal".to_string(),
            status: crate::world_model::GoalStatus::Active,
            priority: 3,
            success_criteria: String::new(),
            progress_notes: String::new(),
            parent_goal_id: None,
            retry_count: 0,
            created_at: plan.created_at,
            updated_at: plan.updated_at,
            completed_at: None,
        });

        // Get recent mutation for context
        let mutation_summary = match self.db.recent_mutations(1) {
            Ok(mutations) => mutations
                .first()
                .map(|m| {
                    let sha = m
                        .commit_sha
                        .as_deref()
                        .map(|s| &s[..s.len().min(7)])
                        .unwrap_or("none");
                    let check = if m.cargo_check_passed { "ok" } else { "FAIL" };
                    let test = if m.cargo_test_passed { "ok" } else { "FAIL" };
                    format!("[{sha}] check:{check} test:{test} — {}", m.description)
                })
                .unwrap_or_default(),
            Err(_) => String::new(),
        };

        let prompt = prompts::reflection_prompt(
            &goal,
            plan.steps.len(),
            &mutation_summary,
            self.get_cycles_since_last_commit(),
            self.db.count_plans_by_status("failed").unwrap_or(0),
        );
        let system =
            "You are reflecting on completed work. Output a JSON array of goal/belief updates.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let (applied, _) = self.apply_model_updates(&response);
                tracing::info!(updates_applied = applied, "Reflection applied");

                // Record reflection thought
                let reflection = Thought {
                    id: uuid::Uuid::new_v4().to_string(),
                    thought_type: ThoughtType::Reflection,
                    content: response.chars().take(500).collect(),
                    context: None,
                    created_at: chrono::Utc::now().timestamp(),
                    salience: None,
                    memory_tier: None,
                    strength: None,
                };
                if self.config.neuroplastic_enabled {
                    let _ = self.db.insert_thought_with_salience(
                        &reflection,
                        0.6,
                        r#"{"novelty":0.5,"reward_signal":0.3,"recency_boost":0.1,"reinforcement":0.0}"#,
                        "working",
                        1.0,
                    );
                } else {
                    let _ = self.db.insert_thought(&reflection);
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Reflection failed");
            }
        }

        // Mark plan as completed
        plan.status = PlanStatus::Completed;
        self.db.update_plan(plan)?;
        let _ = self.db.set_state("active_plan_id", "");
        let _ = self.db.set_state(
            "last_plan_completed_at",
            &chrono::Utc::now().timestamp().to_string(),
        );

        // Emit structured event
        crate::events::emit_event(
            &self.db,
            "info",
            "plan.completed",
            &format!(
                "Plan completed: {} ({} steps)",
                goal.description,
                plan.steps.len()
            ),
            Some(serde_json::json!({
                "steps": plan.steps.len(),
                "replan_count": plan.replan_count,
            })),
            crate::events::EventRefs {
                plan_id: Some(plan.id.clone()),
                goal_id: Some(plan.goal_id.clone()),
                ..Default::default()
            },
        );

        // Plan completion is progress — reset stagnation counter
        // (agents doing peer calls + research are productive without commits)
        self.reset_commit_counter();

        // Record structured outcome for feedback loop
        feedback::record_outcome(&self.db, plan, &goal.description, None);
        capability::record_event(
            &self.db,
            &capability::Capability::PlanComplete,
            true,
            &goal.description,
        );

        self.increment_cycle_count()?;
        Ok(CycleResult {
            step_type: StepType::PlanCompleted,
            entered_code: false,
            summary: format!("plan {} completed ({} steps)", plan.id, plan.steps.len()),
        })
    }

    /// Handle a failed step — replan or fail the plan.
    async fn handle_step_failure(
        &self,
        llm: &LlmClient,
        plan: &mut Plan,
        step_desc: &str,
        error: &str,
    ) -> Result<CycleResult, SoulError> {
        // Track error (goal retry only increments when plan fully fails, not per step)
        self.append_recent_error(error);
        crate::events::emit_event(
            &self.db,
            "warn",
            "plan.step.failed",
            &format!("Step failed: {step_desc} — {error}"),
            Some(serde_json::json!({"step": step_desc})),
            crate::events::EventRefs {
                plan_id: Some(plan.id.clone()),
                goal_id: Some(plan.goal_id.clone()),
                step_index: Some(plan.current_step as i32),
                ..Default::default()
            },
        );

        // Short-circuit: some errors are unsolvable — replanning won't help
        let error_lower = error.to_lowercase();
        let unsolvable = error.contains("Peers found: 0")
            || error.contains("unable to auto-detect email address")
            || error_lower.contains("rate limit")
            || error_lower.contains("429")
            || error_lower.contains("resource_exhausted")
            || error_lower.contains("too many requests")
            || error_lower.contains("protected")
            || error_lower.contains("guard");
        if unsolvable {
            tracing::warn!(
                plan_id = %plan.id,
                error = %error,
                "Unsolvable error — skipping replan, failing plan immediately"
            );
            plan.replan_count = 3; // force max so the block below handles it
        }

        if plan.replan_count >= 3 {
            tracing::warn!(plan_id = %plan.id, "Max replans reached — failing plan");
            plan.status = PlanStatus::Failed;
            self.db.update_plan(plan)?;
            let _ = self.db.set_state("active_plan_id", "");

            // Only increment goal retry when entire plan fails (not per step)
            let _ = self.db.increment_goal_retry(&plan.goal_id);

            // Record structured outcome for feedback loop
            let goal_for_outcome = self
                .db
                .get_goal(&plan.goal_id)
                .ok()
                .flatten()
                .map(|g| g.description.clone())
                .unwrap_or_else(|| "unknown goal".to_string());
            feedback::record_outcome(&self.db, plan, &goal_for_outcome, Some(error));
            capability::record_event(
                &self.db,
                &capability::Capability::PlanComplete,
                false,
                &format!("{}: {}", step_desc, error),
            );
            crate::events::emit_event(
                &self.db,
                "error",
                "plan.failed",
                &format!("Plan failed after {} replans: {}", plan.replan_count, error),
                Some(serde_json::json!({
                    "step": step_desc,
                    "replan_count": plan.replan_count,
                    "unsolvable": unsolvable,
                })),
                crate::events::EventRefs {
                    plan_id: Some(plan.id.clone()),
                    goal_id: Some(plan.goal_id.clone()),
                    step_index: Some(plan.current_step as i32),
                    ..Default::default()
                },
            );

            // Extract and store durable behavioral rules from this failure
            if let Ok(recent) = self.db.get_recent_plan_outcomes(1) {
                if let Some(outcome) = recent.first() {
                    let new_rules = validation::extract_durable_rules(outcome, &self.db);
                    if !new_rules.is_empty() {
                        validation::merge_durable_rules(&self.db, &new_rules);
                        tracing::info!(
                            count = new_rules.len(),
                            "Extracted durable rules from plan failure"
                        );
                    }
                }
            }

            // Write failure to persistent memory so we don't repeat the same mistake
            let goal_desc = self
                .db
                .get_goal(&plan.goal_id)
                .ok()
                .flatten()
                .map(|g| g.description.clone())
                .unwrap_or_else(|| "unknown goal".to_string());
            let failure_note = format!(
                "\n## Failed: {}\n- Step: {}\n- Error: {}\n- Plan had {} replans. Do NOT retry this exact approach.\n",
                goal_desc, step_desc, error, plan.replan_count
            );
            let _ = crate::persistent_memory::append_if_room(
                &self.config.memory_file_path,
                &failure_note,
            );
            tracing::info!("Wrote plan failure to persistent memory");

            self.increment_cycle_count()?;
            return Ok(CycleResult {
                step_type: StepType::Llm,
                entered_code: false,
                summary: format!("plan {} failed after 3 replans", plan.id),
            });
        }

        // Ask LLM to replan
        let goal = self.db.get_goal(&plan.goal_id)?;
        let goal = goal.unwrap_or_else(|| Goal {
            id: plan.goal_id.clone(),
            description: "unknown goal".to_string(),
            status: crate::world_model::GoalStatus::Active,
            priority: 3,
            success_criteria: String::new(),
            progress_notes: String::new(),
            parent_goal_id: None,
            retry_count: 0,
            created_at: plan.created_at,
            updated_at: plan.updated_at,
            completed_at: None,
        });

        let prompt = prompts::replan_prompt(&goal, step_desc, error);
        let system =
            "You are a software engineering planner. Output ONLY a JSON array of plan steps.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let new_steps = crate::normalize::sanitize_plan_steps(
                    crate::normalize::parse_plan_steps(&response, self.config.max_plan_steps)?,
                );

                // Validate replanned steps too
                let goal_desc = goal.description.clone();
                let replan_validation = validation::validate_plan(&new_steps, &self.db, &goal_desc);
                if !replan_validation.is_valid() {
                    tracing::warn!(
                        plan_id = %plan.id,
                        violations = replan_validation.violations.len(),
                        "Replan also rejected by validation — incrementing replan count"
                    );
                    plan.replan_count += 1;
                    self.db.update_plan(plan)?;
                } else {
                    // Replace remaining steps with new steps
                    plan.steps.truncate(plan.current_step);
                    plan.steps.extend(new_steps);
                    plan.replan_count += 1;
                    self.db.update_plan(plan)?;

                    tracing::info!(
                        plan_id = %plan.id,
                        replan_count = plan.replan_count,
                        new_total_steps = plan.steps.len(),
                        "Plan replanned"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Replan failed — marking plan as failed");
                plan.status = PlanStatus::Failed;
                self.db.update_plan(plan)?;
                let _ = self.db.set_state("active_plan_id", "");
                // Increment goal retry — plan fully failed
                let _ = self.db.increment_goal_retry(&plan.goal_id);
                // Record outcome so agents learn from replan failures
                let goal_desc = self
                    .db
                    .get_goal(&plan.goal_id)
                    .ok()
                    .flatten()
                    .map(|g| g.description.clone())
                    .unwrap_or_else(|| "unknown goal".to_string());
                feedback::record_outcome(
                    &self.db,
                    plan,
                    &goal_desc,
                    Some(&format!("Replan LLM call failed: {e}")),
                );
            }
        }

        self.increment_cycle_count()?;
        Ok(CycleResult {
            step_type: StepType::Llm,
            entered_code: false,
            summary: format!("replanned after failure: {step_desc}"),
        })
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

    /// Sync auto-beliefs from a snapshot: ground truth that doesn't need the LLM.
    fn sync_auto_beliefs(&self, snapshot: &NodeSnapshot) {
        let now = chrono::Utc::now().timestamp();

        let node_beliefs = [
            (
                "uptime_hours",
                format!("{}", snapshot.uptime_secs / 3600),
                "auto: from snapshot",
            ),
            (
                "endpoint_count",
                snapshot.endpoint_count.to_string(),
                "auto: from snapshot",
            ),
            (
                "total_payments",
                snapshot.total_payments.to_string(),
                "auto: from snapshot",
            ),
            (
                "total_revenue",
                snapshot.total_revenue.clone(),
                "auto: from snapshot",
            ),
            (
                "children_count",
                snapshot.children_count.to_string(),
                "auto: from snapshot",
            ),
        ];
        for (predicate, value, evidence) in &node_beliefs {
            let belief = Belief {
                id: format!("auto-node-self-{predicate}"),
                domain: BeliefDomain::Node,
                subject: "self".to_string(),
                predicate: predicate.to_string(),
                value: value.clone(),
                confidence: Confidence::High,
                evidence: evidence.to_string(),
                confirmation_count: 1,
                created_at: now,
                updated_at: now,
                active: true,
            };
            if let Err(e) = self.db.upsert_belief(&belief) {
                tracing::warn!(error = %e, predicate, "Failed to upsert auto-belief");
            }
        }

        for ep in &snapshot.endpoints {
            let ep_beliefs = [
                ("payment_count", ep.payment_count.to_string()),
                ("revenue", ep.revenue.clone()),
                ("request_count", ep.request_count.to_string()),
                ("price", ep.price.clone()),
            ];
            for (predicate, value) in &ep_beliefs {
                let belief = Belief {
                    id: format!("auto-ep-{}-{predicate}", ep.slug),
                    domain: BeliefDomain::Endpoints,
                    subject: ep.slug.clone(),
                    predicate: predicate.to_string(),
                    value: value.clone(),
                    confidence: Confidence::High,
                    evidence: "auto: from snapshot".to_string(),
                    confirmation_count: 1,
                    created_at: now,
                    updated_at: now,
                    active: true,
                };
                if let Err(e) = self.db.upsert_belief(&belief) {
                    tracing::warn!(error = %e, slug = %ep.slug, predicate, "Failed to upsert endpoint belief");
                }
            }
        }
    }

    /// Parse and apply model updates from LLM output.
    /// Returns (number of updates applied, remaining free-text).
    fn apply_model_updates(&self, text: &str) -> (u32, String) {
        let json_block = extract_json_array(text);
        let (updates_applied, remaining_text) = match json_block {
            Some((json_str, before, after)) => {
                match serde_json::from_str::<Vec<ModelUpdate>>(&json_str) {
                    Ok(updates) => {
                        let mut applied = 0u32;
                        let now = chrono::Utc::now().timestamp();
                        for update in &updates {
                            match self.apply_single_update(update, now) {
                                Ok(true) => applied += 1,
                                Ok(false) => {}
                                Err(e) => {
                                    tracing::warn!(error = %e, ?update, "Failed to apply update");
                                }
                            }
                        }
                        let remaining = format!("{}{}", before.trim(), after.trim())
                            .trim()
                            .to_string();
                        (applied, remaining)
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "Not valid model updates JSON");
                        (0, text.to_string())
                    }
                }
            }
            None => (0, text.to_string()),
        };
        (updates_applied, remaining_text)
    }

    /// Apply a single model update to the DB.
    fn apply_single_update(&self, update: &ModelUpdate, now: i64) -> Result<bool, SoulError> {
        match update {
            ModelUpdate::Create {
                domain,
                subject,
                predicate,
                value,
                evidence,
            } => {
                let domain = BeliefDomain::parse(domain).unwrap_or(BeliefDomain::Node);
                let belief = Belief {
                    id: uuid::Uuid::new_v4().to_string(),
                    domain,
                    subject: subject.clone(),
                    predicate: predicate.clone(),
                    value: value.clone(),
                    confidence: Confidence::Medium,
                    evidence: evidence.clone(),
                    confirmation_count: 1,
                    created_at: now,
                    updated_at: now,
                    active: true,
                };
                self.db.upsert_belief(&belief)?;
                Ok(true)
            }
            ModelUpdate::Update {
                id,
                value,
                evidence,
            } => {
                let beliefs = self.db.get_all_active_beliefs()?;
                if let Some(existing) = beliefs.iter().find(|b| b.id == *id) {
                    let updated = Belief {
                        value: value.clone(),
                        evidence: if evidence.is_empty() {
                            existing.evidence.clone()
                        } else {
                            evidence.clone()
                        },
                        updated_at: now,
                        ..existing.clone()
                    };
                    self.db.upsert_belief(&updated)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ModelUpdate::Confirm { id } => self.db.confirm_belief(id),
            ModelUpdate::Invalidate { id, reason } => self.db.invalidate_belief(id, reason),
            ModelUpdate::CreateGoal {
                description,
                success_criteria,
                priority,
                parent_goal_id,
            } => {
                use crate::world_model::GoalStatus;

                let active_goals = self.db.get_active_goals().unwrap_or_default();

                // Cap at 3 active goals — prevents goal sprawl
                if active_goals.len() >= 3 {
                    tracing::warn!("Goal cap reached (3 active)");
                    return Ok(false);
                }

                // Dedup: skip if an active goal has similar description (Jaccard word similarity)
                let desc_lower = description.to_lowercase();
                let desc_words: std::collections::HashSet<String> = desc_lower
                    .split_whitespace()
                    .filter(|w| w.len() > 3) // skip short words like "the", "and", "for"
                    .map(|w| w.to_string())
                    .collect();
                let is_duplicate = active_goals.iter().any(|g| {
                    let existing_lower = g.description.to_lowercase();
                    let existing_words: std::collections::HashSet<String> = existing_lower
                        .split_whitespace()
                        .filter(|w| w.len() > 3)
                        .map(|w| w.to_string())
                        .collect();
                    if desc_words.is_empty() || existing_words.is_empty() {
                        return false;
                    }
                    let intersection = desc_words.intersection(&existing_words).count();
                    let union = desc_words.union(&existing_words).count();
                    let similarity = intersection as f64 / union as f64;
                    similarity > 0.4 // 40% word overlap = duplicate
                });
                if is_duplicate {
                    tracing::info!(%description, "Skipping duplicate goal (word similarity)");
                    return Ok(false);
                }

                // Also skip if recently abandoned goal has similar description
                let recently_abandoned =
                    self.db.get_recently_abandoned_goals(10).unwrap_or_default();
                let is_retread = recently_abandoned.iter().any(|g| {
                    let existing_lower = g.description.to_lowercase();
                    let existing_words: std::collections::HashSet<String> = existing_lower
                        .split_whitespace()
                        .filter(|w| w.len() > 3)
                        .map(|w| w.to_string())
                        .collect();
                    if desc_words.is_empty() || existing_words.is_empty() {
                        return false;
                    }
                    let intersection = desc_words.intersection(&existing_words).count();
                    let union = desc_words.union(&existing_words).count();
                    let similarity = intersection as f64 / union as f64;
                    similarity > 0.5 // 50% overlap with abandoned = retread
                });
                if is_retread {
                    tracing::info!(%description, "Skipping retread of abandoned goal");
                    return Ok(false);
                }

                let goal = Goal {
                    id: uuid::Uuid::new_v4().to_string(),
                    description: description.clone(),
                    status: GoalStatus::Active,
                    priority: *priority,
                    success_criteria: success_criteria.clone(),
                    progress_notes: String::new(),
                    parent_goal_id: parent_goal_id.clone(),
                    retry_count: 0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };
                self.db.insert_goal(&goal)?;
                tracing::info!(goal_id = %goal.id, %description, "Goal created");
                Ok(true)
            }
            ModelUpdate::UpdateGoal {
                goal_id,
                progress_notes,
                status,
            } => {
                let status_str = status.as_deref();
                let notes_str = progress_notes.as_deref();
                self.db.update_goal(goal_id, status_str, notes_str, None)
            }
            ModelUpdate::CompleteGoal { goal_id, outcome } => {
                let notes = if outcome.is_empty() {
                    None
                } else {
                    Some(outcome.as_str())
                };
                self.db
                    .update_goal(goal_id, Some("completed"), notes, Some(now))
            }
            ModelUpdate::AbandonGoal { goal_id, reason } => {
                self.db
                    .update_goal(goal_id, Some("abandoned"), Some(reason.as_str()), Some(now))
            }
        }
    }

    fn housekeeping(&self) {
        crate::housekeeping::housekeeping(
            &self.db,
            self.config.prune_threshold,
            &self.config.workspace_root,
        );
    }
}

/// Extract the first JSON array from text, returning (json_str, text_before, text_after).
fn extract_json_array(text: &str) -> Option<(String, String, String)> {
    crate::normalize::extract_json_array(text)
}

/// A single tool execution record.
#[derive(Debug, Clone, Serialize)]
pub struct ToolExecution {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// Result of running the agentic tool loop.
#[derive(Debug)]
pub struct ToolLoopResult {
    pub text: String,
    pub tool_executions: Vec<ToolExecution>,
}

/// Run the agentic tool loop: repeatedly call the LLM, execute any tool calls,
/// and return the final text response plus a log of all tool executions.
/// When `use_deep` is true, uses the deeper/think model (e.g. Gemini Pro).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_tool_loop_with_model(
    llm: &LlmClient,
    system_prompt: &str,
    conversation: &mut Vec<ConversationMessage>,
    tool_declarations: &[FunctionDeclaration],
    tool_executor: &ToolExecutor,
    _db: &Arc<SoulDatabase>,
    max_tool_calls: u32,
    use_deep: bool,
) -> Result<ToolLoopResult, SoulError> {
    let mut tool_calls_made = 0u32;
    let mut final_text = String::new();
    let mut tool_executions = Vec::new();

    for _ in 0..=max_tool_calls {
        // Hard timeout per LLM call to prevent infinite hangs
        let llm_result = if use_deep {
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                llm.think_deep_with_tools(system_prompt, conversation, tool_declarations),
            )
            .await
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                llm.think_with_tools(system_prompt, conversation, tool_declarations),
            )
            .await
        };
        let result = match llm_result {
            Ok(r) => r?,
            Err(_) => {
                tracing::warn!("LLM call timed out after 120s");
                break;
            }
        };

        match result {
            LlmResult::Text(text) => {
                final_text = text;
                break;
            }
            LlmResult::FunctionCall(fc) => {
                if tool_calls_made >= max_tool_calls {
                    tracing::warn!("Hit max tool calls ({max_tool_calls}), stopping");
                    break;
                }

                tracing::info!(tool = %fc.name, args = %fc.args, "Executing tool");

                let tool_result = match tool_executor.execute(&fc.name, &fc.args).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(error = %e, "Tool execution error");
                        tools::ToolResult {
                            stdout: String::new(),
                            stderr: e,
                            exit_code: -1,
                            duration_ms: 0,
                        }
                    }
                };

                let tool_summary = summarize_tool_call(&fc.name, &fc.args);
                tool_executions.push(ToolExecution {
                    command: tool_summary,
                    stdout: tool_result.stdout.clone(),
                    stderr: tool_result.stderr.clone(),
                    exit_code: tool_result.exit_code,
                    duration_ms: tool_result.duration_ms,
                });

                conversation.push(ConversationMessage {
                    role: "model".to_string(),
                    parts: vec![ConversationPart::FunctionCall(fc.clone())],
                });

                let response_value = serde_json::to_value(&tool_result).unwrap_or_default();
                conversation.push(ConversationMessage {
                    role: "user".to_string(),
                    parts: vec![ConversationPart::FunctionResponse(FunctionResponse {
                        name: fc.name,
                        response: response_value,
                    })],
                });

                tool_calls_made += 1;
            }
        }
    }

    Ok(ToolLoopResult {
        text: final_text,
        tool_executions,
    })
}

/// Create a human-readable summary of a tool call for logging.
fn summarize_tool_call(name: &str, args: &serde_json::Value) -> String {
    match name {
        "execute_shell" => args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("read_file: {path}")
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("write_file: {path}")
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("edit_file: {path}")
        }
        "list_directory" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("list_directory: {path}")
        }
        "search_files" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("search_files: {pattern}")
        }
        "check_self" => {
            let endpoint = args.get("endpoint").and_then(|v| v.as_str()).unwrap_or("?");
            format!("check_self: /{endpoint}")
        }
        "update_memory" => "update_memory".to_string(),
        "register_endpoint" => {
            let slug = args.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
            format!("register_endpoint: /{slug}")
        }
        _ => format!("{name}: {args}"),
    }
}
