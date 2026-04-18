use super::problem;
use crate::benchmark::BenchmarkProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 4: REASONING — Logic puzzles + constraint satisfaction
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier4_reasoning() -> Vec<BenchmarkProblem> {
    vec![
        // ── 4.1: N-Queens ────────────────────────────────────────────
        problem(
            "opus-n-queens",
            "tier4",
            "Solve the N-Queens problem: place N queens on an NxN board such that no two \
             queens threaten each other. Return all solutions as vectors of column positions \
             (index i = row i, value = column of queen in that row). Solutions sorted lexicographically.",
            r#"pub fn n_queens(n: usize) -> Vec<Vec<usize>> {
    todo!()
}"#,
            r#"use opus_n_queens::*;

#[test]
fn zero() {
    assert_eq!(n_queens(0), vec![vec![]]);
}

#[test]
fn one() {
    assert_eq!(n_queens(1), vec![vec![0]]);
}

#[test]
fn four() {
    let solutions = n_queens(4);
    assert_eq!(solutions.len(), 2);
    assert!(solutions.contains(&vec![1, 3, 0, 2]));
    assert!(solutions.contains(&vec![2, 0, 3, 1]));
}

#[test]
fn eight_count() {
    assert_eq!(n_queens(8).len(), 92);
}

#[test]
fn no_conflicts() {
    for sol in n_queens(6) {
        let n = sol.len();
        for i in 0..n {
            for j in (i+1)..n {
                assert_ne!(sol[i], sol[j], "Same column");
                let row_diff = j - i;
                let col_diff = (sol[i] as i64 - sol[j] as i64).unsigned_abs() as usize;
                assert_ne!(row_diff, col_diff, "Same diagonal in {:?}", sol);
            }
        }
    }
}

#[test]
fn two_and_three_impossible() {
    assert!(n_queens(2).is_empty());
    assert!(n_queens(3).is_empty());
}"#,
        ),

        // ── 4.2: Water Jugs ─────────────────────────────────────────
        problem(
            "opus-water-jugs",
            "tier4",
            "Solve the water jug problem: given two jugs with capacities `a` and `b`, \
             find the minimum sequence of steps to measure exactly `target` liters in either jug. \
             Steps: FillA, FillB, EmptyA, EmptyB, PourAtoB, PourBtoA. \
             Return None if impossible.",
            r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    FillA, FillB, EmptyA, EmptyB, PourAtoB, PourBtoA,
}

pub fn solve(a: u32, b: u32, target: u32) -> Option<Vec<Step>> {
    todo!()
}"#,
            r#"use opus_water_jugs::*;

fn simulate(a_cap: u32, b_cap: u32, steps: &[Step]) -> (u32, u32) {
    let mut ja = 0u32;
    let mut jb = 0u32;
    for step in steps {
        match step {
            Step::FillA => ja = a_cap,
            Step::FillB => jb = b_cap,
            Step::EmptyA => ja = 0,
            Step::EmptyB => jb = 0,
            Step::PourAtoB => {
                let pour = ja.min(b_cap - jb);
                ja -= pour;
                jb += pour;
            }
            Step::PourBtoA => {
                let pour = jb.min(a_cap - ja);
                jb -= pour;
                ja += pour;
            }
        }
    }
    (ja, jb)
}

#[test]
fn three_five_four() {
    let steps = solve(3, 5, 4).expect("Should be solvable");
    let (ja, jb) = simulate(3, 5, &steps);
    assert!(ja == 4 || jb == 4, "Neither jug has 4: ({}, {})", ja, jb);
}

#[test]
fn five_three_four() {
    let steps = solve(5, 3, 4).expect("Should be solvable");
    let (ja, jb) = simulate(5, 3, &steps);
    assert!(ja == 4 || jb == 4);
}

#[test]
fn impossible() {
    assert!(solve(2, 4, 3).is_none()); // gcd(2,4)=2, 3 not divisible by 2
}

#[test]
fn target_zero() {
    let steps = solve(3, 5, 0).expect("Should be solvable");
    assert!(steps.is_empty()); // both start at 0
}

#[test]
fn target_equals_capacity() {
    let steps = solve(3, 5, 5).expect("Should be solvable");
    let (ja, jb) = simulate(3, 5, &steps);
    assert!(ja == 5 || jb == 5);
}

#[test]
fn is_minimal() {
    // 3,5,4 is solvable in 6 steps (known minimum)
    let steps = solve(3, 5, 4).unwrap();
    assert!(steps.len() <= 6, "Solution has {} steps, expected <= 6", steps.len());
}"#,
        ),

        // ── 4.3: Sudoku 4x4 ─────────────────────────────────────────
        problem(
            "opus-sudoku-4x4",
            "tier4",
            "Solve a 4x4 Sudoku puzzle. Input is a 4x4 grid where 0 means empty. \
             Each row, column, and 2x2 box must contain digits 1-4 exactly once. \
             Return the solved grid, or None if unsolvable.",
            r#"pub fn solve(grid: [[u8; 4]; 4]) -> Option<[[u8; 4]; 4]> {
    todo!()
}"#,
            r#"use opus_sudoku_4x4::*;

fn is_valid(grid: &[[u8; 4]; 4]) -> bool {
    // Check rows
    for row in grid {
        let mut seen = [false; 5];
        for &v in row {
            if v < 1 || v > 4 || seen[v as usize] { return false; }
            seen[v as usize] = true;
        }
    }
    // Check columns
    for c in 0..4 {
        let mut seen = [false; 5];
        for r in 0..4 {
            let v = grid[r][c];
            if seen[v as usize] { return false; }
            seen[v as usize] = true;
        }
    }
    // Check 2x2 boxes
    for br in [0, 2] {
        for bc in [0, 2] {
            let mut seen = [false; 5];
            for r in br..br+2 {
                for c in bc..bc+2 {
                    let v = grid[r][c];
                    if seen[v as usize] { return false; }
                    seen[v as usize] = true;
                }
            }
        }
    }
    true
}

#[test]
fn solve_basic() {
    let puzzle = [
        [1, 0, 0, 4],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [3, 0, 0, 2],
    ];
    let solution = solve(puzzle).expect("Should be solvable");
    assert!(is_valid(&solution));
    // Check that given clues are preserved
    assert_eq!(solution[0][0], 1);
    assert_eq!(solution[0][3], 4);
    assert_eq!(solution[3][0], 3);
    assert_eq!(solution[3][3], 2);
}

#[test]
fn already_solved() {
    let grid = [
        [1, 2, 3, 4],
        [3, 4, 1, 2],
        [2, 1, 4, 3],
        [4, 3, 2, 1],
    ];
    assert_eq!(solve(grid), Some(grid));
}

#[test]
fn unsolvable() {
    let bad = [
        [1, 1, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
    ];
    assert_eq!(solve(bad), None);
}

#[test]
fn minimal_clues() {
    let puzzle = [
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 1],
    ];
    let solution = solve(puzzle).expect("Should be solvable");
    assert!(is_valid(&solution));
    assert_eq!(solution[3][3], 1);
}"#,
        ),

        // ── 4.4: Topological Sort with Cycle Detection ──────────────
        problem(
            "opus-topo-sort",
            "tier4",
            "Implement topological sort on a DAG. Input is a list of (node, dependency) pairs. \
             Return a valid ordering where every node appears after all its dependencies, \
             or return Err with a cycle description if the graph has a cycle. \
             When multiple valid orderings exist, prefer lexicographically smallest.",
            r#"pub fn topo_sort(edges: &[(String, String)]) -> Result<Vec<String>, String> {
    todo!()
}"#,
            r#"use opus_topo_sort::*;

fn e(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn simple_chain() {
    let result = topo_sort(&e(&[("b", "a"), ("c", "b")])).unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn diamond() {
    let result = topo_sort(&e(&[("c", "a"), ("c", "b"), ("d", "c")])).unwrap();
    // a and b before c, c before d
    let pos = |s: &str| result.iter().position(|x| x == s).unwrap();
    assert!(pos("a") < pos("c"));
    assert!(pos("b") < pos("c"));
    assert!(pos("c") < pos("d"));
}

#[test]
fn cycle_detected() {
    let result = topo_sort(&e(&[("a", "b"), ("b", "c"), ("c", "a")]));
    assert!(result.is_err());
}

#[test]
fn empty_graph() {
    let result = topo_sort(&e(&[])).unwrap();
    assert!(result.is_empty());
}

#[test]
fn no_edges() {
    // Nodes with no dependencies — should be sorted lexicographically
    let result = topo_sort(&e(&[("c", "c")]));
    // Self-loop is a cycle
    assert!(result.is_err());
}

#[test]
fn lexicographic_preference() {
    // a, b, c are all independent
    let result = topo_sort(&e(&[("d", "a"), ("d", "b"), ("d", "c")])).unwrap();
    // a, b, c should come before d, and in alphabetical order
    assert_eq!(&result[..3], &["a", "b", "c"]);
    assert_eq!(result[3], "d");
}"#,
        ),

        // ── 4.5: Constraint Scheduler ────────────────────────────────
        problem(
            "opus-scheduler",
            "tier4",
            "Schedule N tasks with durations and dependency constraints to minimize total completion time. \
             Each task has a name, duration, and list of dependencies (tasks that must complete first). \
             Assume unlimited parallelism. Return (total_time, schedule) where schedule maps task -> start_time.",
            r#"use std::collections::HashMap;

pub struct Task {
    pub name: String,
    pub duration: u32,
    pub deps: Vec<String>,
}

pub fn schedule(tasks: &[Task]) -> Result<(u32, HashMap<String, u32>), String> {
    todo!()
}"#,
            r#"use opus_scheduler::*;

fn task(name: &str, dur: u32, deps: &[&str]) -> Task {
    Task { name: name.into(), duration: dur, deps: deps.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn single_task() {
    let (time, sched) = schedule(&[task("a", 5, &[])]).unwrap();
    assert_eq!(time, 5);
    assert_eq!(sched["a"], 0);
}

#[test]
fn sequential() {
    let (time, sched) = schedule(&[
        task("a", 3, &[]),
        task("b", 4, &["a"]),
    ]).unwrap();
    assert_eq!(time, 7);
    assert_eq!(sched["a"], 0);
    assert_eq!(sched["b"], 3);
}

#[test]
fn parallel() {
    let (time, sched) = schedule(&[
        task("a", 3, &[]),
        task("b", 5, &[]),
    ]).unwrap();
    assert_eq!(time, 5); // parallel
    assert_eq!(sched["a"], 0);
    assert_eq!(sched["b"], 0);
}

#[test]
fn diamond_dependency() {
    let (time, sched) = schedule(&[
        task("a", 2, &[]),
        task("b", 3, &["a"]),
        task("c", 1, &["a"]),
        task("d", 4, &["b", "c"]),
    ]).unwrap();
    // a:0-2, b:2-5, c:2-3, d: max(5,3)=5, ends at 9
    assert_eq!(time, 9);
    assert_eq!(sched["d"], 5);
}

#[test]
fn cycle_error() {
    let result = schedule(&[
        task("a", 1, &["b"]),
        task("b", 1, &["a"]),
    ]);
    assert!(result.is_err());
}"#,
        ),

        // ── 4.6: 2-SAT Solver ───────────────────────────────────────
        problem(
            "opus-two-sat",
            "tier4",
            "Solve a 2-SAT problem. Input: number of variables (1-indexed), and clauses \
             where each clause is (literal1, literal2). A positive literal i means variable i is true, \
             negative -i means variable i is false. Return Some(assignment) where assignment[i-1] \
             is the truth value of variable i, or None if unsatisfiable.",
            r#"pub fn solve_2sat(num_vars: usize, clauses: &[(i32, i32)]) -> Option<Vec<bool>> {
    todo!()
}"#,
            r#"use opus_two_sat::*;

fn check(assignment: &[bool], clauses: &[(i32, i32)]) -> bool {
    for &(a, b) in clauses {
        let va = if a > 0 { assignment[(a.unsigned_abs() - 1) as usize] } else { !assignment[(a.unsigned_abs() - 1) as usize] };
        let vb = if b > 0 { assignment[(b.unsigned_abs() - 1) as usize] } else { !assignment[(b.unsigned_abs() - 1) as usize] };
        if !va && !vb { return false; }
    }
    true
}

#[test]
fn simple_sat() {
    // (x1 OR x2)
    let result = solve_2sat(2, &[(1, 2)]).unwrap();
    assert!(check(&result, &[(1, 2)]));
}

#[test]
fn implies_chain() {
    // (x1 OR x2) AND (NOT x1 OR x3) AND (NOT x2 OR x3)
    // This forces x3 to be true
    let result = solve_2sat(3, &[(1, 2), (-1, 3), (-2, 3)]).unwrap();
    assert!(result[2]); // x3 must be true
    assert!(check(&result, &[(1, 2), (-1, 3), (-2, 3)]));
}

#[test]
fn unsatisfiable() {
    // (x1) AND (NOT x1) expressed as 2-SAT:
    // (x1 OR x1) AND (NOT x1 OR NOT x1)
    let result = solve_2sat(1, &[(1, 1), (-1, -1)]);
    assert!(result.is_none());
}

#[test]
fn all_negative() {
    // (NOT x1 OR NOT x2) AND (NOT x2 OR NOT x3)
    let result = solve_2sat(3, &[(-1, -2), (-2, -3)]).unwrap();
    assert!(check(&result, &[(-1, -2), (-2, -3)]));
}

#[test]
fn empty_clauses() {
    let result = solve_2sat(3, &[]).unwrap();
    assert_eq!(result.len(), 3);
}"#,
        ),

        // ── 4.7: Optimal Change with Limited Supply ──────────────────
        problem(
            "opus-change-maker",
            "tier4",
            "Make change for `amount` cents using coins with limited supply. \
             Input: Vec of (denomination, count). Return the minimum number of coins needed, \
             or None if impossible. Also return which coins were used.",
            r#"pub fn make_change(amount: u32, coins: &[(u32, u32)]) -> Option<(u32, Vec<(u32, u32)>)> {
    todo!()
}
// Returns (total_coins_used, Vec<(denomination, count_used)>)"#,
            r#"use opus_change_maker::*;

#[test]
fn exact_single_coin() {
    let (count, used) = make_change(25, &[(25, 1), (10, 5), (5, 5), (1, 10)]).unwrap();
    assert_eq!(count, 1);
    let total: u32 = used.iter().map(|(d, c)| d * c).sum();
    assert_eq!(total, 25);
}

#[test]
fn need_multiple() {
    let (count, used) = make_change(30, &[(25, 1), (10, 5), (5, 5), (1, 10)]).unwrap();
    let total: u32 = used.iter().map(|(d, c)| d * c).sum();
    assert_eq!(total, 30);
    assert!(count <= 3); // 25+5 = 2 coins, or 10+10+10 = 3
}

#[test]
fn impossible_amount() {
    assert!(make_change(3, &[(2, 5)]).is_none());
}

#[test]
fn limited_supply_forces_suboptimal() {
    // With unlimited 25s: 75 = 3x25 (3 coins)
    // With only 2x25: 75 = 2x25 + 2x10 + 1x5 (5 coins)
    let (count, used) = make_change(75, &[(25, 2), (10, 5), (5, 5), (1, 100)]).unwrap();
    let total: u32 = used.iter().map(|(d, c)| d * c).sum();
    assert_eq!(total, 75);
    // Can't use 3 quarters
    let quarters_used = used.iter().find(|(d, _)| *d == 25).map(|(_, c)| *c).unwrap_or(0);
    assert!(quarters_used <= 2);
}

#[test]
fn zero_amount() {
    let (count, _) = make_change(0, &[(1, 10)]).unwrap();
    assert_eq!(count, 0);
}"#,
        ),

        // ── 4.8: Expression to Truth Table ───────────────────────────
        problem(
            "opus-truth-table",
            "tier4",
            "Given a boolean expression with variables (a-z), AND (&), OR (|), NOT (!), \
             and parentheses, generate its truth table. Variables are listed alphabetically. \
             Return the list of variable names and a Vec of (inputs, output) rows.",
            r#"pub fn truth_table(expr: &str) -> (Vec<char>, Vec<(Vec<bool>, bool)>) {
    todo!()
}"#,
            r#"use opus_truth_table::*;

#[test]
fn single_var() {
    let (vars, rows) = truth_table("a");
    assert_eq!(vars, vec!['a']);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], (vec![false], false));
    assert_eq!(rows[1], (vec![true], true));
}

#[test]
fn and_gate() {
    let (vars, rows) = truth_table("a & b");
    assert_eq!(vars, vec!['a', 'b']);
    assert_eq!(rows.len(), 4);
    // Only true when both true
    assert_eq!(rows[3], (vec![true, true], true));
    assert_eq!(rows[0], (vec![false, false], false));
}

#[test]
fn or_gate() {
    let (_, rows) = truth_table("a | b");
    // false only when both false
    assert_eq!(rows[0].1, false);
    assert_eq!(rows[1].1, true);
    assert_eq!(rows[2].1, true);
    assert_eq!(rows[3].1, true);
}

#[test]
fn not_gate() {
    let (vars, rows) = truth_table("!a");
    assert_eq!(vars, vec!['a']);
    assert_eq!(rows[0], (vec![false], true));
    assert_eq!(rows[1], (vec![true], false));
}

#[test]
fn complex() {
    let (vars, rows) = truth_table("(a & b) | (!a & c)");
    assert_eq!(vars, vec!['a', 'b', 'c']);
    assert_eq!(rows.len(), 8);
    // a=T, b=T, c=F -> (T&T)|(F&F) = T|F = T
    assert_eq!(rows[6], (vec![true, true, false], true));
    // a=F, b=F, c=T -> (F&F)|(T&T) = F|T = T
    assert_eq!(rows[1], (vec![false, false, true], true));
}

#[test]
fn de_morgan() {
    // !(a & b) should equal !a | !b
    let (_, rows1) = truth_table("!(a & b)");
    let (_, rows2) = truth_table("!a | !b");
    for (r1, r2) in rows1.iter().zip(rows2.iter()) {
        assert_eq!(r1.1, r2.1, "De Morgan failed for {:?}", r1.0);
    }
}"#,
        ),

        // ── 4.9: Regex Matcher ───────────────────────────────────────
        problem(
            "opus-regex-match",
            "tier4",
            "Implement a simple regex matcher supporting: literal chars, `.` (any char), \
             `*` (zero or more of previous), `+` (one or more of previous), `?` (zero or one of previous), \
             `^` (start anchor), `$` (end anchor). The match is against the full string unless anchors \
             are used. Without anchors, the pattern can match anywhere in the string.",
            r#"pub fn regex_match(pattern: &str, text: &str) -> bool {
    todo!()
}"#,
            r#"use opus_regex_match::*;

#[test] fn literal() { assert!(regex_match("hello", "hello")); }
#[test] fn literal_fail() { assert!(!regex_match("hello", "world")); }
#[test] fn dot() { assert!(regex_match("h.llo", "hello")); }
#[test] fn star() { assert!(regex_match("ab*c", "ac")); }
#[test] fn star_many() { assert!(regex_match("ab*c", "abbbbc")); }
#[test] fn plus() { assert!(regex_match("ab+c", "abc")); }
#[test] fn plus_fail() { assert!(!regex_match("ab+c", "ac")); }
#[test] fn question() { assert!(regex_match("ab?c", "ac")); }
#[test] fn question_one() { assert!(regex_match("ab?c", "abc")); }
#[test] fn question_too_many() { assert!(!regex_match("^ab?c$", "abbc")); }
#[test] fn dot_star() { assert!(regex_match("a.*c", "aXYZc")); }
#[test] fn anchored_start() { assert!(regex_match("^hello", "hello world")); }
#[test] fn anchored_start_fail() { assert!(!regex_match("^hello", "say hello")); }
#[test] fn anchored_end() { assert!(regex_match("world$", "hello world")); }
#[test] fn anchored_both() { assert!(regex_match("^exact$", "exact")); }
#[test] fn anchored_both_fail() { assert!(!regex_match("^exact$", "not exact")); }
#[test] fn substring_match() { assert!(regex_match("ell", "hello")); }
#[test] fn empty_pattern() { assert!(regex_match("", "anything")); }
"#,
        ),

        // ── 4.10: Graph Coloring ─────────────────────────────────────
        problem(
            "opus-graph-color",
            "tier4",
            "Given an undirected graph (adjacency list) and k colors, find a valid k-coloring \
             where no two adjacent vertices share a color. Return Some(coloring) where \
             coloring[i] is the color (0..k) of vertex i, or None if impossible. \
             Prefer lexicographically smallest coloring.",
            r#"pub fn color_graph(adj: &[Vec<usize>], k: usize) -> Option<Vec<usize>> {
    todo!()
}"#,
            r#"use opus_graph_color::*;

fn is_valid(adj: &[Vec<usize>], coloring: &[usize], k: usize) -> bool {
    for (v, neighbors) in adj.iter().enumerate() {
        if coloring[v] >= k { return false; }
        for &u in neighbors {
            if coloring[v] == coloring[u] { return false; }
        }
    }
    true
}

#[test]
fn triangle_3_colors() {
    let adj = vec![vec![1, 2], vec![0, 2], vec![0, 1]];
    let result = color_graph(&adj, 3).unwrap();
    assert!(is_valid(&adj, &result, 3));
}

#[test]
fn triangle_2_colors() {
    let adj = vec![vec![1, 2], vec![0, 2], vec![0, 1]];
    assert!(color_graph(&adj, 2).is_none());
}

#[test]
fn bipartite() {
    // K2,2: vertices 0,1 connected to 2,3
    let adj = vec![vec![2, 3], vec![2, 3], vec![0, 1], vec![0, 1]];
    let result = color_graph(&adj, 2).unwrap();
    assert!(is_valid(&adj, &result, 2));
}

#[test]
fn single_vertex() {
    let adj = vec![vec![]];
    let result = color_graph(&adj, 1).unwrap();
    assert_eq!(result, vec![0]);
}

#[test]
fn empty_graph() {
    let adj: Vec<Vec<usize>> = vec![vec![], vec![], vec![]];
    let result = color_graph(&adj, 1).unwrap();
    assert_eq!(result, vec![0, 0, 0]); // all same color
}

#[test]
fn petersen_3_colors() {
    // Petersen graph: chromatic number is 3
    let adj = vec![
        vec![1, 4, 5],    // 0
        vec![0, 2, 6],    // 1
        vec![1, 3, 7],    // 2
        vec![2, 4, 8],    // 3
        vec![3, 0, 9],    // 4
        vec![0, 7, 8],    // 5
        vec![1, 8, 9],    // 6
        vec![2, 9, 5],    // 7
        vec![3, 5, 6],    // 8
        vec![4, 6, 7],    // 9
    ];
    let result = color_graph(&adj, 3).unwrap();
    assert!(is_valid(&adj, &result, 3));
}"#,
        ),
    ]
}
