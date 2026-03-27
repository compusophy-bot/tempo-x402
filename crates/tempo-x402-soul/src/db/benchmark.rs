// Benchmark run operations (Opus IQ Benchmark)
use super::*;

impl SoulDatabase {
    /// Insert a benchmark run.
    pub fn insert_benchmark_run(
        &self,
        run: &crate::benchmark::BenchmarkRun,
    ) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO benchmark_runs (id, task_id, entry_point, passed, \
             generated_solution, error_output, total_ms, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.id,
                run.task_id,
                run.entry_point,
                run.passed as i32,
                run.generated_solution,
                run.error_output,
                run.total_ms as i64,
                run.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get all benchmark runs (for scoring).
    pub fn get_all_benchmark_runs(&self) -> Result<Vec<crate::benchmark::BenchmarkRun>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, task_id, entry_point, passed, generated_solution, \
             error_output, total_ms, created_at \
             FROM benchmark_runs ORDER BY created_at DESC",
        )?;

        let runs = stmt
            .query_map([], |row| {
                let passed: i32 = row.get(3)?;
                Ok(crate::benchmark::BenchmarkRun {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    entry_point: row.get(2)?,
                    passed: passed != 0,
                    generated_solution: row.get(4)?,
                    error_output: row.get(5)?,
                    total_ms: row.get::<_, i64>(6)? as u64,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(runs)
    }

    /// Get recent benchmark runs (for display).
    pub fn get_recent_benchmark_runs(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::benchmark::BenchmarkRun>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, task_id, entry_point, passed, generated_solution, \
             error_output, total_ms, created_at \
             FROM benchmark_runs ORDER BY created_at DESC LIMIT ?1",
        )?;

        let runs = stmt
            .query_map(params![limit], |row| {
                let passed: i32 = row.get(3)?;
                Ok(crate::benchmark::BenchmarkRun {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    entry_point: row.get(2)?,
                    passed: passed != 0,
                    generated_solution: row.get(4)?,
                    error_output: row.get(5)?,
                    total_ms: row.get::<_, i64>(6)? as u64,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(runs)
    }
}
