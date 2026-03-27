//! Goal creation: first-boot seeding and LLM-driven goal generation.
use super::*;

impl ThinkingLoop {
    pub(super) async fn create_goals(
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

            if self.config.coding_enabled {
                // First goal: make a code change to prove the pipeline works
                seed_goals.push((
                    "Make your first code change: read crates/tempo-x402-soul/src/housekeeping.rs, \
                     then use edit_file to add a useful comment or improve a log message. \
                     Run cargo_check to verify it compiles, then commit the change. \
                     This establishes your ability to self-modify.",
                    "A commit exists with a change to housekeeping.rs that passes cargo check",
                    if self.config.initial_goal.is_some() {
                        4
                    } else {
                        5
                    },
                ));
            } else {
                // Coding disabled — focus on observation, benchmarking, learning
                seed_goals.push((
                    "Run the Opus IQ benchmark to measure your intelligence. Observe your \
                     node's health, endpoints, and beliefs. Record observations about the \
                     system state. Focus on thinking and learning — coding is disabled.",
                    "Benchmark has run at least once and IQ score is recorded",
                    5,
                ));
            }

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
        let opus_summary = crate::benchmark::opus_summary_for_prompt(&self.db);
        let brain_summary = crate::brain::brain_summary(&self.db);
        let cap_with_benchmark = {
            let mut s = cap_guidance;
            if !opus_summary.is_empty() {
                s = format!("{s}\n\n{opus_summary}");
            } else if !benchmark_summary.is_empty() {
                s = format!("{s}\n\n{benchmark_summary}");
            }
            if !brain_summary.is_empty() {
                s = format!("{s}\n\n{brain_summary}");
            }
            // Inject MoE expertise map — shows which peers are best at what
            let moe_section = crate::moe::load_router(&self.db).prompt_section();
            if !moe_section.is_empty() {
                s = format!("{s}\n\n{moe_section}");
            }
            s
        };
        let role_guide = String::new(); // Removed hardcoded roles — colony.rs handles differentiation
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

        // Inject all cognitive systems + recursive self-improvement into goal creation
        let cortex = crate::cortex::load_cortex(&self.db);
        let cortex_report = cortex.curiosity_report();
        let hive = crate::hivemind::load_hivemind(&self.db);
        let hive_report = hive.prompt_section();
        let synth = crate::synthesis::load_synthesis(&self.db);
        let synth_report = synth.prompt_section();
        let improvement_report = crate::autonomy::improvement_prompt(&self.db);
        let fe_report = crate::free_energy::prompt_section(&self.db);
        let extra_intel = [
            cortex_report,
            hive_report,
            synth_report,
            improvement_report,
            fe_report,
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
        let prompt = if extra_intel.is_empty() {
            prompt
        } else {
            format!("{prompt}\n\n{extra_intel}")
        };

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
}
