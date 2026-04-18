// Benchmark run operations (Opus IQ Benchmark)
use super::*;

impl SoulDatabase {
    /// Insert a benchmark run.
    pub fn insert_benchmark_run(
        &self,
        run: &crate::benchmark::BenchmarkRun,
    ) -> Result<(), SoulError> {
        let value = serde_json::to_vec(run)?;
        self.benchmark_runs.insert(run.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get all benchmark runs (for scoring).
    pub fn get_all_benchmark_runs(&self) -> Result<Vec<crate::benchmark::BenchmarkRun>, SoulError> {
        let mut runs: Vec<crate::benchmark::BenchmarkRun> = self
            .benchmark_runs
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice(&v).ok())
            .collect();

        runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(runs)
    }

    /// Get recent benchmark runs (for display).
    pub fn get_recent_benchmark_runs(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::benchmark::BenchmarkRun>, SoulError> {
        let mut runs = self.get_all_benchmark_runs()?;
        runs.truncate(limit as usize);
        Ok(runs)
    }
}
