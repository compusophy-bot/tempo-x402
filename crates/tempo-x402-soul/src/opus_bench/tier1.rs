use super::problem;
use crate::benchmark::BenchmarkProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 1: GENERATION — Multi-constraint Rust coding
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier1_generation() -> Vec<BenchmarkProblem> {
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
