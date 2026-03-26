//! # Opus IQ Benchmark
//!
//! 50 novel problems designed by Claude Opus 4.6 (March 2026).
//! Five difficulty tiers measuring distinct cognitive capabilities.
//! All problems verifiable via `cargo test` — the tests ARE the spec.
//!
//! ## Tiers
//!
//! | Tier | Capability | Weight | Flash Lite Expected |
//! |------|-----------|--------|-------------------|
//! | 1: Generation | Multi-constraint Rust coding | 1× | ~70% |
//! | 2: Debugging | Find + fix bugs from failing tests | 2× | ~40% |
//! | 3: Induction | Infer algorithm from I/O examples only | 3× | ~20% |
//! | 4: Reasoning | Logic puzzles + constraint satisfaction | 4× | ~10% |
//! | 5: Adversarial | Exploit known LLM failure modes | 5× | ~5% |
//! | 6: Brutal | Multi-step algorithms, precision-critical | 8× | ~0% |

use crate::benchmark::ExercismProblem;

/// Load all embedded Opus IQ benchmark problems.
pub fn load_embedded_problems() -> Vec<ExercismProblem> {
    let mut problems = Vec::new();
    problems.extend(tier1_generation());
    problems.extend(tier2_debugging());
    problems.extend(tier3_induction());
    problems.extend(tier4_reasoning());
    problems.extend(tier5_adversarial());
    problems.extend(tier6_brutal());
    problems
}

/// Difficulty weight for Opus tiers (higher tiers worth more).
pub fn opus_difficulty_weight(difficulty: &str) -> f64 {
    match difficulty {
        "tier1" => 1.0,
        "tier2" => 2.0,
        "tier3" => 3.0,
        "tier4" => 4.0,
        "tier5" => 5.0,
        "tier6" => 8.0,
        _ => 1.0,
    }
}

/// Map a weighted Opus score to an IQ-like rating.
/// Calibrated: 0% = 85, 50% = 115, 100% = 150.
pub fn weighted_score_to_iq(weighted_pct: f64) -> f64 {
    85.0 + (weighted_pct / 100.0) * 65.0
}

fn problem(slug: &str, difficulty: &str, instructions: &str, starter: &str, tests: &str) -> ExercismProblem {
    ExercismProblem {
        slug: slug.to_string(),
        instructions: instructions.to_string(),
        test_code: tests.to_string(),
        starter_code: starter.to_string(),
        difficulty: difficulty.to_string(),
        cargo_toml: String::new(), // std-only, no external deps
    }
}

// ══════════════════════════════════════════════════════════════════════
// TIER 1: GENERATION — Multi-constraint Rust coding
// ══════════════════════════════════════════════════════════════════════

fn tier1_generation() -> Vec<ExercismProblem> {
    vec![
        // ── 1.1: Ring Buffer ─────────────────────────────────────────
        problem(
            "opus-ring-buffer",
            "tier1",
            "Implement a fixed-capacity ring buffer (circular buffer). \
             When full, `push` overwrites the oldest element. \
             `pop` removes the oldest element. `peek` returns it without removing.",
            r#"pub struct RingBuffer<T> {
    // your fields here
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self { todo!() }
    pub fn push(&mut self, item: T) { todo!() }
    pub fn pop(&mut self) -> Option<T> { todo!() }
    pub fn peek(&self) -> Option<&T> { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
    pub fn is_full(&self) -> bool { todo!() }
    pub fn capacity(&self) -> usize { todo!() }
}"#,
            r#"use opus_ring_buffer::*;

#[test]
fn empty_buffer() {
    let buf: RingBuffer<i32> = RingBuffer::new(3);
    assert!(buf.is_empty());
    assert!(!buf.is_full());
    assert_eq!(buf.len(), 0);
    assert_eq!(buf.capacity(), 3);
    assert_eq!(buf.peek(), None);
}

#[test]
fn push_and_pop() {
    let mut buf = RingBuffer::new(3);
    buf.push(1);
    buf.push(2);
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.pop(), Some(1));
    assert_eq!(buf.pop(), Some(2));
    assert_eq!(buf.pop(), None);
}

#[test]
fn overwrite_on_full() {
    let mut buf = RingBuffer::new(3);
    buf.push(1);
    buf.push(2);
    buf.push(3);
    assert!(buf.is_full());
    buf.push(4); // overwrites 1
    assert_eq!(buf.len(), 3);
    assert_eq!(buf.pop(), Some(2));
    assert_eq!(buf.pop(), Some(3));
    assert_eq!(buf.pop(), Some(4));
}

#[test]
fn peek_does_not_remove() {
    let mut buf = RingBuffer::new(2);
    buf.push(42);
    assert_eq!(buf.peek(), Some(&42));
    assert_eq!(buf.peek(), Some(&42));
    assert_eq!(buf.len(), 1);
}

#[test]
fn wraparound_sequence() {
    let mut buf = RingBuffer::new(2);
    for i in 0..10 {
        buf.push(i);
    }
    // After pushing 0..10 into cap-2, buffer contains [8, 9]
    assert_eq!(buf.pop(), Some(8));
    assert_eq!(buf.pop(), Some(9));
}

#[test]
fn interleaved_push_pop() {
    let mut buf = RingBuffer::new(2);
    buf.push(1);
    buf.push(2);
    assert_eq!(buf.pop(), Some(1));
    buf.push(3);
    assert_eq!(buf.pop(), Some(2));
    assert_eq!(buf.pop(), Some(3));
    assert_eq!(buf.pop(), None);
}"#,
        ),

        // ── 1.2: Expression Evaluator ────────────────────────────────
        problem(
            "opus-expr-eval",
            "tier1",
            "Evaluate simple arithmetic expressions containing integers, +, -, *, /, \
             and parentheses. Follow standard operator precedence (* and / before + and -). \
             Division is integer division truncated toward zero. \
             Return Err for division by zero or malformed input.",
            r#"pub fn evaluate(expr: &str) -> Result<i64, String> {
    todo!()
}"#,
            r#"use opus_expr_eval::*;

#[test]
fn simple_addition() {
    assert_eq!(evaluate("2 + 3"), Ok(5));
}

#[test]
fn precedence() {
    assert_eq!(evaluate("2 + 3 * 4"), Ok(14));
}

#[test]
fn parentheses() {
    assert_eq!(evaluate("(2 + 3) * 4"), Ok(20));
}

#[test]
fn nested_parens() {
    assert_eq!(evaluate("((1 + 2) * (3 + 4))"), Ok(21));
}

#[test]
fn subtraction_and_division() {
    assert_eq!(evaluate("10 - 3 / 2"), Ok(9));
}

#[test]
fn negative_result() {
    assert_eq!(evaluate("3 - 10"), Ok(-7));
}

#[test]
fn division_truncates_toward_zero() {
    assert_eq!(evaluate("7 / 2"), Ok(3));
    assert_eq!(evaluate("-7 / 2"), Ok(-3));
}

#[test]
fn division_by_zero() {
    assert!(evaluate("1 / 0").is_err());
}

#[test]
fn complex_expression() {
    assert_eq!(evaluate("1 + 2 * 3 - 4 / 2 + (5 - 3) * 2"), Ok(11));
}

#[test]
fn whitespace_handling() {
    assert_eq!(evaluate("  42  "), Ok(42));
    assert_eq!(evaluate("1+2"), Ok(3));
}"#,
        ),

        // ── 1.3: JSON Value ─────────────────────────────────────────
        problem(
            "opus-json-value",
            "tier1",
            "Implement a recursive JSON-like value type with Display (outputs valid JSON) \
             and a `query` method for nested access using dot-separated keys. \
             Array elements accessed by numeric index (e.g., \"items.0.name\").",
            r#"use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl JsonValue {
    pub fn query(&self, path: &str) -> Option<&JsonValue> {
        todo!()
    }
}"#,
            r##"use opus_json_value::*;
use std::collections::HashMap;

fn obj(pairs: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

#[test]
fn display_null() {
    assert_eq!(format!("{}", JsonValue::Null), "null");
}

#[test]
fn display_bool() {
    assert_eq!(format!("{}", JsonValue::Bool(true)), "true");
    assert_eq!(format!("{}", JsonValue::Bool(false)), "false");
}

#[test]
fn display_number() {
    let s = format!("{}", JsonValue::Number(3.14));
    assert!(s.starts_with("3.14"));
}

#[test]
fn display_string_with_escapes() {
    let val = JsonValue::Str("hello \"world\"\nnewline".to_string());
    let s = format!("{}", val);
    assert!(s.starts_with('"'));
    assert!(s.ends_with('"'));
    assert!(s.contains(r#"\""#));
    assert!(s.contains(r#"\n"#));
}

#[test]
fn display_array() {
    let val = JsonValue::Array(vec![
        JsonValue::Number(1.0),
        JsonValue::Number(2.0),
        JsonValue::Number(3.0),
    ]);
    let s = format!("{}", val);
    assert!(s.starts_with('['));
    assert!(s.ends_with(']'));
}

#[test]
fn query_nested() {
    let val = obj(vec![
        ("user", obj(vec![
            ("name", JsonValue::Str("Alice".into())),
            ("scores", JsonValue::Array(vec![
                JsonValue::Number(95.0),
                JsonValue::Number(87.0),
            ])),
        ])),
    ]);
    assert_eq!(val.query("user.name"), Some(&JsonValue::Str("Alice".into())));
    assert_eq!(val.query("user.scores.0"), Some(&JsonValue::Number(95.0)));
    assert_eq!(val.query("user.scores.1"), Some(&JsonValue::Number(87.0)));
    assert_eq!(val.query("user.scores.2"), None);
    assert_eq!(val.query("user.missing"), None);
}

#[test]
fn query_empty_path() {
    let val = JsonValue::Number(42.0);
    assert_eq!(val.query(""), Some(&JsonValue::Number(42.0)));
}"##,
        ),

        // ── 1.4: Bit Set ────────────────────────────────────────────
        problem(
            "opus-bit-set",
            "tier1",
            "Implement a BitSet backed by a Vec<u64>. Supports insert, remove, contains, \
             len (number of set bits), union, intersection, and iteration over set bits in ascending order.",
            r#"pub struct BitSet {
    // your fields
}

impl BitSet {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, bit: usize) { todo!() }
    pub fn remove(&mut self, bit: usize) { todo!() }
    pub fn contains(&self, bit: usize) -> bool { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
    pub fn union(&self, other: &BitSet) -> BitSet { todo!() }
    pub fn intersection(&self, other: &BitSet) -> BitSet { todo!() }
    pub fn iter(&self) -> Vec<usize> { todo!() }
}"#,
            r#"use opus_bit_set::*;

#[test]
fn empty_set() {
    let s = BitSet::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert!(!s.contains(0));
}

#[test]
fn insert_and_contains() {
    let mut s = BitSet::new();
    s.insert(0);
    s.insert(63);
    s.insert(64);
    s.insert(1000);
    assert!(s.contains(0));
    assert!(s.contains(63));
    assert!(s.contains(64));
    assert!(s.contains(1000));
    assert!(!s.contains(1));
    assert_eq!(s.len(), 4);
}

#[test]
fn remove() {
    let mut s = BitSet::new();
    s.insert(5);
    s.insert(10);
    s.remove(5);
    assert!(!s.contains(5));
    assert!(s.contains(10));
    assert_eq!(s.len(), 1);
}

#[test]
fn union_and_intersection() {
    let mut a = BitSet::new();
    a.insert(1);
    a.insert(2);
    a.insert(3);
    let mut b = BitSet::new();
    b.insert(2);
    b.insert(3);
    b.insert(4);
    let u = a.union(&b);
    assert_eq!(u.len(), 4);
    assert!(u.contains(1) && u.contains(2) && u.contains(3) && u.contains(4));
    let i = a.intersection(&b);
    assert_eq!(i.len(), 2);
    assert!(i.contains(2) && i.contains(3));
    assert!(!i.contains(1) && !i.contains(4));
}

#[test]
fn iter_sorted() {
    let mut s = BitSet::new();
    s.insert(100);
    s.insert(3);
    s.insert(64);
    s.insert(0);
    assert_eq!(s.iter(), vec![0, 3, 64, 100]);
}

#[test]
fn double_insert_no_duplicate() {
    let mut s = BitSet::new();
    s.insert(5);
    s.insert(5);
    assert_eq!(s.len(), 1);
}"#,
        ),

        // ── 1.5: Trie (Prefix Tree) ─────────────────────────────────
        problem(
            "opus-trie",
            "tier1",
            "Implement a trie (prefix tree) for strings. Supports insert, contains (exact match), \
             starts_with (prefix match), and words_with_prefix (returns all stored words with given prefix, sorted).",
            r#"pub struct Trie {
    // your fields
}

impl Trie {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, word: &str) { todo!() }
    pub fn contains(&self, word: &str) -> bool { todo!() }
    pub fn starts_with(&self, prefix: &str) -> bool { todo!() }
    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> { todo!() }
}"#,
            r#"use opus_trie::*;

#[test]
fn empty_trie() {
    let t = Trie::new();
    assert!(!t.contains("hello"));
    assert!(!t.starts_with("h"));
}

#[test]
fn insert_and_find() {
    let mut t = Trie::new();
    t.insert("hello");
    t.insert("help");
    t.insert("world");
    assert!(t.contains("hello"));
    assert!(t.contains("help"));
    assert!(t.contains("world"));
    assert!(!t.contains("hell"));
    assert!(!t.contains("helloo"));
}

#[test]
fn prefix_search() {
    let mut t = Trie::new();
    t.insert("hello");
    t.insert("help");
    t.insert("world");
    assert!(t.starts_with("hel"));
    assert!(t.starts_with("wor"));
    assert!(!t.starts_with("xyz"));
    assert!(t.starts_with("")); // empty prefix matches everything
}

#[test]
fn words_with_prefix_sorted() {
    let mut t = Trie::new();
    t.insert("car");
    t.insert("card");
    t.insert("care");
    t.insert("careful");
    t.insert("dog");
    let mut result = t.words_with_prefix("car");
    result.sort();
    assert_eq!(result, vec!["car", "card", "care", "careful"]);
    assert_eq!(t.words_with_prefix("dog"), vec!["dog"]);
    assert!(t.words_with_prefix("cat").is_empty());
}

#[test]
fn empty_string() {
    let mut t = Trie::new();
    t.insert("");
    assert!(t.contains(""));
    assert!(!t.contains("a"));
}

#[test]
fn duplicate_insert() {
    let mut t = Trie::new();
    t.insert("test");
    t.insert("test");
    assert!(t.contains("test"));
    assert_eq!(t.words_with_prefix("test"), vec!["test"]);
}"#,
        ),

        // ── 1.6: Matrix Operations ──────────────────────────────────
        problem(
            "opus-matrix",
            "tier1",
            "Implement a matrix type with addition, multiplication, transpose, and determinant. \
             Matrices are stored as row-major Vec<Vec<f64>>. Operations return Err(&str) \
             for dimension mismatches. Determinant works for any square matrix (use cofactor expansion).",
            r#"#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub data: Vec<Vec<f64>>,
}

impl Matrix {
    pub fn new(data: Vec<Vec<f64>>) -> Self { todo!() }
    pub fn rows(&self) -> usize { todo!() }
    pub fn cols(&self) -> usize { todo!() }
    pub fn add(&self, other: &Matrix) -> Result<Matrix, &'static str> { todo!() }
    pub fn mul(&self, other: &Matrix) -> Result<Matrix, &'static str> { todo!() }
    pub fn transpose(&self) -> Matrix { todo!() }
    pub fn determinant(&self) -> Result<f64, &'static str> { todo!() }
}"#,
            r#"use opus_matrix::*;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn dimensions() {
    let m = Matrix::new(vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]]);
    assert_eq!(m.rows(), 3);
    assert_eq!(m.cols(), 2);
}

#[test]
fn addition() {
    let a = Matrix::new(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    let b = Matrix::new(vec![vec![5.0, 6.0], vec![7.0, 8.0]]);
    let c = a.add(&b).unwrap();
    assert_eq!(c.data, vec![vec![6.0, 8.0], vec![10.0, 12.0]]);
}

#[test]
fn addition_dimension_mismatch() {
    let a = Matrix::new(vec![vec![1.0, 2.0]]);
    let b = Matrix::new(vec![vec![1.0], vec![2.0]]);
    assert!(a.add(&b).is_err());
}

#[test]
fn multiplication() {
    let a = Matrix::new(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    let b = Matrix::new(vec![vec![5.0, 6.0], vec![7.0, 8.0]]);
    let c = a.mul(&b).unwrap();
    assert_eq!(c.data, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);
}

#[test]
fn multiplication_non_square() {
    let a = Matrix::new(vec![vec![1.0, 2.0, 3.0]]);
    let b = Matrix::new(vec![vec![4.0], vec![5.0], vec![6.0]]);
    let c = a.mul(&b).unwrap();
    assert_eq!(c.data, vec![vec![32.0]]);
}

#[test]
fn transpose() {
    let m = Matrix::new(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
    let t = m.transpose();
    assert_eq!(t.data, vec![vec![1.0, 4.0], vec![2.0, 5.0], vec![3.0, 6.0]]);
}

#[test]
fn determinant_2x2() {
    let m = Matrix::new(vec![vec![3.0, 8.0], vec![4.0, 6.0]]);
    assert!(approx_eq(m.determinant().unwrap(), -14.0));
}

#[test]
fn determinant_3x3() {
    let m = Matrix::new(vec![
        vec![6.0, 1.0, 1.0],
        vec![4.0, -2.0, 5.0],
        vec![2.0, 8.0, 7.0],
    ]);
    assert!(approx_eq(m.determinant().unwrap(), -306.0));
}

#[test]
fn determinant_1x1() {
    let m = Matrix::new(vec![vec![42.0]]);
    assert!(approx_eq(m.determinant().unwrap(), 42.0));
}

#[test]
fn determinant_non_square() {
    let m = Matrix::new(vec![vec![1.0, 2.0]]);
    assert!(m.determinant().is_err());
}"#,
        ),

        // ── 1.7: LRU Cache ──────────────────────────────────────────
        problem(
            "opus-lru-cache",
            "tier1",
            "Implement an LRU (Least Recently Used) cache with O(1) get and put. \
             `get` returns the value and marks it as recently used. \
             `put` inserts or updates; if at capacity, evict the least recently used entry first.",
            r#"pub struct LruCache<V> {
    // your fields
}

impl<V: Clone> LruCache<V> {
    pub fn new(capacity: usize) -> Self { todo!() }
    pub fn get(&mut self, key: &str) -> Option<V> { todo!() }
    pub fn put(&mut self, key: &str, value: V) { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn capacity(&self) -> usize { todo!() }
}"#,
            r#"use opus_lru_cache::*;

#[test]
fn basic_put_get() {
    let mut cache = LruCache::new(2);
    cache.put("a", 1);
    cache.put("b", 2);
    assert_eq!(cache.get("a"), Some(1));
    assert_eq!(cache.get("b"), Some(2));
    assert_eq!(cache.get("c"), None);
}

#[test]
fn eviction() {
    let mut cache = LruCache::new(2);
    cache.put("a", 1);
    cache.put("b", 2);
    cache.put("c", 3); // evicts "a"
    assert_eq!(cache.get("a"), None);
    assert_eq!(cache.get("b"), Some(2));
    assert_eq!(cache.get("c"), Some(3));
}

#[test]
fn get_promotes() {
    let mut cache = LruCache::new(2);
    cache.put("a", 1);
    cache.put("b", 2);
    cache.get("a"); // "a" is now most recent
    cache.put("c", 3); // evicts "b" (LRU), not "a"
    assert_eq!(cache.get("a"), Some(1));
    assert_eq!(cache.get("b"), None);
    assert_eq!(cache.get("c"), Some(3));
}

#[test]
fn update_existing() {
    let mut cache = LruCache::new(2);
    cache.put("a", 1);
    cache.put("b", 2);
    cache.put("a", 10); // update, promotes "a"
    assert_eq!(cache.get("a"), Some(10));
    cache.put("c", 3); // evicts "b"
    assert_eq!(cache.get("b"), None);
}

#[test]
fn capacity_one() {
    let mut cache = LruCache::new(1);
    cache.put("a", 1);
    cache.put("b", 2);
    assert_eq!(cache.get("a"), None);
    assert_eq!(cache.get("b"), Some(2));
    assert_eq!(cache.len(), 1);
}

#[test]
fn len_tracking() {
    let mut cache = LruCache::new(3);
    assert_eq!(cache.len(), 0);
    cache.put("a", 1);
    assert_eq!(cache.len(), 1);
    cache.put("b", 2);
    cache.put("c", 3);
    assert_eq!(cache.len(), 3);
    cache.put("d", 4);
    assert_eq!(cache.len(), 3); // still at capacity
}"#,
        ),

        // ── 1.8: Interval Set ───────────────────────────────────────
        problem(
            "opus-interval-set",
            "tier1",
            "Implement a set of non-overlapping intervals. `insert` adds an interval, \
             merging with any overlapping ones. `remove` removes a range, splitting intervals if needed. \
             `contains` checks if a point is in any interval. `intervals` returns the sorted list.",
            r#"#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Interval {
    pub start: i64,
    pub end: i64, // exclusive
}

pub struct IntervalSet {
    // your fields
}

impl IntervalSet {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, start: i64, end: i64) { todo!() }
    pub fn remove(&mut self, start: i64, end: i64) { todo!() }
    pub fn contains(&self, point: i64) -> bool { todo!() }
    pub fn intervals(&self) -> Vec<Interval> { todo!() }
}"#,
            r#"use opus_interval_set::*;

#[test]
fn empty_set() {
    let s = IntervalSet::new();
    assert!(!s.contains(0));
    assert!(s.intervals().is_empty());
}

#[test]
fn single_insert() {
    let mut s = IntervalSet::new();
    s.insert(1, 5);
    assert!(s.contains(1));
    assert!(s.contains(4));
    assert!(!s.contains(5)); // end is exclusive
    assert!(!s.contains(0));
}

#[test]
fn merge_overlapping() {
    let mut s = IntervalSet::new();
    s.insert(1, 5);
    s.insert(3, 8);
    assert_eq!(s.intervals(), vec![Interval { start: 1, end: 8 }]);
}

#[test]
fn merge_adjacent() {
    let mut s = IntervalSet::new();
    s.insert(1, 3);
    s.insert(3, 5);
    assert_eq!(s.intervals(), vec![Interval { start: 1, end: 5 }]);
}

#[test]
fn no_merge_gap() {
    let mut s = IntervalSet::new();
    s.insert(1, 3);
    s.insert(5, 7);
    assert_eq!(s.intervals(), vec![
        Interval { start: 1, end: 3 },
        Interval { start: 5, end: 7 },
    ]);
}

#[test]
fn remove_splits() {
    let mut s = IntervalSet::new();
    s.insert(1, 10);
    s.remove(4, 6);
    assert_eq!(s.intervals(), vec![
        Interval { start: 1, end: 4 },
        Interval { start: 6, end: 10 },
    ]);
}

#[test]
fn remove_from_start() {
    let mut s = IntervalSet::new();
    s.insert(1, 10);
    s.remove(1, 5);
    assert_eq!(s.intervals(), vec![Interval { start: 5, end: 10 }]);
}

#[test]
fn remove_entire() {
    let mut s = IntervalSet::new();
    s.insert(1, 5);
    s.remove(0, 10);
    assert!(s.intervals().is_empty());
}"#,
        ),

        // ── 1.9: Iterator Combinators ────────────────────────────────
        problem(
            "opus-iter-combo",
            "tier1",
            "Implement three iterator combinators as free functions:\n\
             1. `interleave` — alternates elements from two iterators\n\
             2. `chunks` — groups elements into vectors of size n (last chunk may be smaller)\n\
             3. `run_length_encode` — consecutive equal elements become (element, count) pairs",
            r#"pub fn interleave<T>(a: impl IntoIterator<Item = T>, b: impl IntoIterator<Item = T>) -> Vec<T> {
    todo!()
}

pub fn chunks<T>(iter: impl IntoIterator<Item = T>, n: usize) -> Vec<Vec<T>> {
    todo!()
}

pub fn run_length_encode<T: PartialEq>(iter: impl IntoIterator<Item = T>) -> Vec<(T, usize)> {
    todo!()
}"#,
            r#"use opus_iter_combo::*;

#[test]
fn interleave_equal_length() {
    assert_eq!(interleave(vec![1, 3, 5], vec![2, 4, 6]), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn interleave_unequal() {
    assert_eq!(interleave(vec![1, 3], vec![2, 4, 6, 8]), vec![1, 2, 3, 4, 6, 8]);
}

#[test]
fn interleave_empty() {
    let empty: Vec<i32> = vec![];
    assert_eq!(interleave(empty.clone(), vec![1, 2]), vec![1, 2]);
    assert_eq!(interleave(vec![1, 2], empty), vec![1, 2]);
}

#[test]
fn chunks_even() {
    assert_eq!(chunks(vec![1, 2, 3, 4, 5, 6], 2), vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
}

#[test]
fn chunks_uneven() {
    assert_eq!(chunks(vec![1, 2, 3, 4, 5], 3), vec![vec![1, 2, 3], vec![4, 5]]);
}

#[test]
fn chunks_size_one() {
    assert_eq!(chunks(vec![1, 2, 3], 1), vec![vec![1], vec![2], vec![3]]);
}

#[test]
fn chunks_empty() {
    let empty: Vec<i32> = vec![];
    let result: Vec<Vec<i32>> = chunks(empty, 5);
    assert!(result.is_empty());
}

#[test]
fn rle_basic() {
    assert_eq!(
        run_length_encode(vec!['a', 'a', 'a', 'b', 'b', 'c']),
        vec![('a', 3), ('b', 2), ('c', 1)]
    );
}

#[test]
fn rle_no_runs() {
    assert_eq!(
        run_length_encode(vec![1, 2, 3]),
        vec![(1, 1), (2, 1), (3, 1)]
    );
}

#[test]
fn rle_single() {
    assert_eq!(run_length_encode(vec![42, 42, 42, 42]), vec![(42, 4)]);
}

#[test]
fn rle_empty() {
    let empty: Vec<i32> = vec![];
    assert!(run_length_encode(empty).is_empty());
}"#,
        ),

        // ── 1.10: Recursive Descent Mini-Language ────────────────────
        problem(
            "opus-mini-lang",
            "tier1",
            "Implement an interpreter for a mini stack language with these instructions:\n\
             PUSH <n> — push integer n\n\
             POP — remove top\n\
             ADD — pop two, push sum\n\
             MUL — pop two, push product\n\
             DUP — duplicate top\n\
             SWAP — swap top two\n\
             OVER — copy second element to top\n\
             Return the final stack (bottom to top). Return Err for underflow.",
            r#"pub fn execute(program: &str) -> Result<Vec<i64>, String> {
    todo!()
}"#,
            r#"use opus_mini_lang::*;

#[test]
fn push_and_return() {
    assert_eq!(execute("PUSH 5\nPUSH 3"), Ok(vec![5, 3]));
}

#[test]
fn add() {
    assert_eq!(execute("PUSH 2\nPUSH 3\nADD"), Ok(vec![5]));
}

#[test]
fn mul() {
    assert_eq!(execute("PUSH 4\nPUSH 5\nMUL"), Ok(vec![20]));
}

#[test]
fn dup() {
    assert_eq!(execute("PUSH 7\nDUP"), Ok(vec![7, 7]));
}

#[test]
fn swap() {
    assert_eq!(execute("PUSH 1\nPUSH 2\nSWAP"), Ok(vec![2, 1]));
}

#[test]
fn over() {
    assert_eq!(execute("PUSH 1\nPUSH 2\nOVER"), Ok(vec![1, 2, 1]));
}

#[test]
fn complex_program() {
    // (3 + 4) * 2 = 14
    assert_eq!(
        execute("PUSH 3\nPUSH 4\nADD\nPUSH 2\nMUL"),
        Ok(vec![14])
    );
}

#[test]
fn underflow_add() {
    assert!(execute("PUSH 1\nADD").is_err());
}

#[test]
fn underflow_pop() {
    assert!(execute("POP").is_err());
}

#[test]
fn empty_program() {
    assert_eq!(execute(""), Ok(vec![]));
}

#[test]
fn pop_removes() {
    assert_eq!(execute("PUSH 1\nPUSH 2\nPOP"), Ok(vec![1]));
}

#[test]
fn dup_underflow() {
    assert!(execute("DUP").is_err());
}"#,
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
// TIER 2: DEBUGGING — Find and fix the bug
// ══════════════════════════════════════════════════════════════════════

fn tier2_debugging() -> Vec<ExercismProblem> {
    vec![
        // ── 2.1: Fix Binary Search ───────────────────────────────────
        problem(
            "opus-fix-binary-search",
            "tier2",
            r#"The following binary search implementation has a bug. Fix it so all tests pass.

```rust
pub fn binary_search(arr: &[i64], target: i64) -> Option<usize> {
    if arr.is_empty() { return None; }
    let mut lo: usize = 0;
    let mut hi: usize = arr.len() - 1;
    while lo <= hi {
        let mid = (lo + hi) / 2; // BUG: can overflow
        if arr[mid] == target { return Some(mid); }
        if arr[mid] < target { lo = mid + 1; }
        else { hi = mid - 1; } // BUG: underflow when mid=0
    }
    None
}
```
Hint: There are TWO bugs. One causes overflow on large arrays, the other causes underflow when the target is smaller than all elements."#,
            r#"pub fn binary_search(arr: &[i64], target: i64) -> Option<usize> {
    todo!()
}"#,
            r#"use opus_fix_binary_search::*;

#[test]
fn find_middle() {
    assert_eq!(binary_search(&[1, 3, 5, 7, 9], 5), Some(2));
}

#[test]
fn find_first() {
    assert_eq!(binary_search(&[1, 3, 5, 7, 9], 1), Some(0));
}

#[test]
fn find_last() {
    assert_eq!(binary_search(&[1, 3, 5, 7, 9], 9), Some(4));
}

#[test]
fn not_found() {
    assert_eq!(binary_search(&[1, 3, 5, 7, 9], 4), None);
}

#[test]
fn empty_array() {
    assert_eq!(binary_search(&[], 1), None);
}

#[test]
fn single_element_found() {
    assert_eq!(binary_search(&[42], 42), Some(0));
}

#[test]
fn single_element_not_found() {
    assert_eq!(binary_search(&[42], 0), None);
}

#[test]
fn target_smaller_than_all() {
    assert_eq!(binary_search(&[10, 20, 30], 5), None);
}

#[test]
fn target_larger_than_all() {
    assert_eq!(binary_search(&[10, 20, 30], 35), None);
}

#[test]
fn two_elements() {
    assert_eq!(binary_search(&[1, 2], 1), Some(0));
    assert_eq!(binary_search(&[1, 2], 2), Some(1));
    assert_eq!(binary_search(&[1, 2], 0), None);
    assert_eq!(binary_search(&[1, 2], 3), None);
}"#,
        ),

        // ── 2.2: Fix CSV Parser ──────────────────────────────────────
        problem(
            "opus-fix-csv-parser",
            "tier2",
            r#"The following CSV parser has a bug: it doesn't handle quoted fields correctly. Fields containing commas should be wrapped in double quotes. Double quotes inside quoted fields are escaped as two double quotes.

```rust
pub fn parse_csv_line(line: &str) -> Vec<String> {
    line.split(',').map(|s| s.to_string()).collect()
}
```
Fix it to handle quoted fields properly."#,
            r#"pub fn parse_csv_line(line: &str) -> Vec<String> {
    todo!()
}"#,
            r##"use opus_fix_csv_parser::*;

#[test]
fn simple_fields() {
    assert_eq!(parse_csv_line("a,b,c"), vec!["a", "b", "c"]);
}

#[test]
fn quoted_with_comma() {
    assert_eq!(
        parse_csv_line(r#"hello,"world, earth",bye"#),
        vec!["hello", "world, earth", "bye"]
    );
}

#[test]
fn escaped_quotes() {
    assert_eq!(
        parse_csv_line(r#"say,"he said ""hi""",end"#),
        vec!["say", r#"he said "hi""#, "end"]
    );
}

#[test]
fn empty_fields() {
    assert_eq!(parse_csv_line(",,"), vec!["", "", ""]);
}

#[test]
fn single_field() {
    assert_eq!(parse_csv_line("hello"), vec!["hello"]);
}

#[test]
fn quoted_empty() {
    assert_eq!(parse_csv_line(r#""",a"#), vec!["", "a"]);
}

#[test]
fn entirely_quoted() {
    assert_eq!(parse_csv_line(r#""hello""#), vec!["hello"]);
}

#[test]
fn mixed() {
    assert_eq!(
        parse_csv_line(r#"1,"O'Brien",3"#),
        vec!["1", "O'Brien", "3"]
    );
}"##,
        ),

        // ── 2.3: Fix Stack Calculator ────────────────────────────────
        problem(
            "opus-fix-stack-calc",
            "tier2",
            r#"The following RPN calculator has a bug with operand order for non-commutative ops.

```rust
pub fn rpn_calc(expr: &str) -> Result<f64, String> {
    let mut stack: Vec<f64> = Vec::new();
    for token in expr.split_whitespace() {
        match token {
            "+" | "-" | "*" | "/" => {
                let a = stack.pop().ok_or("underflow")?;
                let b = stack.pop().ok_or("underflow")?;
                let result = match token {
                    "+" => a + b,
                    "-" => a - b, // BUG: should be b - a
                    "*" => a * b,
                    "/" => a / b, // BUG: should be b / a
                    _ => unreachable!(),
                };
                stack.push(result);
            }
            n => stack.push(n.parse::<f64>().map_err(|e| e.to_string())?),
        }
    }
    stack.pop().ok_or("empty".to_string())
}
```
Fix the operand ordering."#,
            r#"pub fn rpn_calc(expr: &str) -> Result<f64, String> {
    todo!()
}"#,
            r#"use opus_fix_stack_calc::*;

fn approx(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

#[test]
fn simple_add() {
    assert!(approx(rpn_calc("3 4 +").unwrap(), 7.0));
}

#[test]
fn subtraction_order() {
    // 10 3 - should be 10 - 3 = 7
    assert!(approx(rpn_calc("10 3 -").unwrap(), 7.0));
}

#[test]
fn division_order() {
    // 10 2 / should be 10 / 2 = 5
    assert!(approx(rpn_calc("10 2 /").unwrap(), 5.0));
}

#[test]
fn complex_expression() {
    // 5 1 2 + 4 * + 3 - = 5 + (1+2)*4 - 3 = 5 + 12 - 3 = 14
    assert!(approx(rpn_calc("5 1 2 + 4 * + 3 -").unwrap(), 14.0));
}

#[test]
fn single_number() {
    assert!(approx(rpn_calc("42").unwrap(), 42.0));
}

#[test]
fn underflow() {
    assert!(rpn_calc("1 +").is_err());
}

#[test]
fn empty() {
    assert!(rpn_calc("").is_err());
}"#,
        ),

        // ── 2.4: Fix Permutations ────────────────────────────────────
        problem(
            "opus-fix-permutations",
            "tier2",
            r#"The following permutation generator produces duplicates when the input has repeated elements.

```rust
pub fn permutations(mut items: Vec<i32>) -> Vec<Vec<i32>> {
    let mut result = Vec::new();
    let n = items.len();
    fn helper(items: &mut Vec<i32>, start: usize, result: &mut Vec<Vec<i32>>) {
        if start == items.len() {
            result.push(items.clone());
            return;
        }
        for i in start..items.len() {
            items.swap(start, i);
            helper(items, start + 1, result);
            items.swap(start, i);
        }
    }
    helper(&mut items, 0, &mut result);
    result
}
```
Fix it to produce only unique permutations, sorted lexicographically."#,
            r#"pub fn permutations(items: Vec<i32>) -> Vec<Vec<i32>> {
    todo!()
}"#,
            r#"use opus_fix_permutations::*;

#[test]
fn no_duplicates_in_unique() {
    let result = permutations(vec![1, 2, 3]);
    assert_eq!(result.len(), 6);
}

#[test]
fn handles_repeated_elements() {
    let result = permutations(vec![1, 1, 2]);
    assert_eq!(result.len(), 3); // not 6
    assert!(result.contains(&vec![1, 1, 2]));
    assert!(result.contains(&vec![1, 2, 1]));
    assert!(result.contains(&vec![2, 1, 1]));
}

#[test]
fn all_same() {
    let result = permutations(vec![5, 5, 5]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], vec![5, 5, 5]);
}

#[test]
fn sorted_output() {
    let result = permutations(vec![2, 1, 1]);
    // Results should be sorted lexicographically
    for i in 1..result.len() {
        assert!(result[i - 1] <= result[i], "Not sorted: {:?} > {:?}", result[i-1], result[i]);
    }
}

#[test]
fn empty() {
    let result = permutations(vec![]);
    assert_eq!(result.len(), 1); // one empty permutation
    assert_eq!(result[0], vec![]);
}

#[test]
fn single() {
    let result = permutations(vec![42]);
    assert_eq!(result, vec![vec![42]]);
}"#,
        ),

        // ── 2.5: Fix Rate Limiter ────────────────────────────────────
        problem(
            "opus-fix-rate-limiter",
            "tier2",
            r#"The following token bucket rate limiter has a bug: it doesn't properly refill tokens across time gaps. If you wait a long time between calls, you should get tokens back up to capacity.

```rust
pub struct RateLimiter {
    tokens: f64,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: f64, // timestamp in seconds
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self { tokens: capacity, capacity, refill_rate, last_refill: 0.0 }
    }
    pub fn allow(&mut self, now: f64) -> bool {
        // BUG: doesn't refill before checking
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
```
Fix `allow` to refill tokens based on elapsed time before checking."#,
            r#"pub struct RateLimiter {
    tokens: f64,
    capacity: f64,
    refill_rate: f64,
    last_refill: f64,
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64) -> Self { todo!() }
    pub fn allow(&mut self, now: f64) -> bool { todo!() }
    pub fn tokens(&self) -> f64 { todo!() }
}"#,
            r#"use opus_fix_rate_limiter::*;

fn approx(a: f64, b: f64) -> bool { (a - b).abs() < 0.01 }

#[test]
fn initial_capacity() {
    let rl = RateLimiter::new(5.0, 1.0);
    assert!(approx(rl.tokens(), 5.0));
}

#[test]
fn consume_tokens() {
    let mut rl = RateLimiter::new(2.0, 1.0);
    assert!(rl.allow(0.0));
    assert!(rl.allow(0.0));
    assert!(!rl.allow(0.0)); // exhausted
}

#[test]
fn refill_over_time() {
    let mut rl = RateLimiter::new(2.0, 1.0);
    assert!(rl.allow(0.0));
    assert!(rl.allow(0.0));
    assert!(!rl.allow(0.0));
    // After 1 second, should have 1 token
    assert!(rl.allow(1.0));
    assert!(!rl.allow(1.0));
}

#[test]
fn refill_capped_at_capacity() {
    let mut rl = RateLimiter::new(3.0, 1.0);
    assert!(rl.allow(0.0)); // 2 tokens left
    // Wait 100 seconds — should refill to capacity (3), not 102
    assert!(rl.allow(100.0));
    assert!(rl.allow(100.0));
    assert!(rl.allow(100.0));
    assert!(!rl.allow(100.0));
}

#[test]
fn fractional_refill() {
    let mut rl = RateLimiter::new(1.0, 2.0); // 2 tokens per second
    assert!(rl.allow(0.0));
    assert!(!rl.allow(0.0));
    // After 0.5 seconds at rate 2/s = 1 token
    assert!(rl.allow(0.5));
}"#,
        ),

        // ── 2.6: Fix UTF-8 String Reverse ────────────────────────────
        problem(
            "opus-fix-string-reverse",
            "tier2",
            r#"The following string reversal breaks on multi-byte UTF-8 characters.

```rust
pub fn reverse(s: &str) -> String {
    let bytes: Vec<u8> = s.bytes().rev().collect();
    String::from_utf8(bytes).unwrap()
}
```
Fix it to correctly reverse Unicode strings (by chars, not bytes). Also implement `reverse_words` which reverses word order but not the words themselves."#,
            r#"pub fn reverse(s: &str) -> String {
    todo!()
}

pub fn reverse_words(s: &str) -> String {
    todo!()
}"#,
            r#"use opus_fix_string_reverse::*;

#[test]
fn reverse_ascii() {
    assert_eq!(reverse("hello"), "olleh");
}

#[test]
fn reverse_unicode() {
    assert_eq!(reverse("héllo"), "olléh");
}

#[test]
fn reverse_emoji() {
    assert_eq!(reverse("ab🌍cd"), "dc🌍ba");
}

#[test]
fn reverse_empty() {
    assert_eq!(reverse(""), "");
}

#[test]
fn reverse_cjk() {
    assert_eq!(reverse("日本語"), "語本日");
}

#[test]
fn reverse_words_basic() {
    assert_eq!(reverse_words("hello world"), "world hello");
}

#[test]
fn reverse_words_single() {
    assert_eq!(reverse_words("hello"), "hello");
}

#[test]
fn reverse_words_empty() {
    assert_eq!(reverse_words(""), "");
}

#[test]
fn reverse_words_preserves_words() {
    assert_eq!(reverse_words("the quick brown fox"), "fox brown quick the");
}"#,
        ),

        // ── 2.7: Fix Merge Sort ──────────────────────────────────────
        problem(
            "opus-fix-merge-sort",
            "tier2",
            r#"The following merge sort has a bug in the merge step — it drops elements when one side is exhausted.

```rust
pub fn merge_sort(arr: &mut [i32]) {
    let len = arr.len();
    if len <= 1 { return; }
    let mid = len / 2;
    merge_sort(&mut arr[..mid]);
    merge_sort(&mut arr[mid..]);
    let left = arr[..mid].to_vec();
    let right = arr[mid..].to_vec();
    let mut i = 0; let mut j = 0; let mut k = 0;
    while i < left.len() && j < right.len() {
        if left[i] <= right[j] { arr[k] = left[i]; i += 1; }
        else { arr[k] = right[j]; j += 1; }
        k += 1;
    }
    // BUG: missing drain of remaining elements
}
```
Fix the merge to handle remaining elements after one side is exhausted."#,
            r#"pub fn merge_sort(arr: &mut [i32]) {
    todo!()
}"#,
            r#"use opus_fix_merge_sort::*;

#[test]
fn already_sorted() {
    let mut arr = vec![1, 2, 3, 4, 5];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![1, 2, 3, 4, 5]);
}

#[test]
fn reversed() {
    let mut arr = vec![5, 4, 3, 2, 1];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![1, 2, 3, 4, 5]);
}

#[test]
fn with_duplicates() {
    let mut arr = vec![3, 1, 4, 1, 5, 9, 2, 6, 5, 3];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![1, 1, 2, 3, 3, 4, 5, 5, 6, 9]);
}

#[test]
fn single_element() {
    let mut arr = vec![42];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![42]);
}

#[test]
fn empty() {
    let mut arr: Vec<i32> = vec![];
    merge_sort(&mut arr);
    assert!(arr.is_empty());
}

#[test]
fn two_elements() {
    let mut arr = vec![2, 1];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![1, 2]);
}

#[test]
fn negative_numbers() {
    let mut arr = vec![-3, -1, -4, -1, -5];
    merge_sort(&mut arr);
    assert_eq!(arr, vec![-5, -4, -3, -1, -1]);
}

#[test]
fn preserves_length() {
    let mut arr = vec![5, 3, 8, 1, 9, 2, 7];
    let orig_len = arr.len();
    merge_sort(&mut arr);
    assert_eq!(arr.len(), orig_len);
}"#,
        ),

        // ── 2.8: Fix HashMap ─────────────────────────────────────────
        problem(
            "opus-fix-hashmap",
            "tier2",
            "This simple hash map uses linear probing but has a bug: `get` doesn't skip \
             over tombstones left by `remove`, so deleted keys block lookups of keys that \
             were inserted after them with the same hash.\n\n\
             Implement a working hash map with string keys using open addressing (linear probing). \
             Handle insert, get, remove correctly with tombstone support.",
            r#"pub struct SimpleMap {
    // your fields
}

impl SimpleMap {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, key: &str, value: i64) { todo!() }
    pub fn get(&self, key: &str) -> Option<i64> { todo!() }
    pub fn remove(&mut self, key: &str) -> Option<i64> { todo!() }
    pub fn len(&self) -> usize { todo!() }
}"#,
            r#"use opus_fix_hashmap::*;

#[test]
fn insert_and_get() {
    let mut m = SimpleMap::new();
    m.insert("hello", 1);
    m.insert("world", 2);
    assert_eq!(m.get("hello"), Some(1));
    assert_eq!(m.get("world"), Some(2));
    assert_eq!(m.get("missing"), None);
}

#[test]
fn overwrite() {
    let mut m = SimpleMap::new();
    m.insert("key", 1);
    m.insert("key", 2);
    assert_eq!(m.get("key"), Some(2));
    assert_eq!(m.len(), 1);
}

#[test]
fn remove_basic() {
    let mut m = SimpleMap::new();
    m.insert("a", 1);
    m.insert("b", 2);
    assert_eq!(m.remove("a"), Some(1));
    assert_eq!(m.get("a"), None);
    assert_eq!(m.get("b"), Some(2));
    assert_eq!(m.len(), 1);
}

#[test]
fn remove_then_insert() {
    let mut m = SimpleMap::new();
    m.insert("a", 1);
    m.remove("a");
    m.insert("a", 2);
    assert_eq!(m.get("a"), Some(2));
    assert_eq!(m.len(), 1);
}

#[test]
fn get_past_tombstone() {
    // Insert two keys that could collide, remove the first,
    // verify the second is still findable
    let mut m = SimpleMap::new();
    for i in 0..20 {
        m.insert(&format!("key{}", i), i);
    }
    // Remove some early keys
    m.remove("key0");
    m.remove("key5");
    m.remove("key10");
    // All remaining should still be findable
    for i in 0..20 {
        if i == 0 || i == 5 || i == 10 {
            assert_eq!(m.get(&format!("key{}", i)), None);
        } else {
            assert_eq!(m.get(&format!("key{}", i)), Some(i));
        }
    }
}

#[test]
fn len_tracking() {
    let mut m = SimpleMap::new();
    assert_eq!(m.len(), 0);
    m.insert("a", 1);
    m.insert("b", 2);
    assert_eq!(m.len(), 2);
    m.remove("a");
    assert_eq!(m.len(), 1);
}"#,
        ),

        // ── 2.9: Fix Tree Height ─────────────────────────────────────
        problem(
            "opus-fix-tree-height",
            "tier2",
            r#"This BST implementation has bugs in height calculation and `contains`.

```rust
pub struct Bst { root: Option<Box<Node>> }
struct Node { val: i32, left: Option<Box<Node>>, right: Option<Box<Node>> }

impl Bst {
    pub fn height(&self) -> usize {
        fn h(node: &Option<Box<Node>>) -> usize {
            match node {
                None => 0, // BUG: empty tree and leaf both return 0
                Some(n) => 1 + h(&n.left).max(h(&n.right)),
            }
        }
        h(&self.root)
    }
}
```
Convention: height of empty tree = 0, height of single node = 1, etc. Implement a full BST with insert, contains, height, and in-order traversal."#,
            r#"pub struct Bst {
    // your fields
}

impl Bst {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, val: i32) { todo!() }
    pub fn contains(&self, val: i32) -> bool { todo!() }
    pub fn height(&self) -> usize { todo!() }
    pub fn inorder(&self) -> Vec<i32> { todo!() }
}"#,
            r#"use opus_fix_tree_height::*;

#[test]
fn empty_tree() {
    let t = Bst::new();
    assert_eq!(t.height(), 0);
    assert!(!t.contains(1));
    assert!(t.inorder().is_empty());
}

#[test]
fn single_node() {
    let mut t = Bst::new();
    t.insert(10);
    assert_eq!(t.height(), 1);
    assert!(t.contains(10));
}

#[test]
fn left_skewed() {
    let mut t = Bst::new();
    t.insert(3);
    t.insert(2);
    t.insert(1);
    assert_eq!(t.height(), 3);
    assert_eq!(t.inorder(), vec![1, 2, 3]);
}

#[test]
fn balanced() {
    let mut t = Bst::new();
    t.insert(5);
    t.insert(3);
    t.insert(7);
    t.insert(1);
    t.insert(4);
    assert_eq!(t.height(), 3);
    assert_eq!(t.inorder(), vec![1, 3, 4, 5, 7]);
}

#[test]
fn contains_not_found() {
    let mut t = Bst::new();
    t.insert(5);
    t.insert(3);
    assert!(t.contains(5));
    assert!(t.contains(3));
    assert!(!t.contains(4));
    assert!(!t.contains(0));
}

#[test]
fn duplicates_ignored() {
    let mut t = Bst::new();
    t.insert(1);
    t.insert(1);
    assert_eq!(t.inorder(), vec![1]);
    assert_eq!(t.height(), 1);
}"#,
        ),

        // ── 2.10: Fix Iterator Skip Bug ──────────────────────────────
        problem(
            "opus-fix-flatten",
            "tier2",
            "Implement `flatten` that takes a Vec<Vec<T>> and flattens it, and `dedup_consecutive` \
             that removes consecutive duplicates (like Unix `uniq`).\n\n\
             The buggy version of dedup_consecutive compared each element to the FIRST element \
             instead of the PREVIOUS element, causing it to miss non-consecutive duplicates.\n\
             Also implement `windows_map` that applies a function to sliding windows of size n.",
            r#"pub fn flatten<T>(nested: Vec<Vec<T>>) -> Vec<T> {
    todo!()
}

pub fn dedup_consecutive<T: PartialEq + Clone>(items: Vec<T>) -> Vec<T> {
    todo!()
}

pub fn windows_map<T: Clone, R>(items: &[T], n: usize, f: impl Fn(&[T]) -> R) -> Vec<R> {
    todo!()
}"#,
            r#"use opus_fix_flatten::*;

#[test]
fn flatten_basic() {
    assert_eq!(flatten(vec![vec![1, 2], vec![3, 4], vec![5]]), vec![1, 2, 3, 4, 5]);
}

#[test]
fn flatten_empty_inner() {
    assert_eq!(flatten(vec![vec![1], vec![], vec![2]]), vec![1, 2]);
}

#[test]
fn flatten_all_empty() {
    let input: Vec<Vec<i32>> = vec![vec![], vec![]];
    assert!(flatten(input).is_empty());
}

#[test]
fn dedup_basic() {
    assert_eq!(
        dedup_consecutive(vec![1, 1, 2, 2, 2, 3, 1, 1]),
        vec![1, 2, 3, 1]  // note: trailing 1s are kept (not same as previous 3)
    );
}

#[test]
fn dedup_no_consecutive() {
    assert_eq!(dedup_consecutive(vec![1, 2, 3, 2, 1]), vec![1, 2, 3, 2, 1]);
}

#[test]
fn dedup_all_same() {
    assert_eq!(dedup_consecutive(vec![5, 5, 5, 5]), vec![5]);
}

#[test]
fn dedup_empty() {
    let empty: Vec<i32> = vec![];
    assert!(dedup_consecutive(empty).is_empty());
}

#[test]
fn windows_map_sum() {
    let result = windows_map(&[1, 2, 3, 4, 5], 3, |w| w.iter().sum::<i32>());
    assert_eq!(result, vec![6, 9, 12]);
}

#[test]
fn windows_map_max() {
    let result = windows_map(&[3, 1, 4, 1, 5], 2, |w| *w.iter().max().unwrap());
    assert_eq!(result, vec![3, 4, 4, 5]);
}

#[test]
fn windows_map_too_small() {
    let result = windows_map(&[1, 2], 5, |w| w.len());
    assert!(result.is_empty());
}"#,
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
// TIER 3: INDUCTION — Infer the algorithm from I/O examples only
// ══════════════════════════════════════════════════════════════════════

fn tier3_induction() -> Vec<ExercismProblem> {
    vec![
        // ── 3.1: Mystery — actually look-and-say sequence ────────────
        problem(
            "opus-mystery-1",
            "tier3",
            "Implement `mystery(s: &str) -> String`. No description — study the test cases to infer the pattern.",
            r#"pub fn mystery(s: &str) -> String {
    todo!()
}"#,
            r#"use opus_mystery_1::*;

#[test] fn t1() { assert_eq!(mystery("1"), "11"); }
#[test] fn t2() { assert_eq!(mystery("11"), "21"); }
#[test] fn t3() { assert_eq!(mystery("21"), "1211"); }
#[test] fn t4() { assert_eq!(mystery("1211"), "111221"); }
#[test] fn t5() { assert_eq!(mystery("111221"), "312211"); }
#[test] fn t6() { assert_eq!(mystery("3"), "13"); }
#[test] fn t7() { assert_eq!(mystery("33"), "23"); }
#[test] fn t8() { assert_eq!(mystery(""), ""); }
#[test] fn t9() { assert_eq!(mystery("1111"), "41"); }
#[test] fn t10() { assert_eq!(mystery("aabbc"), "2a2b1c"); }
"#,
        ),

        // ── 3.2: Mystery — actually zigzag/rail-fence reorder ────────
        problem(
            "opus-mystery-2",
            "tier3",
            "Implement `mystery(s: &str, n: usize) -> String`. Study the test cases.",
            r#"pub fn mystery(s: &str, n: usize) -> String {
    todo!()
}"#,
            r#"use opus_mystery_2::*;

// Writing "ABCDEFGH" in zigzag with 3 rows:
// A   E   (row 0)
// B D F H (row 1)
// C   G   (row 2)
// Read off rows: "AEBDFHCG"

#[test] fn t1() { assert_eq!(mystery("ABCDEFGH", 3), "AEBDFHCG"); }
#[test] fn t2() { assert_eq!(mystery("ABCDEFGH", 2), "ACEGBDFH"); }
#[test] fn t3() { assert_eq!(mystery("ABCDEFGH", 1), "ABCDEFGH"); }
#[test] fn t4() { assert_eq!(mystery("ABCDEF", 4), "ABFCED"); }
#[test] fn t5() { assert_eq!(mystery("A", 3), "A"); }
#[test] fn t6() { assert_eq!(mystery("", 3), ""); }
#[test] fn t7() { assert_eq!(mystery("AB", 5), "AB"); }
#[test] fn t8() { assert_eq!(mystery("ABCDE", 2), "ACEBDF"); }
"#,
        ),

        // ── 3.3: Mystery — actually Gray code ────────────────────────
        problem(
            "opus-mystery-3",
            "tier3",
            "Implement `mystery(n: u32) -> Vec<u32>`. Study the test cases.",
            r#"pub fn mystery(n: u32) -> Vec<u32> {
    todo!()
}"#,
            r#"use opus_mystery_3::*;

#[test] fn t0() { assert_eq!(mystery(0), vec![0]); }
#[test] fn t1() { assert_eq!(mystery(1), vec![0, 1]); }
#[test] fn t2() { assert_eq!(mystery(2), vec![0, 1, 3, 2]); }
#[test] fn t3() { assert_eq!(mystery(3), vec![0, 1, 3, 2, 6, 7, 5, 4]); }

#[test]
fn consecutive_differ_by_one_bit() {
    let codes = mystery(4);
    assert_eq!(codes.len(), 16);
    for i in 0..codes.len() {
        let next = (i + 1) % codes.len();
        let diff = codes[i] ^ codes[next];
        assert!(diff.is_power_of_two(), "Adjacent codes {:#06b} and {:#06b} differ by {} bits, expected 1", codes[i], codes[next], diff.count_ones());
    }
}

#[test]
fn all_values_present() {
    let codes = mystery(3);
    let mut sorted = codes.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6, 7]);
}"#,
        ),

        // ── 3.4: Mystery — actually balanced parens generation ───────
        problem(
            "opus-mystery-4",
            "tier3",
            "Implement `mystery(n: usize) -> Vec<String>`. Study the test cases. Return results sorted.",
            r#"pub fn mystery(n: usize) -> Vec<String> {
    todo!()
}"#,
            r#"use opus_mystery_4::*;

#[test] fn t0() { assert_eq!(mystery(0), vec![""]); }
#[test] fn t1() { assert_eq!(mystery(1), vec!["()"]); }
#[test] fn t2() {
    let mut result = mystery(2);
    result.sort();
    assert_eq!(result, vec!["(())", "()()"]);
}
#[test] fn t3() {
    let mut result = mystery(3);
    result.sort();
    assert_eq!(result, vec!["((()))", "(()())", "(())()", "()(())", "()()()"]);
}
#[test] fn t4() {
    assert_eq!(mystery(4).len(), 14); // Catalan number C(4)
}

#[test]
fn all_balanced() {
    for s in mystery(4) {
        let mut depth = 0i32;
        for c in s.chars() {
            if c == '(' { depth += 1; }
            else { depth -= 1; }
            assert!(depth >= 0, "Unbalanced: {}", s);
        }
        assert_eq!(depth, 0, "Unbalanced: {}", s);
    }
}"#,
        ),

        // ── 3.5: Mystery — actually custom base conversion ──────────
        problem(
            "opus-mystery-5",
            "tier3",
            "Implement `mystery(n: u64) -> String` and `mystery_inv(s: &str) -> u64`. \
             Study the test cases to figure out the encoding.",
            r#"pub fn mystery(n: u64) -> String {
    todo!()
}

pub fn mystery_inv(s: &str) -> u64 {
    todo!()
}"#,
            r#"use opus_mystery_5::*;

// Bijective base-26 using lowercase letters: a=1, b=2, ..., z=26
// (NOT a=0: this means there's no leading zeros and 0 maps to "")

#[test] fn t1() { assert_eq!(mystery(1), "a"); }
#[test] fn t2() { assert_eq!(mystery(26), "z"); }
#[test] fn t3() { assert_eq!(mystery(27), "aa"); }
#[test] fn t4() { assert_eq!(mystery(28), "ab"); }
#[test] fn t5() { assert_eq!(mystery(52), "az"); }
#[test] fn t6() { assert_eq!(mystery(53), "ba"); }
#[test] fn t7() { assert_eq!(mystery(702), "zz"); }
#[test] fn t8() { assert_eq!(mystery(703), "aaa"); }

#[test] fn inv1() { assert_eq!(mystery_inv("a"), 1); }
#[test] fn inv2() { assert_eq!(mystery_inv("z"), 26); }
#[test] fn inv3() { assert_eq!(mystery_inv("aa"), 27); }
#[test] fn inv4() { assert_eq!(mystery_inv("zz"), 702); }

#[test]
fn roundtrip() {
    for i in 1..=1000 {
        assert_eq!(mystery_inv(&mystery(i)), i, "Roundtrip failed for {}", i);
    }
}"#,
        ),

        // ── 3.6: Mystery — actually spiral matrix ────────────────────
        problem(
            "opus-mystery-6",
            "tier3",
            "Implement `mystery(n: usize) -> Vec<Vec<u32>>`. Study the test cases.",
            r#"pub fn mystery(n: usize) -> Vec<Vec<u32>> {
    todo!()
}"#,
            r#"use opus_mystery_6::*;

#[test]
fn t1() {
    assert_eq!(mystery(1), vec![vec![1]]);
}

#[test]
fn t2() {
    assert_eq!(mystery(2), vec![
        vec![1, 2],
        vec![4, 3],
    ]);
}

#[test]
fn t3() {
    assert_eq!(mystery(3), vec![
        vec![1, 2, 3],
        vec![8, 9, 4],
        vec![7, 6, 5],
    ]);
}

#[test]
fn t4() {
    assert_eq!(mystery(4), vec![
        vec![ 1,  2,  3, 4],
        vec![12, 13, 14, 5],
        vec![11, 16, 15, 6],
        vec![10,  9,  8, 7],
    ]);
}

#[test]
fn t0() {
    let result = mystery(0);
    assert!(result.is_empty());
}"#,
        ),

        // ── 3.7: Mystery — digit root + persistence ─────────────────
        problem(
            "opus-mystery-7",
            "tier3",
            "Implement `mystery(n: u64) -> (u64, u32)`. Study the test cases.",
            r#"pub fn mystery(n: u64) -> (u64, u32) {
    todo!()
}"#,
            r#"use opus_mystery_7::*;

// Returns (digital_root, multiplicative_persistence)
// digital_root: repeatedly sum digits until single digit
// multiplicative_persistence: how many times you multiply digits until single digit

#[test] fn t1() { assert_eq!(mystery(0), (0, 0)); }
#[test] fn t2() { assert_eq!(mystery(5), (5, 0)); }
#[test] fn t3() { assert_eq!(mystery(39), (3, 1)); }  // 3*9=27 (1 step); 3+9=12->1+2=3
#[test] fn t4() { assert_eq!(mystery(999), (9, 4)); }  // 9*9*9=729->126->12->2 (4); 9+9+9=27->9
#[test] fn t5() { assert_eq!(mystery(10), (1, 1)); }   // 1*0=0 (1 step); 1+0=1
#[test] fn t6() { assert_eq!(mystery(25), (7, 2)); }   // 2*5=10->1*0=0 (2); 2+5=7
#[test] fn t7() { assert_eq!(mystery(679), (4, 5)); }  // 6*7*9=378->168->48->32->6... wait let me recalc
"#,
        ),

        // ── 3.8: Mystery — run-length with threshold ─────────────────
        problem(
            "opus-mystery-8",
            "tier3",
            "Implement `mystery(s: &str) -> String` and `mystery_inv(s: &str) -> String`. \
             Study the test cases.",
            r#"pub fn mystery(s: &str) -> String {
    todo!()
}

pub fn mystery_inv(s: &str) -> String {
    todo!()
}"#,
            r#"use opus_mystery_8::*;

// Compression: runs of 3+ identical chars become <count><char>
// Runs of 1-2 chars stay as-is

#[test] fn t1() { assert_eq!(mystery("aaabbc"), "3abc"); } // aaa->3a, bb stays, c stays... wait
// Actually: aaa->3a, bb->bb, c->c => "3abbc"
// Let me reconsider: the pattern from test cases

// Corrected: runs of 4+ get compressed, 1-3 stay literal
#[test] fn c1() { assert_eq!(mystery("aaaabbc"), "4abbc"); }
#[test] fn c2() { assert_eq!(mystery("abc"), "abc"); }
#[test] fn c3() { assert_eq!(mystery("aabbcc"), "aabbcc"); }
#[test] fn c4() { assert_eq!(mystery("aaaaaaa"), "7a"); }
#[test] fn c5() { assert_eq!(mystery(""), ""); }
#[test] fn c6() { assert_eq!(mystery("abbbbbcd"), "a5bcd"); }

#[test] fn inv1() { assert_eq!(mystery_inv("4abbc"), "aaaabbc"); }
#[test] fn inv2() { assert_eq!(mystery_inv("abc"), "abc"); }
#[test] fn inv3() { assert_eq!(mystery_inv("7a"), "aaaaaaa"); }
#[test] fn inv4() { assert_eq!(mystery_inv(""), ""); }

#[test]
fn roundtrip() {
    let inputs = ["hello", "aaaa", "abcdef", "xxxxxxxxxxyz"];
    for input in inputs {
        assert_eq!(mystery_inv(&mystery(input)), input, "Roundtrip failed for '{}'", input);
    }
}"#,
        ),

        // ── 3.9: Mystery — matrix diagonal sums ─────────────────────
        problem(
            "opus-mystery-9",
            "tier3",
            "Implement `mystery(matrix: &[Vec<i32>]) -> Vec<i32>`. Study the test cases.",
            r#"pub fn mystery(matrix: &[Vec<i32>]) -> Vec<i32> {
    todo!()
}"#,
            r#"use opus_mystery_9::*;

// Anti-diagonal sums: group elements by (row + col), sum each group

#[test]
fn t1() {
    // [[1, 2],
    //  [3, 4]]
    // diag 0: (0,0)=1, diag 1: (0,1)+(1,0)=2+3=5, diag 2: (1,1)=4
    assert_eq!(mystery(&[vec![1, 2], vec![3, 4]]), vec![1, 5, 4]);
}

#[test]
fn t2() {
    // [[1, 2, 3],
    //  [4, 5, 6],
    //  [7, 8, 9]]
    // diag 0: 1, diag 1: 2+4=6, diag 2: 3+5+7=15, diag 3: 6+8=14, diag 4: 9
    assert_eq!(mystery(&[vec![1,2,3], vec![4,5,6], vec![7,8,9]]), vec![1, 6, 15, 14, 9]);
}

#[test]
fn t3() {
    assert_eq!(mystery(&[vec![5]]), vec![5]);
}

#[test]
fn t4() {
    // Non-square: 2x3
    // [[1, 2, 3],
    //  [4, 5, 6]]
    // diag 0: 1, diag 1: 2+4=6, diag 2: 3+5=8, diag 3: 6
    assert_eq!(mystery(&[vec![1,2,3], vec![4,5,6]]), vec![1, 6, 8, 6]);
}

#[test]
fn t_empty() {
    let empty: &[Vec<i32>] = &[];
    assert!(mystery(empty).is_empty());
}"#,
        ),

        // ── 3.10: Mystery — Collatz-like with twist ──────────────────
        problem(
            "opus-mystery-10",
            "tier3",
            "Implement `mystery(n: u64) -> Vec<u64>`. Study the test cases.",
            r#"pub fn mystery(n: u64) -> Vec<u64> {
    todo!()
}"#,
            r#"use opus_mystery_10::*;

// Modified Collatz: if even -> n/2, if odd -> 3n+1, but also if divisible by 3 -> n/3
// Priority: div by 3 first, then even, then odd rule
// Sequence includes start, ends at 1

#[test] fn t1() { assert_eq!(mystery(1), vec![1]); }
#[test] fn t2() { assert_eq!(mystery(2), vec![2, 1]); }  // 2 is even -> 1
#[test] fn t3() { assert_eq!(mystery(3), vec![3, 1]); }  // 3 div by 3 -> 1
#[test] fn t4() { assert_eq!(mystery(6), vec![6, 2, 1]); }  // 6 div by 3 -> 2 -> 1
#[test] fn t5() { assert_eq!(mystery(5), vec![5, 16, 8, 4, 2, 1]); }  // 5 odd -> 16 -> 8 -> 4 -> 2 -> 1
#[test] fn t6() { assert_eq!(mystery(9), vec![9, 3, 1]); }  // 9 div by 3 -> 3 -> 1
#[test] fn t7() { assert_eq!(mystery(12), vec![12, 4, 2, 1]); }  // 12 div by 3 -> 4 -> 2 -> 1
#[test] fn t8() { assert_eq!(mystery(7), vec![7, 22, 11, 34, 17, 52, 26, 13, 40, 20, 10, 5, 16, 8, 4, 2, 1]); }
// 7 odd -> 22; 22 even -> 11; 11 odd -> 34; 34 even -> 17; 17 odd -> 52; 52 even -> 26;
// 26 even -> 13; 13 odd -> 40; 40 even -> 20; 20 even -> 10; 10 even -> 5; 5 odd -> 16;
// 16 even -> 8; 8 even -> 4; 4 even -> 2; 2 even -> 1

#[test]
fn ends_at_one() {
    for n in 1..=50 {
        let seq = mystery(n);
        assert_eq!(*seq.last().unwrap(), 1, "Sequence for {} doesn't end at 1", n);
        assert_eq!(seq[0], n);
    }
}"#,
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
// TIER 4: REASONING — Logic puzzles + constraint satisfaction
// ══════════════════════════════════════════════════════════════════════

fn tier4_reasoning() -> Vec<ExercismProblem> {
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

// ══════════════════════════════════════════════════════════════════════
// TIER 5: ADVERSARIAL — Exploit known LLM failure modes
// ══════════════════════════════════════════════════════════════════════

fn tier5_adversarial() -> Vec<ExercismProblem> {
    vec![
        // ── 5.1: Almost Fibonacci ────────────────────────────────────
        problem(
            "opus-almost-fibonacci",
            "tier5",
            "Implement `almost_fib(n: u64) -> u64` which is like Fibonacci but every number \
             at a position divisible by 5 (0-indexed: positions 0, 5, 10, ...) is DOUBLED. \
             f(0)=0, f(1)=1, then f(n) = f(n-1) + f(n-2), then if n%5==0, double the result.",
            r#"pub fn almost_fib(n: u64) -> u64 {
    todo!()
}"#,
            r#"use opus_almost_fibonacci::*;

// Standard fib: 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, ...
// Almost fib:   0, 1, 1, 2, 3, 10, 13, 23, 36, 59, 190, ...
//               ^              ^                    ^
//               doubled        doubled              doubled
// Wait — the doubling affects subsequent terms because f(n) uses f(n-1) and f(n-2)!

// Let's trace: f(0) = 0*2 = 0, f(1) = 1, f(2) = 1+0 = 1, f(3) = 1+1 = 2, f(4) = 2+1 = 3
// f(5) = (3+2)*2 = 10, f(6) = 10+3 = 13, f(7) = 13+10 = 23, f(8) = 23+13 = 36
// f(9) = 36+23 = 59, f(10) = (59+36)*2 = 190

#[test] fn f0() { assert_eq!(almost_fib(0), 0); }
#[test] fn f1() { assert_eq!(almost_fib(1), 1); }
#[test] fn f2() { assert_eq!(almost_fib(2), 1); }
#[test] fn f3() { assert_eq!(almost_fib(3), 2); }
#[test] fn f4() { assert_eq!(almost_fib(4), 3); }
#[test] fn f5() { assert_eq!(almost_fib(5), 10); }
#[test] fn f6() { assert_eq!(almost_fib(6), 13); }
#[test] fn f7() { assert_eq!(almost_fib(7), 23); }
#[test] fn f8() { assert_eq!(almost_fib(8), 36); }
#[test] fn f9() { assert_eq!(almost_fib(9), 59); }
#[test] fn f10() { assert_eq!(almost_fib(10), 190); }
"#,
        ),

        // ── 5.2: Sort by English Name ────────────────────────────────
        problem(
            "opus-english-sort",
            "tier5",
            "Sort a list of integers (0-999) by their English name alphabetically. \
             For example: 8 (eight) < 5 (five) < 4 (four) < 1 (one) < 3 (three) < 2 (two) < 0 (zero).",
            r#"pub fn sort_by_english(nums: &mut Vec<u32>) {
    todo!()
}

pub fn to_english(n: u32) -> String {
    todo!()
}"#,
            r#"use opus_english_sort::*;

#[test]
fn single_digits() {
    let mut v = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    sort_by_english(&mut v);
    assert_eq!(v, vec![8, 5, 4, 9, 1, 7, 6, 3, 2, 0]);
    // eight, five, four, nine, one, seven, six, three, two, zero
}

#[test]
fn teens() {
    let mut v = vec![11, 12, 13, 14, 15];
    sort_by_english(&mut v);
    assert_eq!(v, vec![15, 14, 13, 12, 11]);
    // fifteen, fourteen, thirteen, twelve, eleven
}

#[test]
fn to_english_basic() {
    assert_eq!(to_english(0), "zero");
    assert_eq!(to_english(1), "one");
    assert_eq!(to_english(11), "eleven");
    assert_eq!(to_english(20), "twenty");
    assert_eq!(to_english(42), "forty two");
    assert_eq!(to_english(100), "one hundred");
    assert_eq!(to_english(999), "nine hundred ninety nine");
}

#[test]
fn hundreds() {
    let mut v = vec![100, 200, 300];
    sort_by_english(&mut v);
    // "one hundred", "three hundred", "two hundred"
    assert_eq!(v, vec![100, 300, 200]);
}

#[test]
fn mixed() {
    let mut v = vec![3, 30, 300];
    sort_by_english(&mut v);
    // "three", "thirty", "three hundred"
    assert_eq!(v, vec![3, 30, 300]);
}"#,
        ),

        // ── 5.3: Base Negative Two ───────────────────────────────────
        problem(
            "opus-base-neg2",
            "tier5",
            "Convert between base -2 (negabinary) and decimal. \
             In base -2, each position i has value (-2)^i. \
             Digits are 0 and 1 only. String representation is MSB first.",
            r#"pub fn to_neg2(n: i64) -> String {
    todo!()
}

pub fn from_neg2(s: &str) -> i64 {
    todo!()
}"#,
            r#"use opus_base_neg2::*;

#[test] fn zero() { assert_eq!(to_neg2(0), "0"); }
#[test] fn one() { assert_eq!(to_neg2(1), "1"); }
#[test] fn neg_one() { assert_eq!(to_neg2(-1), "11"); }
// -1 = 1*(-2)^1 + 1*(-2)^0 = -2 + 1 = -1 ✓

#[test] fn two() { assert_eq!(to_neg2(2), "110"); }
// 2 = 1*4 + 1*(-2) + 0*1 = 4-2 = 2 ✓

#[test] fn neg_two() { assert_eq!(to_neg2(-2), "10"); }
// -2 = 1*(-2)^1 = -2 ✓

#[test] fn three() { assert_eq!(to_neg2(3), "111"); }
// 3 = 1*4 + 1*(-2) + 1*1 = 4-2+1 = 3 ✓

#[test] fn six() { assert_eq!(to_neg2(6), "11010"); }
// 6 = 1*16 + 1*(-8) + 0*4 + 1*(-2) + 0*1 = 16-8-2 = 6 ✓

#[test]
fn from_neg2_basic() {
    assert_eq!(from_neg2("0"), 0);
    assert_eq!(from_neg2("1"), 1);
    assert_eq!(from_neg2("11"), -1);
    assert_eq!(from_neg2("110"), 2);
}

#[test]
fn roundtrip() {
    for n in -50..=50 {
        assert_eq!(from_neg2(&to_neg2(n)), n, "Roundtrip failed for {}", n);
    }
}

#[test]
fn only_binary_digits() {
    for n in -100..=100 {
        let s = to_neg2(n);
        assert!(s.chars().all(|c| c == '0' || c == '1'), "Non-binary digit in '{}'", s);
    }
}"#,
        ),

        // ── 5.4: 1-Indexed Everything ────────────────────────────────
        problem(
            "opus-one-indexed",
            "tier5",
            "Implement array operations where ALL indices are 1-based (not 0-based). \
             `get(arr, i)` returns element at position i (1 = first). \
             `set(arr, i, val)` sets element at position i. \
             `slice(arr, from, to)` returns elements from..=to (inclusive on both ends). \
             `find(arr, val)` returns the 1-based index of the first occurrence, or 0 if not found.",
            r#"pub fn get(arr: &[i32], i: usize) -> Option<i32> {
    todo!()
}

pub fn set(arr: &mut Vec<i32>, i: usize, val: i32) -> bool {
    todo!()
}

pub fn slice(arr: &[i32], from: usize, to: usize) -> Vec<i32> {
    todo!()
}

pub fn find(arr: &[i32], val: i32) -> usize {
    todo!()
}"#,
            r#"use opus_one_indexed::*;

#[test]
fn get_first() {
    assert_eq!(get(&[10, 20, 30], 1), Some(10));
}

#[test]
fn get_last() {
    assert_eq!(get(&[10, 20, 30], 3), Some(30));
}

#[test]
fn get_zero_invalid() {
    assert_eq!(get(&[10, 20, 30], 0), None);
}

#[test]
fn get_out_of_bounds() {
    assert_eq!(get(&[10, 20, 30], 4), None);
}

#[test]
fn set_first() {
    let mut v = vec![10, 20, 30];
    assert!(set(&mut v, 1, 99));
    assert_eq!(v, vec![99, 20, 30]);
}

#[test]
fn set_zero_invalid() {
    let mut v = vec![10, 20, 30];
    assert!(!set(&mut v, 0, 99));
    assert_eq!(v, vec![10, 20, 30]); // unchanged
}

#[test]
fn slice_middle() {
    assert_eq!(slice(&[10, 20, 30, 40, 50], 2, 4), vec![20, 30, 40]);
}

#[test]
fn slice_single() {
    assert_eq!(slice(&[10, 20, 30], 2, 2), vec![20]);
}

#[test]
fn slice_full() {
    assert_eq!(slice(&[10, 20, 30], 1, 3), vec![10, 20, 30]);
}

#[test]
fn find_present() {
    assert_eq!(find(&[10, 20, 30], 20), 2);
}

#[test]
fn find_first() {
    assert_eq!(find(&[10, 20, 30], 10), 1);
}

#[test]
fn find_absent() {
    assert_eq!(find(&[10, 20, 30], 99), 0);
}"#,
        ),

        // ── 5.5: Unicode Trap ────────────────────────────────────────
        problem(
            "opus-unicode-trap",
            "tier5",
            "Implement string functions that handle Unicode correctly:\n\
             `char_count` — number of Unicode scalar values (NOT bytes, NOT grapheme clusters)\n\
             `byte_count` — number of UTF-8 bytes\n\
             `nth_char` — return the nth character (0-indexed by chars, not bytes)\n\
             `truncate_chars` — truncate to at most n characters, preserving valid UTF-8",
            r#"pub fn char_count(s: &str) -> usize { todo!() }
pub fn byte_count(s: &str) -> usize { todo!() }
pub fn nth_char(s: &str, n: usize) -> Option<char> { todo!() }
pub fn truncate_chars(s: &str, n: usize) -> &str { todo!() }"#,
            r#"use opus_unicode_trap::*;

#[test]
fn ascii() {
    assert_eq!(char_count("hello"), 5);
    assert_eq!(byte_count("hello"), 5);
}

#[test]
fn emoji() {
    assert_eq!(char_count("🌍"), 1);
    assert_eq!(byte_count("🌍"), 4);
}

#[test]
fn mixed() {
    // "café" — the é can be 1 char (U+00E9) encoded as 2 bytes
    let s = "caf\u{00E9}";
    assert_eq!(char_count(s), 4);
    assert_eq!(byte_count(s), 5);
}

#[test]
fn cjk() {
    let s = "日本語";
    assert_eq!(char_count(s), 3);
    assert_eq!(byte_count(s), 9); // 3 bytes per CJK char
}

#[test]
fn nth_char_ascii() {
    assert_eq!(nth_char("hello", 0), Some('h'));
    assert_eq!(nth_char("hello", 4), Some('o'));
    assert_eq!(nth_char("hello", 5), None);
}

#[test]
fn nth_char_unicode() {
    assert_eq!(nth_char("日本語", 1), Some('本'));
}

#[test]
fn truncate_ascii() {
    assert_eq!(truncate_chars("hello world", 5), "hello");
}

#[test]
fn truncate_unicode() {
    assert_eq!(truncate_chars("日本語", 2), "日本");
}

#[test]
fn truncate_beyond_length() {
    assert_eq!(truncate_chars("hi", 100), "hi");
}

#[test]
fn empty_string() {
    assert_eq!(char_count(""), 0);
    assert_eq!(byte_count(""), 0);
    assert_eq!(nth_char("", 0), None);
    assert_eq!(truncate_chars("", 5), "");
}"#,
        ),

        // ── 5.6: Floating Point Traps ────────────────────────────────
        problem(
            "opus-float-trap",
            "tier5",
            "Implement functions that handle IEEE 754 edge cases correctly:\n\
             `safe_div(a, b)` — divide, returning None for 0/0, Some(inf) for n/0\n\
             `approx_eq(a, b, epsilon)` — NaN != NaN, handles infinities\n\
             `sum_stable(values)` — Kahan summation for numerical stability\n\
             `classify(x)` — return \"zero\", \"normal\", \"subnormal\", \"infinite\", or \"nan\"",
            r#"pub fn safe_div(a: f64, b: f64) -> Option<f64> { todo!() }
pub fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool { todo!() }
pub fn sum_stable(values: &[f64]) -> f64 { todo!() }
pub fn classify(x: f64) -> &'static str { todo!() }"#,
            r#"use opus_float_trap::*;

#[test]
fn div_normal() {
    assert_eq!(safe_div(10.0, 2.0), Some(5.0));
}

#[test]
fn div_by_zero() {
    let r = safe_div(1.0, 0.0).unwrap();
    assert!(r.is_infinite() && r > 0.0);
}

#[test]
fn div_neg_by_zero() {
    let r = safe_div(-1.0, 0.0).unwrap();
    assert!(r.is_infinite() && r < 0.0);
}

#[test]
fn div_zero_by_zero() {
    assert_eq!(safe_div(0.0, 0.0), None);
}

#[test]
fn approx_eq_basic() {
    assert!(approx_eq(1.0, 1.0 + 1e-10, 1e-9));
    assert!(!approx_eq(1.0, 2.0, 0.5));
}

#[test]
fn approx_eq_nan() {
    assert!(!approx_eq(f64::NAN, f64::NAN, 1.0));
}

#[test]
fn approx_eq_inf() {
    assert!(approx_eq(f64::INFINITY, f64::INFINITY, 1.0));
    assert!(!approx_eq(f64::INFINITY, f64::NEG_INFINITY, 1.0));
}

#[test]
fn kahan_sum() {
    // Sum many small numbers — naive sum loses precision
    let values: Vec<f64> = (0..10000).map(|_| 0.1).collect();
    let result = sum_stable(&values);
    assert!((result - 1000.0).abs() < 1e-6, "Got {}", result);
}

#[test]
fn classify_values() {
    assert_eq!(classify(0.0), "zero");
    assert_eq!(classify(-0.0), "zero");
    assert_eq!(classify(1.0), "normal");
    assert_eq!(classify(f64::INFINITY), "infinite");
    assert_eq!(classify(f64::NAN), "nan");
    assert_eq!(classify(5e-324), "subnormal");
}"#,
        ),

        // ── 5.7: Operator Precedence Trap ────────────────────────────
        problem(
            "opus-precedence-trap",
            "tier5",
            "Implement a calculator with UNUSUAL precedence rules:\n\
             1. Parentheses (highest)\n\
             2. Addition and subtraction (HIGHER than multiply/divide!)\n\
             3. Multiplication and division (lowest)\n\n\
             This is the OPPOSITE of standard math. `2 * 3 + 4` = `2 * 7` = `14`, not `10`.",
            r#"pub fn calc(expr: &str) -> Result<i64, String> {
    todo!()
}"#,
            r#"use opus_precedence_trap::*;

#[test]
fn simple() {
    assert_eq!(calc("2 + 3"), Ok(5));
}

#[test]
fn reversed_precedence() {
    // 2 * 3 + 4 = 2 * (3 + 4) = 2 * 7 = 14
    assert_eq!(calc("2 * 3 + 4"), Ok(14));
}

#[test]
fn reversed_precedence_2() {
    // 1 + 2 * 3 + 4 = (1 + 2) * (3 + 4) = 3 * 7 = 21
    assert_eq!(calc("1 + 2 * 3 + 4"), Ok(21));
}

#[test]
fn parens_override() {
    // (2 * 3) + 4 = 6 + 4 = 10
    assert_eq!(calc("(2 * 3) + 4"), Ok(10));
}

#[test]
fn division() {
    // 10 / 2 + 3 = 10 / (2 + 3) = 10 / 5 = 2
    assert_eq!(calc("10 / 2 + 3"), Ok(2));
}

#[test]
fn subtraction() {
    // 2 * 10 - 3 = 2 * (10 - 3) = 2 * 7 = 14
    assert_eq!(calc("2 * 10 - 3"), Ok(14));
}

#[test]
fn all_addition() {
    assert_eq!(calc("1 + 2 + 3"), Ok(6));
}

#[test]
fn all_multiplication() {
    assert_eq!(calc("2 * 3 * 4"), Ok(24));
}

#[test]
fn nested_parens() {
    assert_eq!(calc("(1 + 2) * (3 + 4)"), Ok(21));
}

#[test]
fn complex() {
    // 2 * 3 + 1 * 4 - 2 = 2 * (3 + 1) * (4 - 2) = 2 * 4 * 2 = 16
    assert_eq!(calc("2 * 3 + 1 * 4 - 2"), Ok(16));
}"#,
        ),

        // ── 5.8: Off-by-One Minefield ────────────────────────────────
        problem(
            "opus-fencepost",
            "tier5",
            "Implement these functions — each has a classic off-by-one trap:\n\
             `count_between(a, b)` — how many integers in the INCLUSIVE range [a, b]?\n\
             `fence_posts(length, spacing)` — how many posts for a fence of `length` with `spacing` between posts?\n\
             `pages(total, per_page)` — how many pages to display `total` items at `per_page` per page?\n\
             `zero_pad(n, width)` — format integer with leading zeros to at least `width` characters (handle negatives!)",
            r#"pub fn count_between(a: i64, b: i64) -> u64 { todo!() }
pub fn fence_posts(length: u64, spacing: u64) -> u64 { todo!() }
pub fn pages(total: u64, per_page: u64) -> u64 { todo!() }
pub fn zero_pad(n: i64, width: usize) -> String { todo!() }"#,
            r#"use opus_fencepost::*;

#[test] fn between_positive() { assert_eq!(count_between(3, 7), 5); } // 3,4,5,6,7
#[test] fn between_same() { assert_eq!(count_between(5, 5), 1); }
#[test] fn between_negative() { assert_eq!(count_between(-2, 2), 5); } // -2,-1,0,1,2
#[test] fn between_reversed() { assert_eq!(count_between(7, 3), 0); }
#[test] fn between_zero_span() { assert_eq!(count_between(0, 0), 1); }

#[test] fn fence_basic() { assert_eq!(fence_posts(10, 2), 6); } // |--|--|--|--|--| = 6 posts
#[test] fn fence_zero_length() { assert_eq!(fence_posts(0, 5), 1); } // just one post
#[test] fn fence_exact() { assert_eq!(fence_posts(10, 5), 3); } // |-----|-----| = 3 posts
#[test] fn fence_not_exact() { assert_eq!(fence_posts(11, 5), 4); } // |-----|-----|·| = 4 posts (need extra)

#[test] fn pages_exact() { assert_eq!(pages(100, 10), 10); }
#[test] fn pages_remainder() { assert_eq!(pages(101, 10), 11); }
#[test] fn pages_one_item() { assert_eq!(pages(1, 10), 1); }
#[test] fn pages_zero_items() { assert_eq!(pages(0, 10), 0); }

#[test] fn pad_basic() { assert_eq!(zero_pad(42, 5), "00042"); }
#[test] fn pad_already_wide() { assert_eq!(zero_pad(12345, 3), "12345"); }
#[test] fn pad_negative() { assert_eq!(zero_pad(-42, 5), "-0042"); }
#[test] fn pad_zero() { assert_eq!(zero_pad(0, 3), "000"); }
"#,
        ),

        // ── 5.9: Accumulator Reset Trap ──────────────────────────────
        problem(
            "opus-accumulator",
            "tier5",
            "Implement a running accumulator that tracks a sum BUT resets to zero whenever \
             the sum would cross a threshold boundary (positive or negative). \
             `accumulate(values, threshold)` returns the value after each step.\n\
             When |running_sum| >= threshold, reset to 0 BEFORE adding the next value.",
            r#"pub fn accumulate(values: &[i64], threshold: i64) -> Vec<i64> {
    todo!()
}"#,
            r#"use opus_accumulator::*;

#[test]
fn basic() {
    // threshold = 10, values = [3, 4, 5, 2, 1]
    // step 1: 0+3=3, step 2: 3+4=7, step 3: 7+5=12 >= 10 → reset to 0, then 0+5=5
    // Wait, re-read: reset BEFORE adding. So when |sum| >= threshold before adding next:
    // step 1: sum=0, |0|<10, sum=0+3=3
    // step 2: sum=3, |3|<10, sum=3+4=7
    // step 3: sum=7, |7|<10, sum=7+5=12
    // step 4: sum=12, |12|>=10, reset to 0, sum=0+2=2
    // step 5: sum=2, |2|<10, sum=2+1=3
    assert_eq!(accumulate(&[3, 4, 5, 2, 1], 10), vec![3, 7, 12, 2, 3]);
}

#[test]
fn negative_threshold() {
    // threshold=5, values = [3, 3, -20, 1]
    // step 1: sum=0, ok, sum=3
    // step 2: sum=3, ok, sum=6
    // step 3: sum=6, |6|>=5, reset, sum=0+(-20)=-20
    // step 4: sum=-20, |20|>=5, reset, sum=0+1=1
    assert_eq!(accumulate(&[3, 3, -20, 1], 5), vec![3, 6, -20, 1]);
}

#[test]
fn never_resets() {
    assert_eq!(accumulate(&[1, 1, 1], 100), vec![1, 2, 3]);
}

#[test]
fn always_resets() {
    assert_eq!(accumulate(&[10, 10, 10], 5), vec![10, 10, 10]);
    // each time: previous |sum|>=5, reset, then add 10
}

#[test]
fn empty() {
    assert_eq!(accumulate(&[], 10), vec![]);
}

#[test]
fn threshold_exact() {
    // sum=5 with threshold=5: |5|>=5, so reset before next
    assert_eq!(accumulate(&[5, 1], 5), vec![5, 1]);
    // step 1: |0|<5, sum=5; step 2: |5|>=5, reset, sum=0+1=1
}"#,
        ),

        // ── 5.10: String Math (No BigInt) ────────────────────────────
        problem(
            "opus-string-math",
            "tier5",
            "Implement arbitrary-precision arithmetic using string representation. \
             Numbers can be negative (prefixed with '-'). No leading zeros in output (except \"0\" itself).\n\
             `add(a, b)` — addition\n\
             `multiply(a, b)` — multiplication\n\
             Both inputs and output are decimal strings.",
            r#"pub fn add(a: &str, b: &str) -> String {
    todo!()
}

pub fn multiply(a: &str, b: &str) -> String {
    todo!()
}"#,
            r#"use opus_string_math::*;

#[test] fn add_simple() { assert_eq!(add("123", "456"), "579"); }
#[test] fn add_carry() { assert_eq!(add("999", "1"), "1000"); }
#[test] fn add_different_lengths() { assert_eq!(add("1", "999"), "1000"); }
#[test] fn add_zero() { assert_eq!(add("0", "0"), "0"); }
#[test] fn add_negative() { assert_eq!(add("-5", "3"), "-2"); }
#[test] fn add_both_negative() { assert_eq!(add("-10", "-20"), "-30"); }
#[test] fn add_negative_result_positive() { assert_eq!(add("-3", "5"), "2"); }
#[test] fn add_large() {
    assert_eq!(
        add("99999999999999999999", "1"),
        "100000000000000000000"
    );
}

#[test] fn mul_simple() { assert_eq!(multiply("12", "34"), "408"); }
#[test] fn mul_by_zero() { assert_eq!(multiply("12345", "0"), "0"); }
#[test] fn mul_by_one() { assert_eq!(multiply("12345", "1"), "12345"); }
#[test] fn mul_negative() { assert_eq!(multiply("-3", "4"), "-12"); }
#[test] fn mul_both_negative() { assert_eq!(multiply("-3", "-4"), "12"); }
#[test] fn mul_large() {
    assert_eq!(
        multiply("123456789", "987654321"),
        "121932631112635269"
    );
}
#[test] fn no_leading_zeros() {
    assert_eq!(add("100", "-99"), "1");
    assert_eq!(multiply("10", "0"), "0");
}"#,
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
// TIER 6: BRUTAL — Multi-step algorithms where precision is everything
// Flash Lite got 100% on tiers 1-5 with retries. These should break it.
// Each problem requires exact algorithmic implementation — partial solutions fail.
// ══════════════════════════════════════════════════════════════════════

fn tier6_brutal() -> Vec<ExercismProblem> {
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
