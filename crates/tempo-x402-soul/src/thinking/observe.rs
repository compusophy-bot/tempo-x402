//! Observation phase: snapshot recording, auto-belief sync, and model updates.
use super::*;

impl ThinkingLoop {
    pub(super) fn observe(
        &self,
        snapshot: &NodeSnapshot,
        pacer: &AdaptivePacer,
    ) -> Result<(), SoulError> {
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

    pub(super) fn sync_auto_beliefs(&self, snapshot: &NodeSnapshot) {
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
    pub(super) fn apply_model_updates(&self, text: &str) -> (u32, String) {
        let json_block = crate::normalize::extract_json_array(text);
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

    pub(super) fn apply_single_update(
        &self,
        update: &ModelUpdate,
        now: i64,
    ) -> Result<bool, SoulError> {
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
                    similarity > 0.3 // 30% overlap with abandoned = retread (was 50%, too lenient)
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
}
