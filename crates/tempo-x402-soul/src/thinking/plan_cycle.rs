//! Plan cycle execution and step progress tracking.
use super::*;

impl ThinkingLoop {
    pub(super) async fn plan_cycle(
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
                // No active plan — try autonomous compilation first (no LLM needed)
                let autonomous_plan = {
                    let top_goal = self
                        .db
                        .get_active_goals()
                        .ok()
                        .and_then(|goals| goals.into_iter().next());
                    if let Some(goal) = top_goal {
                        let instance_id = self.config.instance_id.as_deref().unwrap_or("unknown");
                        match crate::autonomy::compile_autonomous_plan(
                            &self.db,
                            &goal.id,
                            &goal.description,
                            instance_id,
                        ) {
                            crate::autonomy::CompilationResult::Compiled(plan) => {
                                self.db.insert_plan(&plan)?;
                                let _ = self.db.set_state("active_plan_id", &plan.id);
                                crate::events::emit_info(
                                    &self.db,
                                    "plan.autonomous",
                                    &format!("Autonomous plan compiled for: {}", goal.description),
                                );
                                Some(plan)
                            }
                            crate::autonomy::CompilationResult::FallbackToLlm(reason) => {
                                tracing::debug!(reason = %reason, "Autonomous planning fell back to LLM");
                                None
                            }
                        }
                    } else {
                        None
                    }
                };

                // Use autonomous plan if compiled, otherwise fall back to LLM
                let plan_source = if let Some(plan) = autonomous_plan {
                    Some(plan)
                } else {
                    self.create_plan(llm, snapshot, &nudges).await?
                };

                match plan_source {
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

        // Circuit breaker 3: goal has too many trivial completions (TOTAL, not consecutive).
        // Even one interspersed failure shouldn't save a goal that keeps producing
        // trivial read-only plans. 2 trivial completions = abandon immediately.
        if let Ok(Some(goal)) = self.db.get_goal(&plan.goal_id) {
            let recent_outcomes = self.db.get_recent_plan_outcomes(10).unwrap_or_default();
            let total_trivial = recent_outcomes
                .iter()
                .filter(|o| o.goal_id == plan.goal_id && o.status == "completed_trivial")
                .count();
            if total_trivial >= 2 {
                tracing::warn!(
                    goal_id = %plan.goal_id,
                    total_trivial,
                    "Goal stuck in trivial completion loop — abandoning"
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
                let trivial_err = format!(
                    "{} trivial completions — goal is stuck in read-only loop",
                    total_trivial
                );
                feedback::record_outcome(&self.db, &plan, &goal.description, Some(&trivial_err));
                crate::events::emit_event(
                    &self.db,
                    "warn",
                    "goal.trivial_loop",
                    &format!(
                        "Goal abandoned after {} trivial completions: {}",
                        total_trivial, goal.description
                    ),
                    Some(serde_json::json!({"total_trivial": total_trivial})),
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
                        "Goal '{}' completed trivially {} times in a row — it only did reads/thinks. \
                         Pick a goal that requires CONCRETE actions: editing code, creating endpoints, \
                         running shell commands, or making commits.",
                        desc_preview, total_trivial
                    ),
                    4,
                );
                self.increment_cycle_count()?;
                return Ok(CycleResult {
                    step_type: StepType::Observe,
                    entered_code: false,
                    summary: format!(
                        "abandoned goal after {} consecutive trivial completions",
                        total_trivial
                    ),
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
                cycle_count,
                ..Default::default()
            };
            let local_prediction = crate::brain::predict_step(&self.db, &step, &brain_ctx);

            // Combine local prediction with colony expertise (MoE)
            let capability = crate::moe::step_to_capability(&step);
            let router = crate::moe::load_router(&self.db);
            let prediction = router.combine_prediction(&local_prediction, capability);

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

                    // Synthesis: record outcome for metacognitive tracking
                    // Evaluation: record per-system predictions with Brier scoring
                    {
                        let cortex = crate::cortex::load_cortex(&self.db);
                        let gene_pool = crate::genesis::load_gene_pool(&self.db);
                        let hivemind = crate::hivemind::load_hivemind(&self.db);
                        let goal_desc = self
                            .db
                            .get_goal(&plan.goal_id)
                            .ok()
                            .flatten()
                            .map(|g| g.description.clone())
                            .unwrap_or_default();
                        let mut synth = crate::synthesis::load_synthesis(&self.db);
                        let unified = synth.predict_step(
                            &step,
                            &prediction,
                            &cortex,
                            &gene_pool,
                            &hivemind,
                            &goal_desc,
                        );
                        synth.record_outcome(&unified.votes, true);
                        crate::synthesis::save_synthesis(&self.db, &synth);

                        // Rigorous evaluation: per-system Brier score tracking
                        let mut eval = crate::evaluation::load_evaluation(&self.db);
                        for vote in &unified.votes {
                            let prob = (vote.prediction + 1.0) / 2.0; // Map -1..+1 to 0..1
                            eval.record_prediction(
                                &vote.system,
                                prob,
                                vote.confidence,
                                true,
                                &step_summary,
                            );
                        }
                        crate::evaluation::save_evaluation(&self.db, &eval);
                    }

                    // Cortex: record experience for world model
                    {
                        let goal_desc = self
                            .db
                            .get_goal(&plan.goal_id)
                            .ok()
                            .flatten()
                            .map(|g| g.description.clone())
                            .unwrap_or_default();
                        let ctx_tags = crate::cortex::build_context_tags(
                            &step,
                            &goal_desc,
                            if plan.steps.is_empty() {
                                0.0
                            } else {
                                plan.current_step as f32 / plan.steps.len() as f32
                            },
                            cycle_count,
                        );
                        let mut cortex = crate::cortex::load_cortex(&self.db);
                        cortex.record(
                            &crate::cortex::step_to_action_name(&step),
                            ctx_tags,
                            true,
                            1.0,
                            None,
                        );
                        crate::cortex::save_cortex(&self.db, &cortex);

                        // Hivemind: deposit positive pheromone on successful step
                        let mut hive = crate::hivemind::load_hivemind(&self.db);
                        let file_path = match &step {
                            PlanStep::ReadFile { path, .. }
                            | PlanStep::GenerateCode {
                                file_path: path, ..
                            }
                            | PlanStep::EditCode {
                                file_path: path, ..
                            } => Some(path.as_str()),
                            _ => None,
                        };
                        let instance_id = self.config.instance_id.as_deref().unwrap_or("unknown");
                        let goal_kw = crate::genesis::extract_keywords_pub(&goal_desc);
                        hive.deposit_from_step(
                            &crate::cortex::step_to_action_name(&step),
                            file_path,
                            &goal_kw,
                            true,
                            instance_id,
                        );
                        crate::hivemind::save_hivemind(&self.db, &hive);
                    }

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

                    // CRITICAL: Rate limit errors are transient infra issues.
                    // Do NOT record them as capability/brain/cortex/hivemind failures —
                    // that permanently poisons the learning systems against LLM-dependent tools.
                    let is_rate_limited = feedback::is_rate_limit_error(&error);

                    if !is_rate_limited {
                        // Track capability failure (only for real failures)
                        capability::record_step_result(&self.db, &step, false, &error);
                        // Online brain learning — learn from failures immediately
                        let err_cat = crate::feedback::classify_error(&error);
                        crate::brain::train_on_step(
                            &self.db,
                            &step,
                            false,
                            Some(err_cat.clone()),
                            &brain_ctx,
                        );

                        // Synthesis + Evaluation: record failure for metacognitive tracking
                        {
                            let cortex_f = crate::cortex::load_cortex(&self.db);
                            let gene_pool_f = crate::genesis::load_gene_pool(&self.db);
                            let hivemind_f = crate::hivemind::load_hivemind(&self.db);
                            let goal_desc_f = self
                                .db
                                .get_goal(&plan.goal_id)
                                .ok()
                                .flatten()
                                .map(|g| g.description.clone())
                                .unwrap_or_default();
                            let mut synth = crate::synthesis::load_synthesis(&self.db);
                            let unified = synth.predict_step(
                                &step,
                                &prediction,
                                &cortex_f,
                                &gene_pool_f,
                                &hivemind_f,
                                &goal_desc_f,
                            );
                            synth.record_outcome(&unified.votes, false);
                            crate::synthesis::save_synthesis(&self.db, &synth);

                            let mut eval = crate::evaluation::load_evaluation(&self.db);
                            for vote in &unified.votes {
                                let prob = (vote.prediction + 1.0) / 2.0;
                                eval.record_prediction(
                                    &vote.system,
                                    prob,
                                    vote.confidence,
                                    false,
                                    &step_summary,
                                );
                            }
                            crate::evaluation::save_evaluation(&self.db, &eval);
                        }

                        // Cortex: record failure experience for world model
                        {
                            let goal_desc_for_cortex = self
                                .db
                                .get_goal(&plan.goal_id)
                                .ok()
                                .flatten()
                                .map(|g| g.description.clone())
                                .unwrap_or_default();
                            let ctx_tags = crate::cortex::build_context_tags(
                                &step,
                                &goal_desc_for_cortex,
                                if plan.steps.is_empty() {
                                    0.0
                                } else {
                                    plan.current_step as f32 / plan.steps.len() as f32
                                },
                                cycle_count,
                            );
                            let mut cortex = crate::cortex::load_cortex(&self.db);
                            cortex.record(
                                &crate::cortex::step_to_action_name(&step),
                                ctx_tags,
                                false,
                                -0.5,
                                Some(format!("{:?}", err_cat)),
                            );
                            crate::cortex::save_cortex(&self.db, &cortex);

                            // Hivemind: deposit negative pheromone on failed step
                            let mut hive = crate::hivemind::load_hivemind(&self.db);
                            let file_path_for_hive = match &step {
                                PlanStep::ReadFile { path, .. }
                                | PlanStep::GenerateCode {
                                    file_path: path, ..
                                }
                                | PlanStep::EditCode {
                                    file_path: path, ..
                                } => Some(path.as_str()),
                                _ => None,
                            };
                            let instance_id =
                                self.config.instance_id.as_deref().unwrap_or("unknown");
                            let goal_kw =
                                crate::genesis::extract_keywords_pub(&goal_desc_for_cortex);
                            hive.deposit_from_step(
                                &crate::cortex::step_to_action_name(&step),
                                file_path_for_hive,
                                &goal_kw,
                                false,
                                instance_id,
                            );
                            crate::hivemind::save_hivemind(&self.db, &hive);
                        }

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
                    } else {
                        tracing::info!(
                            step = %step_summary,
                            "Rate limit hit — skipping learning system updates to prevent poisoning"
                        );
                    }

                    return self
                        .handle_step_failure(llm, &mut plan, &step_summary, &error)
                        .await;
                }
                StepResult::RateLimited(msg) => {
                    tracing::warn!(step = %step_summary, reason = %msg, "Rate limited — backing off");
                    // Don't poison learning systems with rate-limit failures
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                    return Ok(CycleResult {
                        step_type: StepType::Mechanical,
                        entered_code: false,
                        summary: format!("Rate limited, retrying later: {msg}"),
                    });
                }
                StepResult::NeedsReplan(reason) => {
                    tracing::info!(step = %step_summary, reason = %reason, "Step needs replan");

                    // Rate-limit protection: don't poison learning systems
                    let is_rate_limited_replan = feedback::is_rate_limit_error(&reason);

                    if !is_rate_limited_replan {
                        // Track capability failure
                        capability::record_step_result(&self.db, &step, false, &reason);
                        // Online brain learning
                        let err_cat = crate::feedback::classify_error(&reason);
                        crate::brain::train_on_step(
                            &self.db,
                            &step,
                            false,
                            Some(err_cat.clone()),
                            &brain_ctx,
                        );

                        // Cortex: record replan experience
                        {
                            let goal_desc_for_cortex = self
                                .db
                                .get_goal(&plan.goal_id)
                                .ok()
                                .flatten()
                                .map(|g| g.description.clone())
                                .unwrap_or_default();
                            let ctx_tags = crate::cortex::build_context_tags(
                                &step,
                                &goal_desc_for_cortex,
                                if plan.steps.is_empty() {
                                    0.0
                                } else {
                                    plan.current_step as f32 / plan.steps.len() as f32
                                },
                                cycle_count,
                            );
                            let mut cortex = crate::cortex::load_cortex(&self.db);
                            cortex.record(
                                &crate::cortex::step_to_action_name(&step),
                                ctx_tags,
                                false,
                                -0.3,
                                Some(format!("{:?}", err_cat)),
                            );
                            crate::cortex::save_cortex(&self.db, &cortex);
                        }

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
                    }

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
    pub(super) fn record_step_progress(&self, plan: &Plan, summary: &str) {
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
}
