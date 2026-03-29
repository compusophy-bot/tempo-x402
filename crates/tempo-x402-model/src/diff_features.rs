//! Diff feature extraction — convert git diff output into a numeric feature vector.
//!
//! Pure function: takes diff text, returns feature array. No I/O, no DB, no runtime deps.
//! Used by the Code Quality Model to predict whether a commit is an improvement.

/// Number of features in the diff vector.
pub const DIFF_FEATURE_DIM: usize = 32;

/// Features extracted from a git diff.
#[derive(Debug, Clone, Default)]
pub struct DiffFeatures {
    pub features: [f32; DIFF_FEATURE_DIM],
}

impl DiffFeatures {
    /// Extract features from `git diff --numstat` output and optional `git diff` full output.
    ///
    /// `numstat` format: "added\tdeleted\tfile_path" per line
    /// `full_diff` is the unified diff for pattern detection (optional).
    pub fn from_diff(
        numstat: &str,
        full_diff: &str,
        current_iq: f32,
        current_fitness: f32,
    ) -> Self {
        let mut f = [0.0f32; DIFF_FEATURE_DIM];

        let mut total_added: usize = 0;
        let mut total_deleted: usize = 0;
        let mut files_touched: usize = 0;
        let mut new_files: usize = 0;
        let mut major_changes: usize = 0; // files with >50% change

        for line in numstat.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                continue;
            }
            let added: usize = parts[0].parse().unwrap_or(0);
            let deleted: usize = parts[1].parse().unwrap_or(0);
            let path = parts[2];

            total_added += added;
            total_deleted += deleted;
            files_touched += 1;

            if deleted == 0 && added > 0 {
                // Check if it's a new file (all additions, no deletions)
                if !path.contains('/') || added > 20 {
                    new_files += 1;
                }
            }

            let total_change = added + deleted;
            if total_change > 0 && deleted as f64 / total_change as f64 > 0.5 {
                major_changes += 1;
            }
        }

        // Normalize to reasonable ranges
        let norm = |v: usize, max: f32| -> f32 { (v as f32 / max).min(1.0) };

        f[0] = norm(total_added, 500.0); // LOC added (normalized to 500)
        f[1] = norm(total_deleted, 500.0); // LOC deleted
        f[2] = (total_added as f32 - total_deleted as f32) / 500.0; // Net LOC (signed)
        f[3] = norm(files_touched, 20.0); // Files touched
        f[4] = norm(new_files, 5.0); // New files created
        f[5] = norm(major_changes, 5.0); // Files with >50% change

        // Pattern detection from full diff
        let diff_lower = full_diff.to_lowercase();
        let count_pattern = |pat: &str| -> f32 { diff_lower.matches(pat).count() as f32 };

        f[6] = norm(count_additions(&diff_lower, "fn "), 10.0); // Function signatures changed
        f[7] = norm(count_additions(&diff_lower, "use "), 10.0); // Imports added
        f[8] = count_pattern("unsafe {").min(5.0) / 5.0; // unsafe blocks
        f[9] = count_pattern(".unwrap()").min(10.0) / 10.0; // .unwrap() usage
        f[10] = count_pattern(".expect(").min(10.0) / 10.0; // .expect() usage
        f[11] = count_pattern("todo!").min(5.0) / 5.0; // todo! macros
        f[12] = count_pattern("unimplemented!").min(5.0) / 5.0; // unimplemented! macros
        f[13] = count_pattern("println!").min(5.0) / 5.0; // println! (debug leftover)
        f[14] = norm(count_additions(&diff_lower, "#[test]"), 10.0); // Tests added
        f[15] = norm(count_additions(&diff_lower, "#[cfg(test)]"), 3.0); // Test modules added

        // File type analysis
        f[16] = if diff_lower.contains("cargo.toml") {
            1.0
        } else {
            0.0
        }; // Touches Cargo.toml
        f[17] = if diff_lower.contains("guard.rs") || diff_lower.contains("llm.rs") {
            1.0
        } else {
            0.0
        }; // Touches protected files
        let numstat_lower = numstat.to_lowercase();
        f[18] = if numstat_lower.contains("temp.rs")
            || numstat_lower.contains("tmp.rs")
            || diff_lower.contains("temp.rs")
        {
            1.0
        } else {
            0.0
        }; // Junk files

        // Duplication detection (simple: count lines that appear in both + and - sections)
        let added_lines: Vec<&str> = full_diff
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .map(|l| l.trim_start_matches('+').trim())
            .filter(|l| l.len() > 10)
            .collect();
        let removed_lines: Vec<&str> = full_diff
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .map(|l| l.trim_start_matches('-').trim())
            .filter(|l| l.len() > 10)
            .collect();
        let moved_lines = added_lines
            .iter()
            .filter(|a| removed_lines.contains(a))
            .count();
        f[19] = norm(moved_lines, 50.0); // Lines that were moved (not changed)

        // Comment-only change detection
        let comment_lines = added_lines
            .iter()
            .filter(|l| l.starts_with("//") || l.starts_with("///") || l.starts_with("//!"))
            .count();
        f[20] = if total_added > 0 {
            comment_lines as f32 / total_added as f32
        } else {
            0.0
        }; // Comment ratio (1.0 = all comments)

        // Context features (agent state)
        f[21] = current_iq / 150.0; // Agent's IQ (normalized to max 150)
        f[22] = current_fitness; // Agent's fitness (already 0-1)

        // f[23..31] reserved for future features

        DiffFeatures { features: f }
    }

    /// Get the feature vector as a slice.
    pub fn as_slice(&self) -> &[f32] {
        &self.features
    }
}

/// Count lines in the diff that start with '+' and contain the given pattern.
fn count_additions(diff_lower: &str, pattern: &str) -> usize {
    diff_lower
        .lines()
        .filter(|l| l.starts_with('+') && l.contains(pattern))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_diff_features() {
        let numstat = "10\t2\tsrc/main.rs\n5\t0\tsrc/new_file.rs";
        let diff = "+fn new_function() {\n+    println!(\"hello\");\n+}\n-fn old_function() {\n-}";

        let features = DiffFeatures::from_diff(numstat, diff, 115.0, 0.65);

        assert!(features.features[0] > 0.0); // LOC added
        assert!(features.features[3] > 0.0); // Files touched
        assert!(features.features[13] > 0.0); // println! detected
    }

    #[test]
    fn test_empty_diff() {
        let features = DiffFeatures::from_diff("", "", 100.0, 0.5);
        assert_eq!(features.features[0], 0.0);
        assert_eq!(features.features[3], 0.0);
    }

    #[test]
    fn test_junk_file_detection() {
        let numstat = "100\t0\ttemp.rs";
        let diff = "+// junk file";
        let features = DiffFeatures::from_diff(numstat, diff, 100.0, 0.5);
        assert_eq!(features.features[18], 1.0); // Junk file detected
    }
}
