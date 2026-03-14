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
        let cap_guidance = capability::capability_guidance(&self.db);
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
        );
        let system =
            "You are a software engineering planner. Output ONLY a JSON array of plan steps.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let steps = self.parse_plan_steps(&response)?;
                let steps = Self::sanitize_plan_steps(steps);
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

        // ── Demand seed: if endpoints exist with 0 payments, inject a demand goal ──
        // This bypasses the LLM to ensure demand generation happens deterministically.
        // Back off if a demand goal was recently abandoned (prevents infinite retry loop).
        if snapshot.endpoint_count > 0 && snapshot.total_payments == 0 {
            let active_goals = self.db.get_active_goals().unwrap_or_default();
            let has_demand_goal = active_goals.iter().any(|g| {
                let d = g.description.to_lowercase();
                d.contains("call_peer") || d.contains("discover_peers") || d.contains("call peer")
            });
            // Check if a demand goal was recently abandoned (backoff to avoid tight loop)
            let recently_abandoned = self.db.get_recently_abandoned_goals(5).unwrap_or_default();
            let recently_failed_demand = recently_abandoned.iter().any(|g| {
                let d = g.description.to_lowercase();
                (d.contains("call_peer") || d.contains("discover_peers") || d.contains("demand"))
                    && (chrono::Utc::now().timestamp() - g.updated_at) < 1800 // 30 min backoff
            });
            if !has_demand_goal && !recently_failed_demand {
                let now = chrono::Utc::now().timestamp();
                let goal = Goal {
                    id: uuid::Uuid::new_v4().to_string(),
                    description: "Engage with the agent network: use discover_peers to find sibling agents, \
                         then use call_peer to call at least 2 different peer endpoints. \
                         Also read your own source code (start with crates/tempo-x402-soul/src/thinking.rs) \
                         to find one concrete improvement you could make to yourself. \
                         Record what you learn as beliefs."
                        .to_string(),
                    status: crate::world_model::GoalStatus::Active,
                    priority: 5,
                    success_criteria:
                        "call_peer succeeds on at least 1 peer endpoint AND at least 1 belief recorded about self-improvement opportunity"
                            .to_string(),
                    progress_notes: String::new(),
                    parent_goal_id: None,
                    retry_count: 0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };
                let _ = self.db.insert_goal(&goal);
                tracing::info!(
                    "Demand seed — injected demand-generation goal (0 payments, {} endpoints)",
                    snapshot.endpoint_count
                );
                return Ok(());
            }

            // ── Prune seed: too many endpoints with 0 payments → prune during demand backoff ──
            if recently_failed_demand && snapshot.endpoint_count > 5 {
                let has_prune_goal = active_goals.iter().any(|g| {
                    let d = g.description.to_lowercase();
                    d.contains("delete_endpoint")
                        || d.contains("prune")
                        || d.contains("delete endpoint")
                });
                let recently_pruned = recently_abandoned.iter().any(|g| {
                    let d = g.description.to_lowercase();
                    (d.contains("prune") || d.contains("delete_endpoint"))
                        && (chrono::Utc::now().timestamp() - g.updated_at) < 3600
                });
                if !has_prune_goal && !recently_pruned {
                    let now = chrono::Utc::now().timestamp();
                    let goal = Goal {
                        id: uuid::Uuid::new_v4().to_string(),
                        description: format!(
                            "Prune endpoints: you have {} endpoints and 0 payments. \
                             Use delete_endpoint to remove ALL script endpoints except \
                             the 2-3 most useful ones. Keep core endpoints: chat, info, clone, \
                             soul. Target: 5 or fewer total endpoints. \
                             After pruning, focus on research and code improvement instead.",
                            snapshot.endpoint_count
                        ),
                        status: crate::world_model::GoalStatus::Active,
                        priority: 4,
                        success_criteria: "endpoint count reduced to 5 or fewer".to_string(),
                        progress_notes: String::new(),
                        parent_goal_id: None,
                        retry_count: 0,
                        created_at: now,
                        updated_at: now,
                        completed_at: None,
                    };
                    let _ = self.db.insert_goal(&goal);
                    tracing::info!(
                        "Prune seed — injected endpoint pruning goal ({} endpoints, 0 payments)",
                        snapshot.endpoint_count
                    );
                    return Ok(());
                }
            }
        }

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

            // Extract and store durable behavioral rules from this failure
            if let Ok(recent) = self.db.get_recent_plan_outcomes(1) {
                if let Some(outcome) = recent.first() {
                    let new_rules = validation::extract_durable_rules(outcome);
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
                let new_steps = Self::sanitize_plan_steps(self.parse_plan_steps(&response)?);

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

    /// Parse plan steps from LLM output (find JSON array).
    /// Includes normalization to handle common LLM format mistakes.
    /// Fix common path errors the LLM makes when referencing source files.
    fn fix_common_path_errors(path: &str) -> String {
        let mut p = path.to_string();

        // LLM writes "crates/tempo-x402/src/thinking.rs" but thinking.rs is in tempo-x402-soul
        let soul_files = [
            "thinking.rs",
            "plan.rs",
            "prompts.rs",
            "chat.rs",
            "memory.rs",
            "git.rs",
            "coding.rs",
            "mode.rs",
            "neuroplastic.rs",
            "persistent_memory.rs",
            "world_model.rs",
            "observer.rs",
        ];
        for &f in &soul_files {
            let wrong = format!("crates/tempo-x402/src/{f}");
            if p == wrong || p.ends_with(&format!("/{wrong}")) {
                p = format!("crates/tempo-x402-soul/src/{f}");
                break;
            }
        }

        // Strip leading /data/workspace/ prefix (agent's absolute path)
        if let Some(stripped) = p.strip_prefix("/data/workspace/") {
            p = stripped.to_string();
        }

        p
    }

    /// Sanitize plan steps to remove or fix obviously broken steps.
    /// This runs at plan creation time so broken steps never count as execution failures.
    fn sanitize_plan_steps(steps: Vec<PlanStep>) -> Vec<PlanStep> {
        let original_count = steps.len();
        let mut sanitized = Vec::with_capacity(original_count);

        for step in steps {
            match &step {
                // Remove shell commands that reference undefined shell variables
                // (LLM generates `echo "$soul_response"` thinking plan context is shell vars)
                PlanStep::RunShell { command, store_as } => {
                    // Strip redundant tool-name prefix from command
                    // LLM sometimes generates {"type":"run_shell","command":"run_shell curl ..."}
                    let command = if command.starts_with("run_shell ")
                        || command.starts_with("execute_shell ")
                    {
                        let stripped = command.split_once(' ').map(|x| x.1).unwrap_or(command);
                        tracing::debug!(
                            original = %command,
                            fixed = %stripped,
                            "Stripped tool-name prefix from RunShell command"
                        );
                        stripped
                    } else if command == "run_shell" || command == "execute_shell" {
                        // Bare tool name with no actual command — skip
                        tracing::debug!("Sanitized out bare run_shell/execute_shell command");
                        continue;
                    } else {
                        command.as_str()
                    };

                    // Skip commands that are purely writing shell-variable placeholders to files
                    // e.g., `echo "$response" > file.json` or `echo '$info_call_result' > file`
                    let has_placeholder_var = command.contains("$soul_response")
                        || command.contains("$info_call_result")
                        || command.contains("$chat_response")
                        || command.contains("$soul_call_result")
                        || command.contains("${soul")
                        || command.contains("${info")
                        || command.contains("${chat");
                    let is_just_echo_to_file = command.starts_with("echo ")
                        && (command.contains(" > ") || command.contains(" >> "))
                        && has_placeholder_var;

                    if is_just_echo_to_file {
                        tracing::debug!(
                            command = %command,
                            "Sanitized out shell command with undefined variable placeholder"
                        );
                        continue;
                    }

                    // Skip commands that use unavailable tools
                    if command.starts_with("jq ") || command.contains("| jq ") {
                        tracing::debug!(
                            command = %command,
                            "Sanitized out shell command using unavailable 'jq'"
                        );
                        continue;
                    }

                    // If command was modified (prefix stripped), push corrected step
                    sanitized.push(PlanStep::RunShell {
                        command: command.to_string(),
                        store_as: store_as.clone(),
                    });
                }
                // Remove ReadFile steps that reference non-existent plan context files
                // (LLM generates `read siblings.json` or `read discovered_peers.json`)
                PlanStep::ReadFile { path, store_as } => {
                    let bogus_files = [
                        "siblings.json",
                        "discovered_peers.json",
                        "filtered_peers.json",
                        "available_tools.txt",
                        "target_peer_info.json",
                        "verified_source_paths.txt",
                        "soul_call_result.json",
                        "last_soul_call.json",
                        "peer_info.json",
                        "peer_data.json",
                        "peer_status.json",
                        "info_call_result.json",
                        "soul_response.json",
                        "call_result.json",
                        "network_data.json",
                        "health_data.json",
                    ];
                    let filename = path.rsplit('/').next().unwrap_or(path);
                    // Also catch any *_result.json or *_response.json patterns
                    let is_bogus_pattern = filename.ends_with("_result.json")
                        || filename.ends_with("_response.json")
                        || filename.ends_with("_data.json")
                        || filename.ends_with("_output.json");
                    if bogus_files.iter().any(|&b| filename == b) || is_bogus_pattern {
                        tracing::debug!(
                            path = %path,
                            "Sanitized out ReadFile for non-existent plan artifact"
                        );
                        continue;
                    }
                    // Fix common path errors
                    let fixed_path = Self::fix_common_path_errors(path);
                    if fixed_path != *path {
                        sanitized.push(PlanStep::ReadFile {
                            path: fixed_path,
                            store_as: store_as.clone(),
                        });
                    } else {
                        sanitized.push(step);
                    }
                }
                // Fix common path mistakes in EditCode/GenerateCode
                // LLM often writes "crates/tempo-x402/src/thinking.rs" (wrong crate)
                PlanStep::EditCode {
                    file_path,
                    description,
                    context_keys,
                } => {
                    let fixed_path = Self::fix_common_path_errors(file_path);
                    if crate::guard::is_protected(&fixed_path) {
                        tracing::debug!(
                            path = %fixed_path,
                            "Sanitized out EditCode for protected file"
                        );
                        continue;
                    }
                    if fixed_path != *file_path {
                        tracing::debug!(
                            original = %file_path,
                            fixed = %fixed_path,
                            "Fixed path in EditCode step"
                        );
                        sanitized.push(PlanStep::EditCode {
                            file_path: fixed_path,
                            description: description.clone(),
                            context_keys: context_keys.clone(),
                        });
                    } else {
                        sanitized.push(step);
                    }
                }
                PlanStep::GenerateCode {
                    file_path,
                    description,
                    context_keys,
                } => {
                    let fixed_path = Self::fix_common_path_errors(file_path);
                    if crate::guard::is_protected(&fixed_path) {
                        tracing::debug!(
                            path = %fixed_path,
                            "Sanitized out GenerateCode for protected file"
                        );
                        continue;
                    }
                    if fixed_path != *file_path {
                        sanitized.push(PlanStep::GenerateCode {
                            file_path: fixed_path,
                            description: description.clone(),
                            context_keys: context_keys.clone(),
                        });
                    } else {
                        sanitized.push(step);
                    }
                }
                // Remove CallPaidEndpoint with localhost URLs
                PlanStep::CallPaidEndpoint { url, .. } => {
                    if url.contains("localhost") || url.contains("127.0.0.1") {
                        tracing::debug!(
                            url = %url,
                            "Sanitized out CallPaidEndpoint with localhost URL"
                        );
                        continue;
                    }
                    sanitized.push(step);
                }
                // Everything else passes through
                _ => sanitized.push(step),
            }
        }

        if sanitized.len() < original_count {
            tracing::info!(
                original = original_count,
                sanitized = sanitized.len(),
                removed = original_count - sanitized.len(),
                "Plan steps sanitized"
            );
        }

        sanitized
    }

    fn parse_plan_steps(&self, text: &str) -> Result<Vec<PlanStep>, SoulError> {
        let try_parse = |json_str: &str| -> Result<Vec<PlanStep>, serde_json::Error> {
            // Always normalize first — coerces wrong types (maps→strings) and fixes
            // missing "type" fields before attempting deserialization.
            let normalized = Self::normalize_plan_json(json_str);
            match serde_json::from_str::<Vec<PlanStep>>(&normalized) {
                Ok(steps) => Ok(steps),
                Err(_) if normalized != json_str => {
                    // Normalization changed something but still failed — try original
                    // in case normalization mangled valid JSON
                    serde_json::from_str::<Vec<PlanStep>>(json_str)
                }
                Err(e) => Err(e),
            }
        };

        // Try to find a JSON array in the response
        if let Some((json_str, _, _)) = extract_json_array(text) {
            match try_parse(&json_str) {
                Ok(mut steps) => {
                    let max = self.config.max_plan_steps;
                    if steps.len() > max {
                        steps.truncate(max);
                    }
                    if steps.is_empty() {
                        return Err(SoulError::Config("LLM returned empty plan".to_string()));
                    }
                    return Ok(steps);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse plan steps JSON");
                }
            }
        }

        // Fallback: try parsing the entire text as JSON
        match try_parse(text.trim()) {
            Ok(mut steps) => {
                let max = self.config.max_plan_steps;
                if steps.len() > max {
                    steps.truncate(max);
                }
                if steps.is_empty() {
                    Err(SoulError::Config("LLM returned empty plan".to_string()))
                } else {
                    Ok(steps)
                }
            }
            Err(e) => Err(SoulError::Config(format!(
                "Cannot parse plan steps: {e}. Response: {}",
                &text[..text.len().min(200)]
            ))),
        }
    }

    /// Normalize common LLM plan JSON mistakes into valid PlanStep format.
    /// The LLM often outputs {"action": "ls", "name": "explore"} instead of
    /// {"type": "run_shell", "command": "ls"}.
    /// Also coerces non-string values to strings (LLM sometimes returns maps/arrays
    /// where strings are expected).
    fn normalize_plan_json(json_str: &str) -> String {
        let parsed: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return json_str.to_string(),
        };

        let normalized: Vec<serde_json::Value> = parsed
            .into_iter()
            .filter_map(|mut obj| {
                let map = obj.as_object_mut()?;

                // Coerce non-string field values to strings (except "type" and "context_keys").
                // LLMs sometimes return {"store_as": {"key": "val"}} instead of "val".
                let keys: Vec<String> = map.keys().cloned().collect();
                for key in &keys {
                    if key == "type" || key == "context_keys" {
                        continue;
                    }
                    let needs_coerce = match map.get(key) {
                        Some(serde_json::Value::Object(_)) => true,
                        Some(serde_json::Value::Array(_)) => true,
                        Some(serde_json::Value::Number(n)) => {
                            // Coerce numbers to strings
                            map.insert(key.clone(), serde_json::json!(n.to_string()));
                            false
                        }
                        Some(serde_json::Value::Bool(b)) => {
                            map.insert(key.clone(), serde_json::json!(b.to_string()));
                            false
                        }
                        _ => false,
                    };
                    if needs_coerce {
                        // Try to extract a string from the nested value
                        let coerced = match map.get(key) {
                            Some(serde_json::Value::Object(inner)) => {
                                // Take the first string value, or serialize the whole thing
                                inner
                                    .values()
                                    .find_map(|v| v.as_str().map(String::from))
                                    .unwrap_or_else(|| {
                                        serde_json::to_string(map.get(key).unwrap())
                                            .unwrap_or_default()
                                    })
                            }
                            Some(serde_json::Value::Array(arr)) => {
                                // For arrays of strings, join them; otherwise serialize
                                let strings: Vec<&str> =
                                    arr.iter().filter_map(|v| v.as_str()).collect();
                                if strings.len() == arr.len() {
                                    strings.join(", ")
                                } else {
                                    serde_json::to_string(map.get(key).unwrap()).unwrap_or_default()
                                }
                            }
                            _ => continue,
                        };
                        map.insert(key.clone(), serde_json::json!(coerced));
                    }
                }

                // Coerce context_keys: if it's not an array of strings, fix it
                if let Some(ck) = map.get("context_keys") {
                    match ck {
                        serde_json::Value::String(s) => {
                            // Single string → wrap in array
                            map.insert("context_keys".to_string(), serde_json::json!([s.clone()]));
                        }
                        serde_json::Value::Object(inner) => {
                            // Map → extract string values as array
                            let vals: Vec<String> = inner
                                .values()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                            map.insert("context_keys".to_string(), serde_json::json!(vals));
                        }
                        serde_json::Value::Array(arr) => {
                            // Array of non-strings → coerce each to string
                            let vals: Vec<String> = arr
                                .iter()
                                .map(|v| {
                                    v.as_str()
                                        .map(String::from)
                                        .unwrap_or_else(|| v.to_string())
                                })
                                .collect();
                            map.insert("context_keys".to_string(), serde_json::json!(vals));
                        }
                        _ => {}
                    }
                }

                // Already has a valid "type" field — return with coerced values
                if map.contains_key("type") {
                    return Some(obj);
                }

                // Infer type from other fields the LLM commonly uses
                let action = map
                    .get("action")
                    .or_else(|| map.get("command"))
                    .or_else(|| map.get("cmd"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let Some(action_str) = action {
                    let mut step = serde_json::Map::new();
                    let action_lower = action_str.to_lowercase();

                    if action_lower.starts_with("ls")
                        || action_lower.starts_with("find ")
                        || action_lower.starts_with("tree")
                    {
                        // Directory listing
                        if action_lower == "ls" || action_lower.starts_with("ls ") {
                            let path = action_str.strip_prefix("ls").unwrap_or(".").trim();
                            let path = if path.is_empty() || path == "-F" || path == "-la" {
                                "."
                            } else {
                                path.trim_start_matches("-F ")
                                    .trim_start_matches("-la ")
                                    .trim()
                            };
                            step.insert("type".to_string(), serde_json::json!("list_dir"));
                            step.insert("path".to_string(), serde_json::json!(path));
                        } else {
                            step.insert("type".to_string(), serde_json::json!("run_shell"));
                            step.insert("command".to_string(), serde_json::json!(action_str));
                        }
                    } else if action_lower.starts_with("cat ") || action_lower.starts_with("read ")
                    {
                        let path = action_str.split_once(' ').map(|x| x.1).unwrap_or("");
                        if path.ends_with('/') || (!path.contains('.') && !path.is_empty()) {
                            step.insert("type".to_string(), serde_json::json!("list_dir"));
                        } else {
                            step.insert("type".to_string(), serde_json::json!("read_file"));
                        }
                        step.insert("path".to_string(), serde_json::json!(path));
                    } else if action_lower.starts_with("grep ") || action_lower.starts_with("rg ") {
                        step.insert("type".to_string(), serde_json::json!("run_shell"));
                        step.insert("command".to_string(), serde_json::json!(action_str));
                    } else if action_lower.starts_with("read_file")
                        || action_lower.starts_with("read file")
                    {
                        // LLM used "read_file /path" as action — convert to read_file step
                        let path = action_str.split_once(' ').map(|x| x.1).unwrap_or(".");
                        step.insert("type".to_string(), serde_json::json!("read_file"));
                        step.insert("path".to_string(), serde_json::json!(path));
                    } else if action_lower.starts_with("search_code")
                        || action_lower.starts_with("search code")
                        || action_lower.starts_with("search_files")
                    {
                        // LLM used "search_code pattern" as action — convert to search_code step
                        let pattern = action_str.split_once(' ').map(|x| x.1).unwrap_or("*");
                        step.insert("type".to_string(), serde_json::json!("search_code"));
                        step.insert("pattern".to_string(), serde_json::json!(pattern));
                        step.insert("directory".to_string(), serde_json::json!("."));
                    } else if action_lower.starts_with("list_dir")
                        || action_lower.starts_with("list dir")
                    {
                        let path = action_str.split_once(' ').map(|x| x.1).unwrap_or(".");
                        step.insert("type".to_string(), serde_json::json!("list_dir"));
                        step.insert("path".to_string(), serde_json::json!(path));
                    } else if action_lower.starts_with("call_peer")
                        || action_lower.starts_with("call peer")
                    {
                        // LLM used "call_peer soul" as action — convert to call_peer step
                        let slug = action_str
                            .split_once(' ')
                            .map(|x| x.1.trim())
                            .unwrap_or("info");
                        step.insert("type".to_string(), serde_json::json!("call_peer"));
                        step.insert("slug".to_string(), serde_json::json!(slug));
                    } else if action_lower.starts_with("discover_peers")
                        || action_lower.starts_with("discover peers")
                    {
                        step.insert("type".to_string(), serde_json::json!("discover_peers"));
                    } else if action_lower.starts_with("check_self")
                        || action_lower.starts_with("check self")
                    {
                        let endpoint = action_str
                            .split_once(' ')
                            .map(|x| x.1.trim())
                            .unwrap_or("health");
                        step.insert("type".to_string(), serde_json::json!("check_self"));
                        step.insert("endpoint".to_string(), serde_json::json!(endpoint));
                    } else if action_lower.starts_with("commit") {
                        let message = action_str
                            .split_once(' ')
                            .map(|x| x.1.trim())
                            .unwrap_or("auto-commit");
                        step.insert("type".to_string(), serde_json::json!("commit"));
                        step.insert("message".to_string(), serde_json::json!(message));
                    } else if action_lower.starts_with("cargo_check")
                        || action_lower.starts_with("cargo check")
                    {
                        step.insert("type".to_string(), serde_json::json!("cargo_check"));
                    } else if action_lower.starts_with("clone_self")
                        || action_lower.starts_with("clone self")
                    {
                        step.insert("type".to_string(), serde_json::json!("clone_self"));
                    } else if action_lower.starts_with("spawn_specialist")
                        || action_lower.starts_with("spawn specialist")
                    {
                        let spec = action_str
                            .split_once(' ')
                            .map(|x| x.1.trim())
                            .unwrap_or("generalist");
                        step.insert("type".to_string(), serde_json::json!("spawn_specialist"));
                        step.insert("specialization".to_string(), serde_json::json!(spec));
                    } else if action_lower.starts_with("delegate_task")
                        || action_lower.starts_with("delegate task")
                    {
                        let desc = action_str.split_once(' ').map(|x| x.1.trim()).unwrap_or("");
                        step.insert("type".to_string(), serde_json::json!("delegate_task"));
                        step.insert("task_description".to_string(), serde_json::json!(desc));
                        step.insert("target".to_string(), serde_json::json!(""));
                    } else if action_lower.starts_with("run_shell")
                        || action_lower.starts_with("execute_shell")
                    {
                        // LLM used "run_shell curl ..." or "execute_shell ls" as action —
                        // strip the tool name prefix and keep just the actual command
                        let actual_cmd =
                            action_str.split_once(' ').map(|x| x.1.trim()).unwrap_or("");
                        if actual_cmd.is_empty() {
                            // Bare "run_shell" with no actual command — skip entirely
                            return None;
                        }
                        step.insert("type".to_string(), serde_json::json!("run_shell"));
                        step.insert("command".to_string(), serde_json::json!(actual_cmd));
                    } else if action_lower.starts_with("think") {
                        // LLM used "think: ..." or "think about ..." as action
                        let question = action_str
                            .strip_prefix("think:")
                            .or_else(|| action_str.strip_prefix("think "))
                            .map(|s| s.trim())
                            .unwrap_or(&action_str);
                        step.insert("type".to_string(), serde_json::json!("think"));
                        step.insert("question".to_string(), serde_json::json!(question));
                    } else {
                        // Default: treat as shell command
                        step.insert("type".to_string(), serde_json::json!("run_shell"));
                        step.insert("command".to_string(), serde_json::json!(action_str));
                    }

                    // Carry over store_as if present
                    if let Some(store) = map.get("store_as").or_else(|| map.get("name")) {
                        step.insert("store_as".to_string(), store.clone());
                    }

                    return Some(serde_json::Value::Object(step));
                }

                // Has "path" but no type — infer read_file or list_dir
                if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                    let mut step = serde_json::Map::new();
                    if path.ends_with('/')
                        || path == "."
                        || path == ".."
                        || (!path.contains('.') && !path.is_empty())
                    {
                        step.insert("type".to_string(), serde_json::json!("list_dir"));
                    } else {
                        step.insert("type".to_string(), serde_json::json!("read_file"));
                    }
                    step.insert("path".to_string(), serde_json::json!(path));
                    if let Some(store) = map.get("store_as") {
                        step.insert("store_as".to_string(), store.clone());
                    }
                    return Some(serde_json::Value::Object(step));
                }

                // Has "question" but no type — probably a think
                if let Some(q) = map.get("question").and_then(|v| v.as_str()) {
                    let mut step = serde_json::Map::new();
                    step.insert("type".to_string(), serde_json::json!("think"));
                    step.insert("question".to_string(), serde_json::json!(q));
                    if let Some(store) = map.get("store_as") {
                        step.insert("store_as".to_string(), store.clone());
                    }
                    return Some(serde_json::Value::Object(step));
                }

                None // Unrecognizable step — skip it
            })
            .collect();

        serde_json::to_string(&normalized).unwrap_or_else(|_| json_str.to_string())
    }

    /// Increment the total_think_cycles counter, cycles_since_last_commit, and update last_think_at.
    fn increment_cycle_count(&self) -> Result<(), SoulError> {
        let current: u64 = self
            .db
            .get_state("total_think_cycles")?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        self.db
            .set_state("total_think_cycles", &(current + 1).to_string())?;
        self.db
            .set_state("last_think_at", &chrono::Utc::now().timestamp().to_string())?;

        // Increment cycles_since_last_commit
        let since_commit: u64 = self
            .db
            .get_state("cycles_since_last_commit")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        self.db
            .set_state("cycles_since_last_commit", &(since_commit + 1).to_string())?;

        Ok(())
    }

    /// Reset cycles_since_last_commit (called when a commit succeeds).
    fn reset_commit_counter(&self) {
        let _ = self.db.set_state("cycles_since_last_commit", "0");
    }

    /// Append an error to the recent_errors list (capped at 5).
    fn append_recent_error(&self, error: &str) {
        let truncated: String = error.chars().take(200).collect();
        let mut errors: Vec<String> = self
            .db
            .get_state("recent_errors")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        errors.push(truncated);
        if errors.len() > 5 {
            errors.drain(..errors.len() - 5);
        }
        if let Ok(json) = serde_json::to_string(&errors) {
            let _ = self.db.set_state("recent_errors", &json);
        }
    }

    /// Get recent errors from soul_state.
    fn get_recent_errors(&self) -> Vec<String> {
        self.db
            .get_state("recent_errors")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Get cycles since last commit from soul_state.
    fn get_cycles_since_last_commit(&self) -> u64 {
        self.db
            .get_state("cycles_since_last_commit")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
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

    /// Background housekeeping: decay, promotion, belief decay, consolidation.
    /// Ported from mind crate's subconscious loop — runs inline, no separate task.
    fn housekeeping(&self) {
        let cycle_count: u64 = self
            .db
            .get_state("total_think_cycles")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Every 10 cycles: decay + promote + belief decay
        if cycle_count > 0 && cycle_count.is_multiple_of(10) {
            match self.db.run_decay_cycle(self.config.prune_threshold) {
                Ok((decayed, pruned)) => {
                    if decayed > 0 || pruned > 0 {
                        tracing::info!(decayed, pruned, "Housekeeping: decay cycle");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Housekeeping: decay failed"),
            }

            match self.db.promote_salient_sensory(0.6) {
                Ok(promoted) => {
                    if promoted > 0 {
                        tracing::info!(promoted, "Housekeeping: promotion");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Housekeeping: promotion failed"),
            }

            match self.db.decay_beliefs() {
                Ok((dh, dm, da)) => {
                    if dh > 0 || dm > 0 || da > 0 {
                        tracing::info!(
                            demoted_high = dh,
                            demoted_med = dm,
                            deactivated = da,
                            "Housekeeping: belief decay"
                        );
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Housekeeping: belief decay failed"),
            }

            // WAL checkpoint — prevent .db-wal files from growing unbounded
            let _ = self.db.wal_checkpoint();

            // Lifecycle pruning — keep the database bounded
            match self.db.prune_old_data() {
                Ok(stats) => {
                    if stats.total() > 0 {
                        tracing::info!(
                            thoughts = stats.thoughts,
                            goals = stats.goals,
                            plans = stats.plans,
                            mutations = stats.mutations,
                            nudges = stats.nudges,
                            beliefs = stats.beliefs,
                            messages = stats.messages,
                            sessions = stats.sessions,
                            "Housekeeping: lifecycle pruning"
                        );
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Housekeeping: pruning failed"),
            }

            // Clean up cargo build artifacts — target/ can be 2-4 GB
            // Only cleaned after commits normally, but if a plan fails mid-way
            // the target/ directory persists between cycles
            let target_dir = format!("{}/target", self.config.workspace_root);
            if std::path::Path::new(&target_dir).exists() {
                tracing::info!("Housekeeping: cleaning workspace target/ to reclaim disk space");
                let _ = std::fs::remove_dir_all(&target_dir);
            }

            // Clean up git garbage — pack loose objects to reduce .git/ size
            let workspace = &self.config.workspace_root;
            let _ = std::process::Command::new("git")
                .args(["gc", "--auto", "--quiet"])
                .current_dir(workspace)
                .output();
        }

        // Every 40 cycles: simple consolidation (no LLM — save tokens for coding)
        if cycle_count > 0 && cycle_count.is_multiple_of(40) {
            self.simple_consolidate();
        }
    }

    /// Simple memory consolidation: fetch recent thoughts, concatenate, store as MemoryConsolidation.
    /// No LLM — keeps token budget for actual coding work.
    fn simple_consolidate(&self) {
        let thoughts = match self.db.recent_thoughts_by_type(
            &[
                ThoughtType::Reasoning,
                ThoughtType::Decision,
                ThoughtType::Observation,
                ThoughtType::Reflection,
            ],
            20,
        ) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, "Consolidation: failed to fetch thoughts");
                return;
            }
        };

        if thoughts.len() < 5 {
            return;
        }

        let entries: Vec<String> = thoughts
            .iter()
            .map(|t| {
                let truncated: String = t.content.chars().take(100).collect();
                format!("[{}] {}", t.thought_type.as_str(), truncated)
            })
            .collect();

        let summary = format!(
            "[Memory consolidation ({} thoughts)]\n{}",
            thoughts.len(),
            entries.join("\n")
        );

        let consolidation = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::MemoryConsolidation,
            content: summary,
            context: None,
            created_at: chrono::Utc::now().timestamp(),
            salience: None,
            memory_tier: None,
            strength: None,
        };

        match self.db.insert_thought_with_salience(
            &consolidation,
            0.9,
            r#"{"novelty":0.8,"reward_signal":0.0,"recency_boost":0.1,"reinforcement":0.0}"#,
            "long_term",
            1.0,
        ) {
            Ok(()) => {
                // Consolidation is digestion — delete the source thoughts that were absorbed
                let ids: Vec<String> = thoughts.iter().map(|t| t.id.clone()).collect();
                let mut deleted = 0u32;
                for id in &ids {
                    if let Ok(1) = self.db.delete_thought(id) {
                        deleted += 1;
                    }
                }
                tracing::info!(
                    consolidated = thoughts.len(),
                    deleted,
                    "Housekeeping: memory consolidation (absorbed {} thoughts)",
                    deleted
                );
            }
            Err(e) => tracing::warn!(error = %e, "Housekeeping: consolidation insert failed"),
        }
    }
}

/// Extract the first JSON array from text, returning (json_str, text_before, text_after).
fn extract_json_array(text: &str) -> Option<(String, String, String)> {
    let start = text.find('[')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &ch) in bytes[start..].iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'[' if !in_string => depth += 1,
            b']' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i + 1;
                    let json_str = text[start..end].to_string();
                    let before = text[..start].to_string();
                    let after = text[end..].to_string();
                    return Some((json_str, before, after));
                }
            }
            _ => {}
        }
    }
    None
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
