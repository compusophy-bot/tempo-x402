//! Plan creation: workspace analysis, validation, cortex simulation, and LLM-driven planning.
use super::*;

impl ThinkingLoop {
    pub(super) async fn create_plan(
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
        let role_guide = String::new(); // Removed hardcoded roles — colony.rs handles differentiation
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

        // Inject all cognitive systems into planning
        let gene_pool = crate::genesis::load_gene_pool(&self.db);
        let template_section = gene_pool.prompt_section(&goal.description);
        // Record best matching template ID for feedback loop
        let best_template_id = gene_pool
            .suggest_templates(&goal.description, 1)
            .first()
            .map(|(t, _)| t.id);
        if let Some(tid) = best_template_id {
            let _ = self
                .db
                .set_state("active_genesis_template_id", &tid.to_string());
        } else {
            let _ = self.db.set_state("active_genesis_template_id", "");
        }
        let cortex = crate::cortex::load_cortex(&self.db);
        let cortex_section = cortex.curiosity_report();
        let hive = crate::hivemind::load_hivemind(&self.db);
        let hive_section = hive.prompt_section();
        let synth = crate::synthesis::load_synthesis(&self.db);
        let synth_section = synth.prompt_section();

        // Free energy: inject regime + surprise decomposition into planning
        let fe_section = crate::free_energy::prompt_section(&self.db);
        // Colony: inject rank + specialization niche into planning
        let colony_section = crate::colony::prompt_section(&self.db);

        // Imagination: generate plans WITHOUT LLM from causal graph
        let imagined = synth.imagine_plans(&cortex, &gene_pool, &goal.description);
        let imagine_section = if imagined.is_empty() {
            String::new()
        } else {
            let mut lines = vec![
                "# Imagined Plans (COPY THESE — generated from world model, proven to work)"
                    .to_string(),
                "These step sequences are predicted to succeed. Use them directly:".to_string(),
            ];
            // Show best imagined plan as copyable JSON
            if let Some(best) = imagined.iter().max_by(|a, b| {
                a.id.partial_cmp(&b.id)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                lines.push(format!(
                    "\nBest plan (ID: {}):",
                    best.id
                ));
                lines.push("```json".to_string());
                let json_steps_val = serde_json::to_string(&best.steps).unwrap_or_default();
                lines.push(json_steps_val.clone());
                lines.push("```".to_string());
                lines.push(format!("Reasoning: {}", best.id));
            }
            // Also list alternatives compactly
            for (i, plan) in imagined.iter().enumerate().take(3) {
                lines.push(format!(
                    "Alt {}: {} (Status: {:?})",
                    i + 1,
                    plan.id,
                    plan.status
                ));
            }
            lines.join("\n")
        };
        // Track imagination for evaluation
        {
            let mut eval = crate::evaluation::load_evaluation(&self.db);
            eval.record_imagination(imagined.len() as u64, false, None);
            crate::evaluation::save_evaluation(&self.db, &eval);
        }
        crate::synthesis::save_synthesis(&self.db, &synth);

        // Plan transformer: if trained, generate a plan and inject as strong hint
        let transformer_section = match crate::model::generate_plan(&self.db, &goal.description) {
            Some(steps) => {
                let steps_json: Vec<String> = steps
                    .iter()
                    .map(|s| format!("  {{\"type\": \"{}\"}}", s))
                    .collect();
                format!(
                        "# Transformer Plan (generated by 284K-param model trained on colony experience)\n\
                         This plan was generated WITHOUT LLM by the local transformer model.\n\
                         Use it as your starting point — adapt details but follow the step structure:\n\
                         ```json\n[\n{}\n]\n```",
                        steps_json.join(",\n")
                    )
            }
            None => String::new(),
        };

        // Code quality model: tell the agent what its quality model predicts
        let quality_section = {
            let qm = crate::code_quality::load_model(&self.db);
            if qm.train_steps > 5 {
                format!(
                    "# Code Quality Model (trained on {} examples)\n\
                     Your quality model has learned from past commits. It will evaluate your diff \
                     before committing. Focus on changes that add test coverage, use Result/Option \
                     types, and avoid .unwrap()/.expect(). The model blocks commits predicted to regress.",
                    qm.train_steps
                )
            } else {
                String::new()
            }
        };

        // Lifecycle: tell the agent what phase it's in and encourage differentiation
        let lifecycle_section = crate::lifecycle::prompt_section(&self.db);

        let accel_section = crate::acceleration::prompt_section(&self.db);

        let extra = [
            template_section,
            cortex_section,
            hive_section,
            synth_section,
            fe_section,
            accel_section,
            colony_section,
            imagine_section,
            transformer_section,
            quality_section,
            lifecycle_section,
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
        let prompt = if extra.is_empty() {
            prompt
        } else {
            format!("{prompt}\n\n{extra}")
        };

        let system =
            "You are a software engineering planner. Output ONLY a JSON array of plan steps.";

        match llm.think(system, &prompt).await {
            Ok(response) => {
                let steps =
                    crate::normalize::parse_plan_steps(&response, self.config.max_plan_steps)?;
                let mut steps = crate::normalize::sanitize_plan_steps(steps);
                if steps.is_empty() {
                    tracing::warn!("Plan sanitization removed all steps — skipping plan creation");
                    return Ok(None);
                }

                // Auto-fix: insert CargoCheck before Commit if missing (prevents
                // weaker models from getting stuck in validation rejection loops)
                validation::auto_fix_cargo_check(&mut steps);

                // ── Mechanical plan validation ──
                // Hard checks that reject bad plans BEFORE execution.
                // This is server-side enforcement, not prompt injection.
                let validation = validation::validate_plan_with_coding(
                    &steps,
                    &self.db,
                    &goal.description,
                    self.config.coding_enabled,
                );
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

                // ── Cortex mental simulation ──
                // Predict plan outcome before executing. Log predictions; block
                // only if cortex is highly confident the plan will fail.
                let cortex = crate::cortex::load_cortex(&self.db);
                let simulation = cortex.simulate_plan(&steps);
                if simulation.confidence > 0.5 {
                    tracing::info!(
                        predicted_success = format!("{:.1}%", simulation.predicted_success * 100.0),
                        confidence = format!("{:.1}%", simulation.confidence * 100.0),
                        novel_steps = ?simulation.novel_steps,
                        risky_steps = ?simulation.risky_steps,
                        "Cortex plan simulation"
                    );
                    // Block if cortex predicts near-certain failure with high confidence
                    if simulation.predicted_success < 0.05 && simulation.confidence > 0.7 {
                        tracing::warn!(
                            "Cortex predicts plan will fail ({:.1}% success, {:.1}% confidence) — rejecting",
                            simulation.predicted_success * 100.0,
                            simulation.confidence * 100.0,
                        );
                        let _ = self.db.insert_nudge(
                            "cortex",
                            &format!(
                                "Plan rejected by cortex simulation: {:.0}% predicted success. Risky steps: {:?}",
                                simulation.predicted_success * 100.0,
                                simulation.risky_steps,
                            ),
                            3,
                        );
                        return Ok(None);
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
}
