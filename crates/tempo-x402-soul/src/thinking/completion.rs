//! Plan completion and step failure handling.
use super::*;

impl ThinkingLoop {
    pub(super) async fn complete_plan(
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

        // Genesis: record successful plan + close template feedback loop
        // Only record substantive plans — trivial read-only plans pollute the gene pool
        {
            let is_substantive = plan.executed_substantive();
            let step_types: Vec<String> = plan
                .steps
                .iter()
                .map(crate::cortex::step_to_action_name)
                .collect();
            let instance_id = self.config.instance_id.as_deref().unwrap_or("unknown");
            let mut gene_pool = crate::genesis::load_gene_pool(&self.db);
            if is_substantive {
                gene_pool.record_success(&goal.description, step_types, instance_id);
            } else {
                tracing::info!(
                    plan_id = %plan.id,
                    "Skipping genesis recording — plan was trivial (read-only)"
                );
            }
            // Feedback: if this plan was influenced by a template, record its success/failure
            if let Some(tid_str) = self
                .db
                .get_state("active_genesis_template_id")
                .ok()
                .flatten()
            {
                if let Ok(tid) = tid_str.parse::<u64>() {
                    if is_substantive {
                        gene_pool.record_template_success(tid);
                        tracing::info!(template_id = tid, "Genesis template feedback: SUCCESS");
                    } else {
                        gene_pool.record_failure(tid);
                        tracing::info!(
                            template_id = tid,
                            "Genesis template feedback: TRIVIAL (counted as failure)"
                        );
                    }
                }
            }
            // Inject seed templates if pool has no substantive templates
            crate::genesis::inject_seed_templates(&mut gene_pool, instance_id);
            // Enforce diversity
            crate::genesis::enforce_diversity(&mut gene_pool);
            crate::genesis::save_gene_pool(&self.db, &gene_pool);
        }

        self.increment_cycle_count()?;
        Ok(CycleResult {
            step_type: StepType::PlanCompleted,
            entered_code: false,
            summary: format!("plan {} completed ({} steps)", plan.id, plan.steps.len()),
        })
    }

    /// Handle a failed step — replan or fail the plan.
    pub(super) async fn handle_step_failure(
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
        // IMPORTANT: Rate limits (429) are NOT unsolvable — they're transient infra issues.
        // Treating them as unsolvable poisons durable rules, hivemind, cortex, and brain.
        let is_rate_limited = feedback::is_rate_limit_error(error);
        let unsolvable = !is_rate_limited
            && (error.contains("Peers found: 0")
                || error.contains("unable to auto-detect email address")
                || error.to_lowercase().contains("protected")
                || error.to_lowercase().contains("guard"));
        if is_rate_limited {
            tracing::warn!(
                plan_id = %plan.id,
                error = %error,
                "Rate limit hit — pausing plan (NOT recording as failure to avoid poisoning)"
            );
            // Don't fail the plan — just pause it. The next cycle will retry.
            // Increase cycle interval to back off.
            self.increment_cycle_count()?;
            return Ok(CycleResult {
                step_type: StepType::Llm,
                entered_code: false,
                summary: format!("rate limited — backing off (plan {} paused)", plan.id),
            });
        }
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
            // Genesis feedback: record template failure
            if let Some(tid_str) = self
                .db
                .get_state("active_genesis_template_id")
                .ok()
                .flatten()
            {
                if let Ok(tid) = tid_str.parse::<u64>() {
                    let mut gene_pool = crate::genesis::load_gene_pool(&self.db);
                    gene_pool.record_failure(tid);
                    crate::genesis::save_gene_pool(&self.db, &gene_pool);
                    tracing::info!(template_id = tid, "Genesis template feedback: FAILURE");
                }
            }
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
                let mut new_steps = crate::normalize::sanitize_plan_steps(
                    crate::normalize::parse_plan_steps(&response, self.config.max_plan_steps)?,
                );

                // Auto-fix: insert CargoCheck before Commit if missing
                validation::auto_fix_cargo_check(&mut new_steps);

                // Validate replanned steps too
                let goal_desc = goal.description.clone();
                let replan_validation = validation::validate_plan_with_coding(
                    &new_steps,
                    &self.db,
                    &goal_desc,
                    self.config.coding_enabled,
                );
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
}
