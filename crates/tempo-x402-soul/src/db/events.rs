// Structured event operations: insert, query, resolve, prune
use super::*;

impl SoulDatabase {
    /// Insert a structured event.
    pub fn insert_event(&self, event: &SoulEvent) -> Result<(), SoulError> {
        let value = serde_json::to_vec(event)?;
        self.events.insert(event.id.as_bytes(), value)?;
        Ok(())
    }

    /// Query events with filtering.
    pub fn query_events(&self, filter: &EventFilter) -> Result<Vec<SoulEvent>, SoulError> {
        let mut events: Vec<SoulEvent> = self
            .events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<SoulEvent>(&v).ok())
            .filter(|e| {
                if let Some(ref level) = filter.level {
                    if &e.level != level {
                        return false;
                    }
                }
                if let Some(ref prefix) = filter.code_prefix {
                    if !e.code.starts_with(prefix.as_str()) {
                        return false;
                    }
                }
                if let Some(ref category) = filter.category {
                    if &e.category != category {
                        return false;
                    }
                }
                if let Some(ref plan_id) = filter.plan_id {
                    if e.plan_id.as_deref() != Some(plan_id.as_str()) {
                        return false;
                    }
                }
                if let Some(resolved) = filter.resolved {
                    if e.resolved != resolved {
                        return false;
                    }
                }
                if let Some(since) = filter.since {
                    if e.created_at < since {
                        return false;
                    }
                }
                if let Some(until) = filter.until {
                    if e.created_at > until {
                        return false;
                    }
                }
                true
            })
            .collect();

        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let limit = filter.limit.min(200) as usize;
        let offset = filter.offset as usize;
        let result = events.into_iter().skip(offset).take(limit).collect();
        Ok(result)
    }

    /// Resolve a single event by ID.
    pub fn resolve_event(&self, id: &str, resolution: &str) -> Result<(), SoulError> {
        if let Some(raw) = self.events.get(id.as_bytes())? {
            let mut event: SoulEvent = serde_json::from_slice(&raw)?;
            event.resolved = true;
            event.resolved_at = Some(chrono::Utc::now().timestamp());
            event.resolution = Some(resolution.to_string());
            let value = serde_json::to_vec(&event)?;
            self.events.insert(id.as_bytes(), value)?;
        }
        Ok(())
    }

    /// Resolve all unresolved events matching a code prefix.
    pub fn resolve_events_by_code(
        &self,
        code_prefix: &str,
        resolution: &str,
    ) -> Result<u32, SoulError> {
        let now = chrono::Utc::now().timestamp();
        let mut count = 0u32;

        for item in self.events.iter() {
            let (key, val) = item?;
            let mut event: SoulEvent = serde_json::from_slice(&val)?;
            if !event.resolved && event.code.starts_with(code_prefix) {
                event.resolved = true;
                event.resolved_at = Some(now);
                event.resolution = Some(resolution.to_string());
                self.events.insert(key, serde_json::to_vec(&event)?)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Resolve all unresolved events matching a code prefix for a specific plan.
    pub fn resolve_events_by_plan(
        &self,
        code_prefix: &str,
        plan_id: &str,
        resolution: &str,
    ) -> Result<u32, SoulError> {
        let now = chrono::Utc::now().timestamp();
        let mut count = 0u32;

        for item in self.events.iter() {
            let (key, val) = item?;
            let mut event: SoulEvent = serde_json::from_slice(&val)?;
            if !event.resolved
                && event.code.starts_with(code_prefix)
                && event.plan_id.as_deref() == Some(plan_id)
            {
                event.resolved = true;
                event.resolved_at = Some(now);
                event.resolution = Some(resolution.to_string());
                self.events.insert(key, serde_json::to_vec(&event)?)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get unresolved events at a given level.
    pub fn get_unresolved_events(
        &self,
        level: &str,
        limit: u32,
    ) -> Result<Vec<SoulEvent>, SoulError> {
        let mut events: Vec<SoulEvent> = self
            .events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<SoulEvent>(&v).ok())
            .filter(|e| !e.resolved && e.level == level)
            .collect();

        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        events.truncate(limit as usize);
        Ok(events)
    }

    /// Count events at a given level since a timestamp.
    pub fn count_events_since(&self, level: &str, since: i64) -> Result<u64, SoulError> {
        let count = self
            .events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<SoulEvent>(&v).ok())
            .filter(|e| e.level == level && e.created_at >= since)
            .count();

        Ok(count as u64)
    }

    /// Get the most recent event at a given level.
    pub fn get_latest_event_by_level(&self, level: &str) -> Result<Option<SoulEvent>, SoulError> {
        let result = self
            .events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<SoulEvent>(&v).ok())
            .filter(|e| e.level == level)
            .max_by_key(|e| e.created_at);

        Ok(result)
    }

    /// Get top error codes by count in a time window.
    pub fn top_event_codes_since(
        &self,
        level: &str,
        since: i64,
        limit: u32,
    ) -> Result<Vec<ErrorCodeCount>, SoulError> {
        let mut code_counts: HashMap<String, u64> = HashMap::new();

        for item in self.events.iter() {
            let (_, val) = item?;
            if let Ok(event) = serde_json::from_slice::<SoulEvent>(&val) {
                if event.level == level && event.created_at >= since {
                    *code_counts.entry(event.code.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut codes: Vec<ErrorCodeCount> = code_counts
            .into_iter()
            .map(|(code, count)| ErrorCodeCount { code, count })
            .collect();

        codes.sort_by(|a, b| b.count.cmp(&a.count));
        codes.truncate(limit as usize);
        Ok(codes)
    }

    /// Prune events with tiered retention.
    pub fn prune_events(&self) -> Result<u32, SoulError> {
        let now = chrono::Utc::now().timestamp();
        let one_day = 86400i64;
        let mut total = 0u32;

        // Collect all events with their keys for pruning decisions
        let mut events: Vec<(sled::IVec, SoulEvent)> = self
            .events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(k, v)| serde_json::from_slice::<SoulEvent>(&v).ok().map(|e| (k, e)))
            .collect();

        // Determine which keys to delete based on tiered retention
        let mut to_delete: Vec<sled::IVec> = Vec::new();

        for (key, event) in &events {
            let should_delete = match event.level.as_str() {
                "debug" => event.created_at < now - one_day,
                "info" => event.created_at < now - one_day * 3,
                "warn" => {
                    if event.resolved {
                        event.created_at < now - one_day * 3
                    } else {
                        event.created_at < now - one_day * 7
                    }
                }
                "error" => {
                    if event.resolved {
                        event.created_at < now - one_day * 7
                    } else {
                        event.created_at < now - one_day * 30
                    }
                }
                _ => false,
            };
            if should_delete {
                to_delete.push(key.clone());
            }
        }

        for key in &to_delete {
            self.events.remove(key)?;
            total += 1;
        }

        // Hard cap: keep most recent 5000
        // Remove already-deleted events from our list
        let deleted_set: std::collections::HashSet<&[u8]> =
            to_delete.iter().map(|k| k.as_ref()).collect();
        events.retain(|(k, _)| !deleted_set.contains(k.as_ref()));

        if events.len() > 5000 {
            events.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));
            for (key, _) in events.iter().skip(5000) {
                self.events.remove(key)?;
                total += 1;
            }
        }

        Ok(total)
    }
}
