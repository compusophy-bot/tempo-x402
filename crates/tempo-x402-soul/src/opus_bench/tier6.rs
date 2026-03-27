use super::problem;
use crate::benchmark::ExercismProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 6: BRUTAL — Multi-step algorithms where precision is everything
// Flash Lite got 100% on tiers 1-5 with retries. These should break it.
// Each problem requires exact algorithmic implementation — partial solutions fail.
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier6_brutal() -> Vec<ExercismProblem> {
    vec![
        // ── 6.1: Big Integer Division ──────────────────────────────────
        problem(
            "opus-bigint-div",
            "tier6",
            "Implement big integer division for arbitrary-precision decimal strings. \
             `div(a: &str, b: &str) -> (String, String)` returns (quotient, remainder). \
             Both inputs can be 100+ digit decimal strings. Handle negative numbers: \
             division truncates toward zero (like Rust integer division). \
             The remainder has the same sign as the dividend. \
             Panic if b is \"0\". No leading zeros in output (except \"0\" itself).",
            r#"/// Returns (quotient, remainder) of a / b for arbitrary-precision decimal strings.
/// Panics if b is "0".
pub fn div(a: &str, b: &str) -> (String, String) {
    todo!()
}"#,
            r#"use opus_bigint_div::*;

#[test] fn simple_exact() { assert_eq!(div("100", "10"), ("10".into(), "0".into())); }
#[test] fn simple_remainder() { assert_eq!(div("7", "3"), ("2".into(), "1".into())); }
#[test] fn dividend_smaller() { assert_eq!(div("3", "7"), ("0".into(), "3".into())); }
#[test] fn one_by_one() { assert_eq!(div("1", "1"), ("1".into(), "0".into())); }
#[test] fn large_exact() {
    assert_eq!(
        div("123456789012345678901234567890", "987654321"),
        ("124999998860937500".into(), "124999890".into())
    );
}
#[test] fn very_large_dividend() {
    // 10^50 / 7
    let a = "100000000000000000000000000000000000000000000000000";
    let (q, r) = div(a, "7");
    assert_eq!(r, "2");
    // Verify: q * 7 + 2 == a (we check length and first/last digits)
    assert_eq!(q, "14285714285714285714285714285714285714285714285714");
}
#[test] fn negative_dividend() {
    assert_eq!(div("-7", "3"), ("-2".into(), "-1".into()));
}
#[test] fn negative_divisor() {
    assert_eq!(div("7", "-3"), ("-2".into(), "1".into()));
}
#[test] fn both_negative() {
    assert_eq!(div("-7", "-3"), ("2".into(), "-1".into()));
}
#[test] fn large_negative() {
    assert_eq!(
        div("-100000000000000000000", "3"),
        ("-33333333333333333333".into(), "-1".into())
    );
}
#[test] fn divisor_larger_negative() {
    assert_eq!(div("-3", "7"), ("0".into(), "-3".into()));
}
#[test] fn no_leading_zeros_in_quotient() {
    let (q, _) = div("1000000", "999999");
    assert_eq!(q, "1");
}
#[test] #[should_panic] fn divide_by_zero() { div("123", "0"); }
#[test] fn zero_dividend() { assert_eq!(div("0", "12345"), ("0".into(), "0".into())); }
#[test] fn power_of_ten() {
    assert_eq!(
        div("99999999999999999999999999999999999999999999999999", "99999999999999999999999999999999999999999999999999"),
        ("1".into(), "0".into())
    );
}
"#,
        ),

        // ── 6.2: Regex Engine with Capturing Groups ────────────────────
        problem(
            "opus-regex-engine",
            "tier6",
            "Implement a regex engine with capturing groups. \
             `regex_match(pattern: &str, text: &str) -> Option<Vec<String>>` \
             Returns None if no match. If matched, result[0] is the full match, \
             result[1..] are capture groups (empty string if group didn't participate). \
             The match must cover the ENTIRE text (anchored). \
             Supported syntax: `.` (any char), `*` (zero or more, greedy), `+` (one or more, greedy), \
             `?` (zero or one), `()` (capture group), `|` (alternation, lowest precedence), \
             `\\d` (digit), `\\w` (word char: [a-zA-Z0-9_]). \
             Backslash escapes: `\\.`, `\\*`, `\\+`, `\\?`, `\\(`, `\\)`, `\\|`, `\\\\`.",
            r#"/// Match pattern against entire text, returning captures if matched.
pub fn regex_match(pattern: &str, text: &str) -> Option<Vec<String>> {
    todo!()
}"#,
            r#"use opus_regex_engine::*;

#[test] fn literal_match() {
    let r = regex_match("hello", "hello");
    assert_eq!(r, Some(vec!["hello".into()]));
}
#[test] fn literal_no_match() {
    assert_eq!(regex_match("hello", "world"), None);
}
#[test] fn dot_matches_any() {
    let r = regex_match("h.llo", "hello").unwrap();
    assert_eq!(r[0], "hello");
}
#[test] fn star_zero() {
    let r = regex_match("ab*c", "ac").unwrap();
    assert_eq!(r[0], "ac");
}
#[test] fn star_many() {
    let r = regex_match("ab*c", "abbbc").unwrap();
    assert_eq!(r[0], "abbbc");
}
#[test] fn plus_one() {
    let r = regex_match("ab+c", "abc").unwrap();
    assert_eq!(r[0], "abc");
}
#[test] fn plus_zero_fails() {
    assert_eq!(regex_match("ab+c", "ac"), None);
}
#[test] fn question_mark() {
    assert!(regex_match("ab?c", "ac").is_some());
    assert!(regex_match("ab?c", "abc").is_some());
    assert!(regex_match("ab?c", "abbc").is_none());
}
#[test] fn capture_group_simple() {
    let r = regex_match("(abc)", "abc").unwrap();
    assert_eq!(r, vec!["abc".into(), "abc".into()]);
}
#[test] fn capture_group_nested() {
    let r = regex_match("((a)(b))", "ab").unwrap();
    assert_eq!(r.len(), 4);
    assert_eq!(r[0], "ab");
    assert_eq!(r[1], "ab");  // outer group
    assert_eq!(r[2], "a");   // first inner
    assert_eq!(r[3], "b");   // second inner
}
#[test] fn alternation() {
    assert!(regex_match("cat|dog", "cat").is_some());
    assert!(regex_match("cat|dog", "dog").is_some());
    assert!(regex_match("cat|dog", "bat").is_none());
}
#[test] fn alternation_in_group() {
    let r = regex_match("(cat|dog) food", "cat food").unwrap();
    assert_eq!(r[0], "cat food");
    assert_eq!(r[1], "cat");
}
#[test] fn digit_class() {
    let r = regex_match("\\d+", "42").unwrap();
    assert_eq!(r[0], "42");
    assert!(regex_match("\\d+", "abc").is_none());
}
#[test] fn word_class() {
    let r = regex_match("\\w+", "hello_42").unwrap();
    assert_eq!(r[0], "hello_42");
}
#[test] fn complex_pattern() {
    let r = regex_match("(\\d+)\\.(\\d+)", "3.14").unwrap();
    assert_eq!(r[0], "3.14");
    assert_eq!(r[1], "3");
    assert_eq!(r[2], "14");
}
#[test] fn escaped_dot() {
    assert!(regex_match("a\\.b", "a.b").is_some());
    assert!(regex_match("a\\.b", "axb").is_none());
}
#[test] fn must_match_entire_text() {
    assert!(regex_match("abc", "xabcx").is_none());
    assert!(regex_match("abc", "abc").is_some());
}
"#,
        ),

        // ── 6.3: B-Tree Order 3 ───────────────────────────────────────
        problem(
            "opus-b-tree",
            "tier6",
            "Implement a B-tree of order 3 (max 2 keys per node, min 1 key in non-root internal nodes). \
             Operations: `insert(key: i32)`, `search(key: i32) -> bool`, `delete(key: i32) -> bool`, \
             `in_order() -> Vec<i32>`. Delete must handle all cases: leaf deletion, internal node deletion \
             with in-order predecessor/successor replacement, underflow with redistribution from sibling, \
             and underflow with merge. The tree must maintain B-tree invariants after every operation.",
            r#"pub struct BTree {
    // your fields here
}

impl BTree {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, key: i32) { todo!() }
    pub fn search(&self, key: i32) -> bool { todo!() }
    pub fn delete(&mut self, key: i32) -> bool { todo!() }
    pub fn in_order(&self) -> Vec<i32> { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
}"#,
            r#"use opus_b_tree::*;

#[test] fn empty_tree() {
    let t = BTree::new();
    assert!(t.is_empty());
    assert_eq!(t.in_order(), vec![]);
    assert!(!t.search(1));
}

#[test] fn insert_single() {
    let mut t = BTree::new();
    t.insert(5);
    assert!(t.search(5));
    assert_eq!(t.in_order(), vec![5]);
}

#[test] fn insert_causes_split() {
    let mut t = BTree::new();
    // Order 3: max 2 keys per node, inserting 3 forces a split
    t.insert(1);
    t.insert(2);
    t.insert(3);
    assert_eq!(t.in_order(), vec![1, 2, 3]);
    assert!(t.search(1));
    assert!(t.search(2));
    assert!(t.search(3));
}

#[test] fn insert_many_sorted() {
    let mut t = BTree::new();
    for i in 1..=10 {
        t.insert(i);
    }
    assert_eq!(t.in_order(), (1..=10).collect::<Vec<_>>());
}

#[test] fn insert_many_reverse() {
    let mut t = BTree::new();
    for i in (1..=10).rev() {
        t.insert(i);
    }
    assert_eq!(t.in_order(), (1..=10).collect::<Vec<_>>());
}

#[test] fn insert_many_random_order() {
    let mut t = BTree::new();
    for &i in &[5, 3, 7, 1, 9, 2, 8, 4, 6, 10] {
        t.insert(i);
    }
    assert_eq!(t.in_order(), (1..=10).collect::<Vec<_>>());
}

#[test] fn delete_from_leaf() {
    let mut t = BTree::new();
    for i in 1..=5 { t.insert(i); }
    assert!(t.delete(1));
    assert!(!t.search(1));
    assert_eq!(t.in_order(), vec![2, 3, 4, 5]);
}

#[test] fn delete_nonexistent() {
    let mut t = BTree::new();
    t.insert(1);
    assert!(!t.delete(99));
    assert_eq!(t.in_order(), vec![1]);
}

#[test] fn delete_causes_merge() {
    let mut t = BTree::new();
    for i in 1..=7 { t.insert(i); }
    // Delete enough to cause underflow and merge
    for i in 1..=5 {
        assert!(t.delete(i));
    }
    assert_eq!(t.in_order(), vec![6, 7]);
}

#[test] fn delete_all() {
    let mut t = BTree::new();
    for i in 1..=10 { t.insert(i); }
    for i in 1..=10 {
        assert!(t.delete(i));
    }
    assert!(t.is_empty());
    assert_eq!(t.in_order(), vec![]);
}

#[test] fn delete_internal_node() {
    let mut t = BTree::new();
    for &i in &[5, 3, 7, 1, 9, 2, 8, 4, 6, 10] { t.insert(i); }
    // Delete keys likely in internal nodes
    assert!(t.delete(5));
    assert!(!t.search(5));
    let remaining = t.in_order();
    assert_eq!(remaining, vec![1, 2, 3, 4, 6, 7, 8, 9, 10]);
}

#[test] fn delete_root_collapse() {
    let mut t = BTree::new();
    t.insert(1);
    t.insert(2);
    t.insert(3);
    assert!(t.delete(2));
    assert!(t.delete(1));
    assert_eq!(t.in_order(), vec![3]);
    assert!(t.delete(3));
    assert!(t.is_empty());
}

#[test] fn stress_insert_delete() {
    let mut t = BTree::new();
    // Insert 20 items
    let items: Vec<i32> = vec![15, 3, 18, 7, 12, 1, 9, 20, 5, 14, 2, 17, 11, 6, 19, 4, 16, 8, 13, 10];
    for &i in &items { t.insert(i); }
    assert_eq!(t.in_order(), (1..=20).collect::<Vec<_>>());
    // Delete odd numbers
    for i in (1..=20).filter(|x| x % 2 == 1) {
        assert!(t.delete(i));
    }
    assert_eq!(t.in_order(), vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
    // Delete remaining
    for i in (2..=20).step_by(2) {
        assert!(t.delete(i));
    }
    assert!(t.is_empty());
}
"#,
        ),

        // ── 6.4: LZ77 Compression ─────────────────────────────────────
        problem(
            "opus-lz77-compress",
            "tier6",
            "Implement LZ77 compression and decompression. \
             `compress(data: &[u8]) -> Vec<Token>` where `Token` is either `Literal(u8)` or \
             `Match { offset: usize, length: usize }`. Use a sliding window of 4096 bytes. \
             Find the longest match (minimum length 3) looking back in the window. \
             `decompress(tokens: &[Token]) -> Vec<u8>` reconstructs the original data. \
             Roundtrip must be exact: `decompress(&compress(data)) == data` for all inputs. \
             Token and its variants must derive Debug, Clone, PartialEq.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Literal(u8),
    Match { offset: usize, length: usize },
}

pub fn compress(data: &[u8]) -> Vec<Token> {
    todo!()
}

pub fn decompress(tokens: &[Token]) -> Vec<u8> {
    todo!()
}"#,
            r#"use opus_lz77_compress::*;

#[test] fn empty_roundtrip() {
    let data: &[u8] = b"";
    assert_eq!(decompress(&compress(data)), data);
}

#[test] fn single_byte() {
    let data = b"a";
    let tokens = compress(data);
    assert_eq!(tokens, vec![Token::Literal(b'a')]);
    assert_eq!(decompress(&tokens), data);
}

#[test] fn no_repeats() {
    let data = b"abcdefgh";
    let tokens = compress(data);
    // All literals since nothing repeats with length >= 3
    assert_eq!(decompress(&tokens), data);
    assert!(tokens.iter().all(|t| matches!(t, Token::Literal(_))));
}

#[test] fn simple_repeat() {
    let data = b"abcabc";
    let tokens = compress(data);
    assert_eq!(decompress(&tokens), data);
    // Should have some Match tokens
    assert!(tokens.iter().any(|t| matches!(t, Token::Match { .. })));
}

#[test] fn long_repeat() {
    let data = b"abcdefghijabcdefghij";
    let tokens = compress(data);
    assert_eq!(decompress(&tokens), data);
    // Compressed should be shorter in token count
    assert!(tokens.len() < data.len());
}

#[test] fn all_same_byte() {
    let data = vec![b'x'; 100];
    let tokens = compress(&data);
    assert_eq!(decompress(&tokens), data);
    assert!(tokens.len() < 50); // significant compression
}

#[test] fn overlapping_match() {
    // "aaaaaa" — match can overlap: offset=1, length=5
    let data = b"aaaaaa";
    let tokens = compress(data);
    assert_eq!(decompress(&tokens), data);
}

#[test] fn binary_data_roundtrip() {
    let data: Vec<u8> = (0..=255).collect();
    assert_eq!(decompress(&compress(&data)), data);
}

#[test] fn repetitive_pattern_compresses() {
    let pattern = b"the quick brown fox ";
    let mut data = Vec::new();
    for _ in 0..10 {
        data.extend_from_slice(pattern);
    }
    let tokens = compress(&data);
    assert_eq!(decompress(&tokens), data);
    // Tokens should be significantly fewer than bytes
    assert!(tokens.len() < data.len() / 2);
}

#[test] fn mixed_literal_and_match() {
    let data = b"xyzxyzQQQxyzxyz";
    let tokens = compress(data);
    assert_eq!(decompress(&tokens), data);
}

#[test] fn minimum_match_length() {
    // "ab" repeated — but match length 2 is below threshold, should be literals
    let data = b"ababab";
    let tokens = compress(data);
    assert_eq!(decompress(&tokens), data);
}

#[test] fn large_data_roundtrip() {
    let mut data = Vec::new();
    for i in 0u16..1000 {
        data.extend_from_slice(&i.to_le_bytes());
    }
    // Append some repetitive section
    data.extend_from_slice(&data[0..200].to_vec());
    assert_eq!(decompress(&compress(&data)), data);
}
"#,
        ),

        // ── 6.5: DPLL SAT Solver ──────────────────────────────────────
        problem(
            "opus-sat-solver",
            "tier6",
            "Implement a DPLL SAT solver for CNF (Conjunctive Normal Form) formulas. \
             `solve(num_vars: usize, clauses: &[Vec<i32>]) -> Option<Vec<bool>>` \
             Variables are 1-indexed: positive literal `i` means var i is true, \
             negative literal `-i` means var i is false. \
             Return Some(assignment) where assignment[0] is var 1, assignment[1] is var 2, etc., \
             or None if unsatisfiable. \
             Must implement: unit propagation, pure literal elimination, and DPLL branching.",
            r#"/// Solve a CNF SAT problem. Variables are 1-indexed in clauses.
/// Returns assignment for vars 1..=num_vars (0-indexed in result Vec).
pub fn solve(num_vars: usize, clauses: &[Vec<i32>]) -> Option<Vec<bool>> {
    todo!()
}"#,
            r#"use opus_sat_solver::*;

fn verify(num_vars: usize, clauses: &[Vec<i32>], assignment: &[bool]) -> bool {
    assert_eq!(assignment.len(), num_vars);
    clauses.iter().all(|clause| {
        clause.iter().any(|&lit| {
            let var = lit.unsigned_abs() as usize - 1;
            if lit > 0 { assignment[var] } else { !assignment[var] }
        })
    })
}

#[test] fn trivial_sat() {
    // (x1)
    let result = solve(1, &[vec![1]]).unwrap();
    assert!(result[0]);
}

#[test] fn trivial_unsat() {
    // (x1) AND (NOT x1)
    assert!(solve(1, &[vec![1], vec![-1]]).is_none());
}

#[test] fn two_vars_sat() {
    // (x1 OR x2) AND (NOT x1 OR x2)
    let clauses = vec![vec![1, 2], vec![-1, 2]];
    let result = solve(2, &clauses).unwrap();
    assert!(verify(2, &clauses, &result));
}

#[test] fn unit_propagation() {
    // (x1) AND (x1 OR x2) AND (NOT x1 OR x3)
    let clauses = vec![vec![1], vec![1, 2], vec![-1, 3]];
    let result = solve(3, &clauses).unwrap();
    assert!(verify(3, &clauses, &result));
    assert!(result[0]); // x1 must be true
}

#[test] fn pure_literal() {
    // x1 only appears positive, x2 only negative
    // (x1 OR x2) AND (x1 OR NOT x2)
    let clauses = vec![vec![1, 2], vec![1, -2]];
    let result = solve(2, &clauses).unwrap();
    assert!(verify(2, &clauses, &result));
}

#[test] fn empty_clause_unsat() {
    // Empty clause is always false
    assert!(solve(1, &[vec![]]).is_none());
}

#[test] fn no_clauses_sat() {
    // No constraints — anything works
    assert!(solve(3, &[]).is_some());
}

#[test] fn pigeonhole_3_2_unsat() {
    // 3 pigeons, 2 holes — classic UNSAT
    // p_ij = pigeon i in hole j: var (i-1)*2 + j, for i in 1..=3, j in 1..=2
    // Each pigeon in at least one hole
    // No two pigeons in same hole
    let clauses = vec![
        vec![1, 2],       // pigeon 1 in hole 1 or 2
        vec![3, 4],       // pigeon 2 in hole 1 or 2
        vec![5, 6],       // pigeon 3 in hole 1 or 2
        vec![-1, -3],     // not (p1h1 and p2h1)
        vec![-1, -5],     // not (p1h1 and p3h1)
        vec![-3, -5],     // not (p2h1 and p3h1)
        vec![-2, -4],     // not (p1h2 and p2h2)
        vec![-2, -6],     // not (p1h2 and p3h2)
        vec![-4, -6],     // not (p2h2 and p3h2)
    ];
    assert!(solve(6, &clauses).is_none());
}

#[test] fn medium_sat_instance() {
    // 10 variables, satisfiable chain: x1 => x2 => x3 => ... => x10
    let mut clauses = Vec::new();
    clauses.push(vec![1]); // x1 must be true
    for i in 1..10 {
        clauses.push(vec![-(i as i32), (i as i32 + 1)]); // xi => x(i+1)
    }
    let result = solve(10, &clauses).unwrap();
    assert!(verify(10, &clauses, &result));
    // All must be true due to chain
    for i in 0..10 {
        assert!(result[i], "var {} should be true", i + 1);
    }
}

#[test] fn graph_coloring_3_sat() {
    // 3-coloring of a triangle (K3) — satisfiable
    // 3 nodes, 3 colors: var (node*3 + color + 1), node in 0..3, color in 0..3
    // Each node has exactly one color, adjacent nodes differ
    let mut clauses = Vec::new();
    // Each node gets at least one color
    for n in 0..3 {
        clauses.push(vec![n*3+1, n*3+2, n*3+3]);
    }
    // Each node gets at most one color
    for n in 0..3 {
        for c1 in 0..3 {
            for c2 in (c1+1)..3 {
                clauses.push(vec![-(n*3+c1+1), -(n*3+c2+1)]);
            }
        }
    }
    // Adjacent nodes have different colors (complete graph)
    for n1 in 0..3 {
        for n2 in (n1+1)..3 {
            for c in 0..3 {
                clauses.push(vec![-(n1*3+c+1), -(n2*3+c+1)]);
            }
        }
    }
    let result = solve(9, &clauses).unwrap();
    assert!(verify(9, &clauses, &result));
}

#[test] fn larger_unsat() {
    // At most one of x1..x4 true, but at least two must be true
    let mut clauses = Vec::new();
    // At least x1 or x2 true
    clauses.push(vec![1, 2]);
    // At least x3 or x4 true
    clauses.push(vec![3, 4]);
    // At most one true overall (pairwise exclusion)
    for i in 1..=4 {
        for j in (i+1)..=4 {
            clauses.push(vec![-(i as i32), -(j as i32)]);
        }
    }
    assert!(solve(4, &clauses).is_none());
}
"#,
        ),

        // ── 6.6: Myers Diff Algorithm ──────────────────────────────────
        problem(
            "opus-diff-algorithm",
            "tier6",
            "Implement the Myers diff algorithm for computing the minimum edit script \
             between two sequences of lines. \
             `diff(old: &[&str], new: &[&str]) -> Vec<DiffOp>` where \
             `DiffOp` is `Keep(String)`, `Insert(String)`, or `Delete(String)`. \
             The result must represent a MINIMUM edit distance (fewest Insert + Delete ops). \
             `apply(old: &[&str], ops: &[DiffOp]) -> Vec<String>` applies the diff to reconstruct new. \
             DiffOp must derive Debug, Clone, PartialEq.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum DiffOp {
    Keep(String),
    Insert(String),
    Delete(String),
}

/// Compute minimum edit script from old to new.
pub fn diff<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffOp> {
    todo!()
}

/// Apply diff ops to old to produce new.
pub fn apply(old: &[&str], ops: &[DiffOp]) -> Vec<String> {
    todo!()
}"#,
            r#"use opus_diff_algorithm::*;

fn edit_distance(ops: &[DiffOp]) -> usize {
    ops.iter().filter(|op| !matches!(op, DiffOp::Keep(_))).count()
}

#[test] fn identical() {
    let old = vec!["a", "b", "c"];
    let ops = diff(&old, &old);
    assert!(ops.iter().all(|op| matches!(op, DiffOp::Keep(_))));
    assert_eq!(apply(&old, &ops), old.iter().map(|s| s.to_string()).collect::<Vec<_>>());
}

#[test] fn all_deleted() {
    let old = vec!["a", "b", "c"];
    let new: Vec<&str> = vec![];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 3);
    assert_eq!(apply(&old, &ops), Vec::<String>::new());
}

#[test] fn all_inserted() {
    let old: Vec<&str> = vec![];
    let new = vec!["a", "b", "c"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 3);
    assert_eq!(apply(&old, &ops), vec!["a", "b", "c"]);
}

#[test] fn simple_insert() {
    let old = vec!["a", "c"];
    let new = vec!["a", "b", "c"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 1); // just one insert
    assert_eq!(apply(&old, &ops), vec!["a", "b", "c"]);
}

#[test] fn simple_delete() {
    let old = vec!["a", "b", "c"];
    let new = vec!["a", "c"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 1);
    assert_eq!(apply(&old, &ops), vec!["a", "c"]);
}

#[test] fn replace_middle() {
    let old = vec!["a", "b", "c"];
    let new = vec!["a", "x", "c"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 2); // delete b + insert x
    assert_eq!(apply(&old, &ops), vec!["a", "x", "c"]);
}

#[test] fn complex_diff() {
    let old = vec!["a", "b", "c", "d", "e"];
    let new = vec!["a", "x", "c", "y", "e"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 4); // delete b,d + insert x,y
    assert_eq!(apply(&old, &ops), vec!["a", "x", "c", "y", "e"]);
}

#[test] fn minimum_edit_distance() {
    // This is the key test: must find MINIMUM edits, not just any correct diff
    let old = vec!["a", "b", "c", "d"];
    let new = vec!["a", "c", "d", "b"];
    let ops = diff(&old, &new);
    // Minimum is 2: delete b from pos 1, insert b at end
    assert_eq!(edit_distance(&ops), 2);
    assert_eq!(apply(&old, &ops), vec!["a", "c", "d", "b"]);
}

#[test] fn completely_different() {
    let old = vec!["a", "b", "c"];
    let new = vec!["x", "y", "z"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 6);
    assert_eq!(apply(&old, &ops), vec!["x", "y", "z"]);
}

#[test] fn longer_sequence() {
    let old = vec!["the", "quick", "brown", "fox", "jumps"];
    let new = vec!["the", "slow", "brown", "fox", "crawls"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 4); // delete quick + jumps, insert slow + crawls
    assert_eq!(apply(&old, &ops), vec!["the", "slow", "brown", "fox", "crawls"]);
}

#[test] fn empty_to_empty() {
    let old: Vec<&str> = vec![];
    let new: Vec<&str> = vec![];
    let ops = diff(&old, &new);
    assert!(ops.is_empty());
}

#[test] fn single_line_change() {
    let old = vec!["hello"];
    let new = vec!["world"];
    let ops = diff(&old, &new);
    assert_eq!(edit_distance(&ops), 2);
    assert_eq!(apply(&old, &ops), vec!["world"]);
}
"#,
        ),

        // ── 6.7: Huffman Coding ────────────────────────────────────────
        problem(
            "opus-huffman",
            "tier6",
            "Implement Huffman coding. \
             `encode(data: &[u8]) -> (Vec<bool>, HuffmanTree)` compresses data to bits + tree. \
             `decode(bits: &[bool], tree: &HuffmanTree) -> Vec<u8>` decompresses. \
             The tree must be canonical: when building, if two nodes have equal frequency, \
             the one with the smaller minimum symbol value goes left. \
             Left = 0, Right = 1. Single-byte input uses code `false` (one bit). \
             HuffmanTree must derive Debug and Clone.",
            r#"#[derive(Debug, Clone)]
pub enum HuffmanTree {
    Leaf(u8),
    Node(Box<HuffmanTree>, Box<HuffmanTree>),
}

pub fn encode(data: &[u8]) -> (Vec<bool>, HuffmanTree) {
    todo!()
}

pub fn decode(bits: &[bool], tree: &HuffmanTree) -> Vec<u8> {
    todo!()
}"#,
            r#"use opus_huffman::*;

#[test] fn empty_encode() {
    let (bits, _) = encode(b"");
    assert!(bits.is_empty());
}

#[test] fn single_byte() {
    let data = b"a";
    let (bits, tree) = encode(data);
    assert_eq!(bits.len(), 1);
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn single_byte_repeated() {
    let data = b"aaaa";
    let (bits, tree) = encode(data);
    assert_eq!(bits.len(), 4); // 1 bit per symbol
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn two_symbols() {
    let data = b"ab";
    let (bits, tree) = encode(data);
    assert_eq!(bits.len(), 2); // 1 bit each
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn roundtrip_hello() {
    let data = b"hello world";
    let (bits, tree) = encode(data);
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn roundtrip_all_bytes() {
    let data: Vec<u8> = (0..=255).collect();
    let (bits, tree) = encode(&data);
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn compression_ratio() {
    // Highly skewed distribution should compress well
    let mut data = vec![b'a'; 100];
    data.push(b'b');
    let (bits, tree) = encode(&data);
    assert_eq!(decode(&bits, &tree), data);
    // 'a' should get short code (1 bit), total bits < 8 * 101
    assert!(bits.len() < data.len() * 8);
    // Actually 'a' should be ~1 bit, 'b' ~1 bit, so ~102 bits total
    assert!(bits.len() < 200);
}

#[test] fn canonical_tree_deterministic() {
    // Same input should always produce same encoding
    let data = b"abracadabra";
    let (bits1, _) = encode(data);
    let (bits2, _) = encode(data);
    assert_eq!(bits1, bits2);
}

#[test] fn roundtrip_binary() {
    let data: Vec<u8> = (0..200).map(|i| (i * 7 + 13) as u8).collect();
    let (bits, tree) = encode(&data);
    assert_eq!(decode(&bits, &tree), data);
}

#[test] fn roundtrip_long_text() {
    let text = b"the quick brown fox jumps over the lazy dog ";
    let mut data = Vec::new();
    for _ in 0..20 { data.extend_from_slice(text); }
    let (bits, tree) = encode(&data);
    assert_eq!(decode(&bits, &tree), data);
    // Should compress repetitive text
    assert!(bits.len() < data.len() * 8);
}

#[test] fn three_symbols_canonical_order() {
    // With equal frequencies, smaller symbol value should go left
    let data = b"aabbcc";
    let (bits, tree) = encode(data);
    let decoded = decode(&bits, &tree);
    assert_eq!(decoded, data);
}

#[test] fn prefix_free() {
    // Verify no code is a prefix of another by checking roundtrip with varied input
    let data = b"abcabcabcdefdefghijklmnop";
    let (bits, tree) = encode(data);
    assert_eq!(decode(&bits, &tree), data);
}
"#,
        ),

        // ── 6.8: Weighted Interval Scheduling ─────────────────────────
        problem(
            "opus-interval-scheduling",
            "tier6",
            "Solve the weighted interval scheduling problem optimally. \
             Given intervals with (start, end, weight), find the maximum total weight \
             subset of non-overlapping intervals. Two intervals overlap if one starts \
             before the other ends (strictly: intervals [s1,e1) and [s2,e2) overlap iff s1 < e2 && s2 < e1). \
             `max_weight_schedule(intervals: &[(u64, u64, u64)]) -> (u64, Vec<usize>)` \
             returns (total_weight, indices_of_selected_intervals). Indices are 0-based \
             and must be sorted ascending. Must be optimal — greedy by weight alone fails.",
            r#"/// Returns (max_weight, selected_indices) for non-overlapping intervals.
/// Each interval is (start, end, weight). Intervals are half-open [start, end).
pub fn max_weight_schedule(intervals: &[(u64, u64, u64)]) -> (u64, Vec<usize>) {
    todo!()
}"#,
            r#"use opus_interval_scheduling::*;

fn no_overlaps(intervals: &[(u64, u64, u64)], selected: &[usize]) -> bool {
    for i in 0..selected.len() {
        for j in (i+1)..selected.len() {
            let (s1, e1, _) = intervals[selected[i]];
            let (s2, e2, _) = intervals[selected[j]];
            if s1 < e2 && s2 < e1 { return false; }
        }
    }
    true
}

#[test] fn empty() {
    let (w, sel) = max_weight_schedule(&[]);
    assert_eq!(w, 0);
    assert!(sel.is_empty());
}

#[test] fn single_interval() {
    let (w, sel) = max_weight_schedule(&[(0, 10, 5)]);
    assert_eq!(w, 5);
    assert_eq!(sel, vec![0]);
}

#[test] fn non_overlapping() {
    let intervals = vec![(0, 5, 3), (5, 10, 4), (10, 15, 2)];
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 9);
    assert_eq!(sel.len(), 3);
}

#[test] fn fully_overlapping_pick_heaviest() {
    let intervals = vec![(0, 10, 3), (0, 10, 7), (0, 10, 5)];
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 7);
    assert_eq!(sel.len(), 1);
    assert_eq!(intervals[sel[0]].2, 7);
}

#[test] fn greedy_by_weight_fails() {
    // Greedy picking heaviest first gives 10, but optimal is 3+4+5=12
    let intervals = vec![
        (0, 100, 10),   // big heavy interval
        (0, 30, 3),     // three smaller ones that don't overlap each other
        (30, 60, 4),
        (60, 100, 5),
    ];
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 12);
    assert!(no_overlaps(&intervals, &sel));
}

#[test] fn greedy_by_finish_fails() {
    // Greedy by earliest finish gives 1+1=2, but optimal is 100
    let intervals = vec![
        (0, 1, 1),
        (1, 2, 1),
        (0, 2, 100),
    ];
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 100);
}

#[test] fn complex_optimal() {
    let intervals = vec![
        (1, 4, 5),
        (3, 5, 1),
        (0, 6, 8),
        (4, 7, 4),
        (3, 8, 6),
        (5, 9, 2),
        (6, 10, 4),
        (8, 11, 2),
    ];
    let (w, sel) = max_weight_schedule(&intervals);
    // Optimal: [0,6)=8 + [6,10)=4 + ... = many possibilities
    // Best is (1,4)=5 + (4,7)=4 + (8,11)=2 = 11 or (0,6)=8 + (6,10)=4 + ... = 12
    assert!(w >= 12);
    assert!(no_overlaps(&intervals, &sel));
    // Verify weight sum
    let sum: u64 = sel.iter().map(|&i| intervals[i].2).sum();
    assert_eq!(sum, w);
}

#[test] fn adjacent_intervals() {
    // [0,5) and [5,10) do NOT overlap
    let intervals = vec![(0, 5, 10), (5, 10, 10)];
    let (w, _) = max_weight_schedule(&intervals);
    assert_eq!(w, 20);
}

#[test] fn barely_overlapping() {
    // [0,5) and [4,10) DO overlap
    let intervals = vec![(0, 5, 10), (4, 10, 10)];
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 10);
    assert_eq!(sel.len(), 1);
}

#[test] fn indices_sorted() {
    let intervals = vec![(0, 5, 3), (5, 10, 4), (10, 15, 2)];
    let (_, sel) = max_weight_schedule(&intervals);
    for i in 1..sel.len() {
        assert!(sel[i] > sel[i-1], "indices must be sorted ascending");
    }
}

#[test] fn many_small_vs_one_big() {
    // 10 tiny intervals worth 2 each vs one big worth 15
    let mut intervals: Vec<(u64, u64, u64)> = (0..10).map(|i| (i*10, i*10+10, 2)).collect();
    intervals.push((0, 100, 15));
    let (w, sel) = max_weight_schedule(&intervals);
    assert_eq!(w, 20); // 10 * 2 = 20 > 15
    assert!(no_overlaps(&intervals, &sel));
}
"#,
        ),

        // ── 6.9: JSON Parser from Scratch ──────────────────────────────
        problem(
            "opus-parser-combinator",
            "tier6",
            "Build a complete JSON parser from scratch. \
             `parse_json(input: &str) -> Result<JsonValue, String>` \
             JsonValue enum: Null, Bool(bool), Number(f64), Str(String), Array(Vec<JsonValue>), \
             Object(Vec<(String, JsonValue)>). \
             Must handle: nested objects/arrays, string escapes (\\n, \\t, \\r, \\\\, \\\", \\/), \
             unicode escapes (\\uXXXX), numbers (integer, float, negative, exponent notation), \
             whitespace between tokens. Must REJECT invalid JSON. \
             Object keys preserve insertion order. \
             JsonValue must derive Debug, Clone, PartialEq.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    todo!()
}"#,
            r#"use opus_parser_combinator::*;

#[test] fn parse_null() {
    assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
}

#[test] fn parse_true() {
    assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
}

#[test] fn parse_false() {
    assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
}

#[test] fn parse_integer() {
    assert_eq!(parse_json("42").unwrap(), JsonValue::Number(42.0));
}

#[test] fn parse_negative() {
    assert_eq!(parse_json("-17").unwrap(), JsonValue::Number(-17.0));
}

#[test] fn parse_float() {
    assert_eq!(parse_json("3.14").unwrap(), JsonValue::Number(3.14));
}

#[test] fn parse_exponent() {
    assert_eq!(parse_json("1e10").unwrap(), JsonValue::Number(1e10));
    assert_eq!(parse_json("2.5E-3").unwrap(), JsonValue::Number(2.5e-3));
}

#[test] fn parse_string() {
    assert_eq!(parse_json("\"hello\"").unwrap(), JsonValue::Str("hello".into()));
}

#[test] fn parse_string_escapes() {
    assert_eq!(
        parse_json("\"a\\nb\\tc\\\\d\\\"e\\/f\"").unwrap(),
        JsonValue::Str("a\nb\tc\\d\"e/f".into())
    );
}

#[test] fn parse_unicode_escape() {
    assert_eq!(
        parse_json("\"\\u0041\\u0042\"").unwrap(),
        JsonValue::Str("AB".into())
    );
}

#[test] fn parse_empty_array() {
    assert_eq!(parse_json("[]").unwrap(), JsonValue::Array(vec![]));
}

#[test] fn parse_array() {
    let v = parse_json("[1, 2, 3]").unwrap();
    assert_eq!(v, JsonValue::Array(vec![
        JsonValue::Number(1.0),
        JsonValue::Number(2.0),
        JsonValue::Number(3.0),
    ]));
}

#[test] fn parse_empty_object() {
    assert_eq!(parse_json("{}").unwrap(), JsonValue::Object(vec![]));
}

#[test] fn parse_object() {
    let v = parse_json("{\"a\": 1, \"b\": true}").unwrap();
    assert_eq!(v, JsonValue::Object(vec![
        ("a".into(), JsonValue::Number(1.0)),
        ("b".into(), JsonValue::Bool(true)),
    ]));
}

#[test] fn parse_nested() {
    let input = "{\"arr\": [1, {\"x\": null}], \"flag\": false}";
    let v = parse_json(input).unwrap();
    match v {
        JsonValue::Object(pairs) => {
            assert_eq!(pairs.len(), 2);
            assert_eq!(pairs[0].0, "arr");
            match &pairs[0].1 {
                JsonValue::Array(arr) => {
                    assert_eq!(arr.len(), 2);
                    assert_eq!(arr[0], JsonValue::Number(1.0));
                    match &arr[1] {
                        JsonValue::Object(inner) => {
                            assert_eq!(inner[0], ("x".into(), JsonValue::Null));
                        }
                        _ => panic!("expected object"),
                    }
                }
                _ => panic!("expected array"),
            }
        }
        _ => panic!("expected object"),
    }
}

#[test] fn parse_whitespace() {
    let v = parse_json("  {  \"a\"  :  [  1  ,  2  ]  }  ").unwrap();
    assert!(matches!(v, JsonValue::Object(_)));
}

#[test] fn reject_trailing_comma() {
    assert!(parse_json("[1, 2,]").is_err());
}

#[test] fn reject_single_quotes() {
    assert!(parse_json("'hello'").is_err());
}

#[test] fn reject_unquoted_key() {
    assert!(parse_json("{key: 1}").is_err());
}

#[test] fn reject_trailing_text() {
    assert!(parse_json("123 abc").is_err());
}

#[test] fn reject_incomplete() {
    assert!(parse_json("{\"a\":").is_err());
    assert!(parse_json("[1, 2").is_err());
}

#[test] fn parse_deeply_nested() {
    let input = "[[[[[[1]]]]]]";
    let v = parse_json(input).unwrap();
    // Unwrap 6 levels
    let mut current = &v;
    for _ in 0..6 {
        match current {
            JsonValue::Array(a) => current = &a[0],
            _ => panic!("expected array"),
        }
    }
    assert_eq!(current, &JsonValue::Number(1.0));
}

#[test] fn parse_zero() {
    assert_eq!(parse_json("0").unwrap(), JsonValue::Number(0.0));
}

#[test] fn reject_leading_zero() {
    assert!(parse_json("01").is_err());
}
"#,
        ),

        // ── 6.10: Raft Leader Election State Machine ───────────────────
        problem(
            "opus-raft-state",
            "tier6",
            "Implement the Raft consensus leader election state machine (no log replication). \
             `RaftNode` has state: Follower, Candidate, or Leader. \
             `new(id: u64, peers: Vec<u64>) -> Self` creates a follower at term 0. \
             `receive_message(msg: Message) -> Vec<Message>` processes a message and returns response messages. \
             Messages: `RequestVote { term, candidate_id }`, `VoteResponse { term, vote_granted }`, \
             `Heartbeat { term, leader_id }`, `HeartbeatResponse { term }`, `Timeout`. \
             Rules: \
             - Timeout as follower/candidate: increment term, become candidate, vote for self, send RequestVote to all peers. \
             - Grant vote only if: candidate term >= current term AND haven't voted in this term (or voted for same candidate). \
             - If receive message with higher term: revert to follower, update term, clear voted_for. \
             - Candidate with majority votes: become leader, send Heartbeat to all. \
             - Leader sends HeartbeatResponse (with its term) to heartbeats from higher terms. \
             - Heartbeat from valid leader (term >= current): reset to follower. \
             `state() -> State`, `current_term() -> u64`, `voted_for() -> Option<u64>`, `id() -> u64`.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    RequestVote { term: u64, candidate_id: u64 },
    VoteResponse { term: u64, vote_granted: bool },
    Heartbeat { term: u64, leader_id: u64 },
    HeartbeatResponse { term: u64 },
    Timeout,
}

pub struct RaftNode {
    // your fields here
}

impl RaftNode {
    pub fn new(id: u64, peers: Vec<u64>) -> Self { todo!() }
    pub fn receive_message(&mut self, msg: Message) -> Vec<Message> { todo!() }
    pub fn state(&self) -> State { todo!() }
    pub fn current_term(&self) -> u64 { todo!() }
    pub fn voted_for(&self) -> Option<u64> { todo!() }
    pub fn id(&self) -> u64 { todo!() }
}"#,
            r#"use opus_raft_state::*;

#[test] fn initial_state() {
    let node = RaftNode::new(1, vec![2, 3, 4, 5]);
    assert_eq!(node.state(), State::Follower);
    assert_eq!(node.current_term(), 0);
    assert_eq!(node.voted_for(), None);
    assert_eq!(node.id(), 1);
}

#[test] fn timeout_starts_election() {
    let mut node = RaftNode::new(1, vec![2, 3, 4, 5]);
    let msgs = node.receive_message(Message::Timeout);
    assert_eq!(node.state(), State::Candidate);
    assert_eq!(node.current_term(), 1);
    assert_eq!(node.voted_for(), Some(1)); // voted for self
    // Should send RequestVote to all 4 peers
    assert_eq!(msgs.len(), 4);
    for msg in &msgs {
        match msg {
            Message::RequestVote { term, candidate_id } => {
                assert_eq!(*term, 1);
                assert_eq!(*candidate_id, 1);
            }
            _ => panic!("expected RequestVote"),
        }
    }
}

#[test] fn win_election_with_majority() {
    let mut node = RaftNode::new(1, vec![2, 3, 4, 5]);
    node.receive_message(Message::Timeout); // term 1, candidate
    // Need 2 more votes (already voted for self = 1, need 3 total for majority of 5)
    let msgs1 = node.receive_message(Message::VoteResponse { term: 1, vote_granted: true });
    assert_eq!(node.state(), State::Candidate); // 2 votes, need 3
    assert!(msgs1.is_empty());

    let msgs2 = node.receive_message(Message::VoteResponse { term: 1, vote_granted: true });
    assert_eq!(node.state(), State::Leader); // 3 votes = majority of 5
    // Should send heartbeats to all peers
    assert_eq!(msgs2.len(), 4);
    for msg in &msgs2 {
        assert!(matches!(msg, Message::Heartbeat { term: 1, leader_id: 1 }));
    }
}

#[test] fn rejected_votes_dont_count() {
    let mut node = RaftNode::new(1, vec![2, 3, 4, 5]);
    node.receive_message(Message::Timeout);
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: false });
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: false });
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: false });
    assert_eq!(node.state(), State::Candidate); // still candidate
}

#[test] fn higher_term_reverts_to_follower() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    node.receive_message(Message::Timeout); // term 1, candidate
    node.receive_message(Message::RequestVote { term: 5, candidate_id: 2 });
    assert_eq!(node.state(), State::Follower);
    assert_eq!(node.current_term(), 5);
}

#[test] fn grant_vote_once_per_term() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    // Receive vote request from node 2, term 1
    let msgs1 = node.receive_message(Message::RequestVote { term: 1, candidate_id: 2 });
    assert_eq!(node.voted_for(), Some(2));
    assert!(msgs1.iter().any(|m| matches!(m, Message::VoteResponse { vote_granted: true, .. })));

    // Receive vote request from node 3, same term — should deny
    let msgs2 = node.receive_message(Message::RequestVote { term: 1, candidate_id: 3 });
    assert_eq!(node.voted_for(), Some(2)); // still voted for 2
    assert!(msgs2.iter().any(|m| matches!(m, Message::VoteResponse { vote_granted: false, .. })));
}

#[test] fn heartbeat_resets_follower() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    node.receive_message(Message::Timeout); // term 1, candidate
    let msgs = node.receive_message(Message::Heartbeat { term: 1, leader_id: 2 });
    assert_eq!(node.state(), State::Follower);
    assert_eq!(node.current_term(), 1);
    assert!(msgs.iter().any(|m| matches!(m, Message::HeartbeatResponse { term: 1 })));
}

#[test] fn stale_vote_response_ignored() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    node.receive_message(Message::Timeout); // term 1
    node.receive_message(Message::Timeout); // term 2
    // Vote from term 1 should not count
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: true });
    assert_eq!(node.state(), State::Candidate); // still candidate, stale vote ignored
}

#[test] fn split_vote_then_new_election() {
    let mut node = RaftNode::new(1, vec![2, 3, 4, 5]);
    node.receive_message(Message::Timeout); // term 1, candidate
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: true }); // 2 votes
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: false });
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: false });
    // Split vote — still candidate
    assert_eq!(node.state(), State::Candidate);

    // Timeout again — new election at term 2
    let msgs = node.receive_message(Message::Timeout);
    assert_eq!(node.current_term(), 2);
    assert_eq!(node.state(), State::Candidate);
    assert_eq!(msgs.len(), 4); // new RequestVote to all peers
}

#[test] fn leader_steps_down_on_higher_term() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    node.receive_message(Message::Timeout); // term 1
    node.receive_message(Message::VoteResponse { term: 1, vote_granted: true }); // wins (2 of 3)
    assert_eq!(node.state(), State::Leader);

    // Receive heartbeat from higher term
    node.receive_message(Message::Heartbeat { term: 5, leader_id: 2 });
    assert_eq!(node.state(), State::Follower);
    assert_eq!(node.current_term(), 5);
}

#[test] fn deny_vote_for_lower_term() {
    let mut node = RaftNode::new(1, vec![2, 3]);
    // Manually set node to term 5 by receiving high-term message
    node.receive_message(Message::Heartbeat { term: 5, leader_id: 2 });
    assert_eq!(node.current_term(), 5);

    // Request vote for term 3 — should deny
    let msgs = node.receive_message(Message::RequestVote { term: 3, candidate_id: 3 });
    assert!(msgs.iter().any(|m| matches!(m, Message::VoteResponse { vote_granted: false, .. })));
}

#[test] fn three_node_cluster_election() {
    let mut n1 = RaftNode::new(1, vec![2, 3]);
    let mut n2 = RaftNode::new(2, vec![1, 3]);
    let mut n3 = RaftNode::new(3, vec![1, 2]);

    // Node 1 times out, starts election
    let vote_reqs = n1.receive_message(Message::Timeout);
    assert_eq!(n1.state(), State::Candidate);

    // Deliver RequestVote to n2 and n3
    let resp2 = n2.receive_message(vote_reqs[0].clone()); // to one peer
    let resp3 = n3.receive_message(vote_reqs[1].clone()); // to other peer

    // Both should grant
    assert!(resp2.iter().any(|m| matches!(m, Message::VoteResponse { vote_granted: true, .. })));
    assert!(resp3.iter().any(|m| matches!(m, Message::VoteResponse { vote_granted: true, .. })));

    // Deliver first vote response — should win (2 of 3 including self)
    let heartbeats = n1.receive_message(resp2[0].clone());
    assert_eq!(n1.state(), State::Leader);
    assert!(!heartbeats.is_empty()); // sends heartbeats
}
"#,
        ),
    ]
}
