use super::problem;
use crate::benchmark::BenchmarkProblem;

/// Extended Tier 1: Generation — more Rust coding problems with increasing complexity.
pub(super) fn tier1_ext() -> Vec<BenchmarkProblem> {
    vec![
        problem(
            "opus-matrix-ops",
            "tier1",
            "Implement a simple matrix type supporting addition, scalar multiplication, and transpose.",
            r#"pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Vec<f64>>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize) -> Self { todo!() }
    pub fn from_vec(data: Vec<Vec<f64>>) -> Self { todo!() }
    pub fn get(&self, r: usize, c: usize) -> f64 { todo!() }
    pub fn set(&mut self, r: usize, c: usize, val: f64) { todo!() }
    pub fn add(&self, other: &Matrix) -> Option<Matrix> { todo!() }
    pub fn scale(&self, s: f64) -> Matrix { todo!() }
    pub fn transpose(&self) -> Matrix { todo!() }
}"#,
            r#"use opus_matrix_ops::*;

#[test]
fn new_matrix() {
    let m = Matrix::new(2, 3);
    assert_eq!(m.rows, 2);
    assert_eq!(m.cols, 3);
    assert_eq!(m.get(0, 0), 0.0);
}

#[test]
fn set_and_get() {
    let mut m = Matrix::new(2, 2);
    m.set(0, 1, 5.0);
    assert_eq!(m.get(0, 1), 5.0);
}

#[test]
fn add_matrices() {
    let a = Matrix::from_vec(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    let b = Matrix::from_vec(vec![vec![5.0, 6.0], vec![7.0, 8.0]]);
    let c = a.add(&b).unwrap();
    assert_eq!(c.get(0, 0), 6.0);
    assert_eq!(c.get(1, 1), 12.0);
}

#[test]
fn add_mismatched_returns_none() {
    let a = Matrix::new(2, 2);
    let b = Matrix::new(3, 2);
    assert!(a.add(&b).is_none());
}

#[test]
fn scale_matrix() {
    let m = Matrix::from_vec(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    let s = m.scale(3.0);
    assert_eq!(s.get(0, 1), 6.0);
    assert_eq!(s.get(1, 0), 9.0);
}

#[test]
fn transpose() {
    let m = Matrix::from_vec(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
    let t = m.transpose();
    assert_eq!(t.rows, 3);
    assert_eq!(t.cols, 2);
    assert_eq!(t.get(0, 0), 1.0);
    assert_eq!(t.get(2, 1), 6.0);
}"#,
        ),
        problem(
            "opus-run-length",
            "tier1",
            "Implement run-length encoding and decoding. \
             encode('AABBBCCCD') -> '2A3B3C1D'. \
             decode('2A3B') -> 'AABBB'.",
            r#"pub fn encode(s: &str) -> String { todo!() }
pub fn decode(s: &str) -> String { todo!() }"#,
            r#"use opus_run_length::*;

#[test]
fn encode_basic() { assert_eq!(encode("AABBBCCCD"), "2A3B3C1D"); }
#[test]
fn encode_single() { assert_eq!(encode("A"), "1A"); }
#[test]
fn encode_empty() { assert_eq!(encode(""), ""); }
#[test]
fn encode_all_same() { assert_eq!(encode("AAAA"), "4A"); }
#[test]
fn encode_all_different() { assert_eq!(encode("ABCD"), "1A1B1C1D"); }
#[test]
fn decode_basic() { assert_eq!(decode("2A3B3C1D"), "AABBBCCCD"); }
#[test]
fn decode_single() { assert_eq!(decode("1A"), "A"); }
#[test]
fn decode_empty() { assert_eq!(decode(""), ""); }
#[test]
fn roundtrip() { assert_eq!(decode(&encode("AABBBCCCD")), "AABBBCCCD"); }
#[test]
fn decode_large_count() { assert_eq!(decode("10A"), "AAAAAAAAAA"); }"#,
        ),
        problem(
            "opus-fraction",
            "tier1",
            "Implement a Fraction type supporting addition, multiplication, and simplification. \
             Fractions should always be stored in lowest terms.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub struct Fraction {
    pub num: i64,
    pub den: i64,
}

impl Fraction {
    pub fn new(num: i64, den: i64) -> Self { todo!() }
    pub fn add(&self, other: &Fraction) -> Fraction { todo!() }
    pub fn mul(&self, other: &Fraction) -> Fraction { todo!() }
    pub fn to_f64(&self) -> f64 { todo!() }
}

impl std::fmt::Display for Fraction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { todo!() }
}"#,
            r#"use opus_fraction::*;

#[test]
fn new_simplifies() { assert_eq!(Fraction::new(2, 4), Fraction::new(1, 2)); }
#[test]
fn new_negative_den() { assert_eq!(Fraction::new(1, -2), Fraction::new(-1, 2)); }
#[test]
fn add_fractions() {
    assert_eq!(Fraction::new(1, 3).add(&Fraction::new(1, 6)), Fraction::new(1, 2));
}
#[test]
fn mul_fractions() {
    assert_eq!(Fraction::new(2, 3).mul(&Fraction::new(3, 4)), Fraction::new(1, 2));
}
#[test]
fn display() { assert_eq!(format!("{}", Fraction::new(3, 4)), "3/4"); }
#[test]
fn display_whole() { assert_eq!(format!("{}", Fraction::new(6, 3)), "2/1"); }
#[test]
fn to_f64() { assert!((Fraction::new(1, 3).to_f64() - 0.3333333).abs() < 0.001); }
#[test]
fn zero() { assert_eq!(Fraction::new(0, 5), Fraction::new(0, 1)); }"#,
        ),
        problem(
            "opus-trie",
            "tier1",
            "Implement a trie (prefix tree) for string storage with insert, search, and starts_with.",
            r#"pub struct Trie { /* your fields */ }

impl Trie {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, word: &str) { todo!() }
    pub fn search(&self, word: &str) -> bool { todo!() }
    pub fn starts_with(&self, prefix: &str) -> bool { todo!() }
    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> { todo!() }
}"#,
            r#"use opus_trie::*;

#[test]
fn insert_and_search() {
    let mut t = Trie::new();
    t.insert("hello");
    assert!(t.search("hello"));
    assert!(!t.search("hell"));
}
#[test]
fn starts_with() {
    let mut t = Trie::new();
    t.insert("hello");
    t.insert("help");
    assert!(t.starts_with("hel"));
    assert!(!t.starts_with("hex"));
}
#[test]
fn words_with_prefix() {
    let mut t = Trie::new();
    t.insert("cat");
    t.insert("car");
    t.insert("card");
    t.insert("dog");
    let mut words = t.words_with_prefix("car");
    words.sort();
    assert_eq!(words, vec!["car", "card"]);
}
#[test]
fn empty_search() {
    let t = Trie::new();
    assert!(!t.search("anything"));
}
#[test]
fn empty_prefix() {
    let mut t = Trie::new();
    t.insert("a");
    t.insert("b");
    let mut all = t.words_with_prefix("");
    all.sort();
    assert_eq!(all, vec!["a", "b"]);
}"#,
        ),
        problem(
            "opus-lru-cache",
            "tier1",
            "Implement an LRU (Least Recently Used) cache with get and put operations. \
             When the cache is full, evict the least recently used entry.",
            r#"pub struct LruCache<V> { /* your fields */ }

impl<V: Clone> LruCache<V> {
    pub fn new(capacity: usize) -> Self { todo!() }
    pub fn get(&mut self, key: &str) -> Option<V> { todo!() }
    pub fn put(&mut self, key: String, value: V) { todo!() }
    pub fn len(&self) -> usize { todo!() }
}"#,
            r#"use opus_lru_cache::*;

#[test]
fn basic_put_get() {
    let mut c = LruCache::new(2);
    c.put("a".into(), 1);
    assert_eq!(c.get("a"), Some(1));
}
#[test]
fn eviction() {
    let mut c = LruCache::new(2);
    c.put("a".into(), 1);
    c.put("b".into(), 2);
    c.put("c".into(), 3);
    assert_eq!(c.get("a"), None);
    assert_eq!(c.get("b"), Some(2));
    assert_eq!(c.get("c"), Some(3));
}
#[test]
fn access_refreshes() {
    let mut c = LruCache::new(2);
    c.put("a".into(), 1);
    c.put("b".into(), 2);
    c.get("a"); // refresh a
    c.put("c".into(), 3); // should evict b, not a
    assert_eq!(c.get("a"), Some(1));
    assert_eq!(c.get("b"), None);
}
#[test]
fn overwrite() {
    let mut c = LruCache::new(2);
    c.put("a".into(), 1);
    c.put("a".into(), 10);
    assert_eq!(c.get("a"), Some(10));
    assert_eq!(c.len(), 1);
}
#[test]
fn len() {
    let mut c = LruCache::<i32>::new(3);
    assert_eq!(c.len(), 0);
    c.put("a".into(), 1);
    assert_eq!(c.len(), 1);
}"#,
        ),
        problem(
            "opus-roman-numerals",
            "tier1",
            "Convert between integers and Roman numeral strings. Handle 1-3999.",
            r#"pub fn to_roman(n: u32) -> Option<String> { todo!() }
pub fn from_roman(s: &str) -> Option<u32> { todo!() }"#,
            r#"use opus_roman_numerals::*;

#[test]
fn basic() { assert_eq!(to_roman(1), Some("I".into())); }
#[test]
fn four() { assert_eq!(to_roman(4), Some("IV".into())); }
#[test]
fn nine() { assert_eq!(to_roman(9), Some("IX".into())); }
#[test]
fn forty_two() { assert_eq!(to_roman(42), Some("XLII".into())); }
#[test]
fn thousand() { assert_eq!(to_roman(1000), Some("M".into())); }
#[test]
fn max() { assert_eq!(to_roman(3999), Some("MMMCMXCIX".into())); }
#[test]
fn zero_invalid() { assert_eq!(to_roman(0), None); }
#[test]
fn over_max() { assert_eq!(to_roman(4000), None); }
#[test]
fn from_basic() { assert_eq!(from_roman("XIV"), Some(14)); }
#[test]
fn from_complex() { assert_eq!(from_roman("MCMXCIX"), Some(1999)); }
#[test]
fn roundtrip() { assert_eq!(from_roman(&to_roman(2024).unwrap()), Some(2024)); }
#[test]
fn from_invalid() { assert_eq!(from_roman("IIII"), None); }"#,
        ),
        problem(
            "opus-tokenizer",
            "tier1",
            "Implement a simple expression tokenizer that splits a math expression string \
             into tokens: Number(f64), Plus, Minus, Star, Slash, LParen, RParen.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> { todo!() }"#,
            r#"use opus_tokenizer::*;

#[test]
fn simple_add() {
    assert_eq!(tokenize("1+2").unwrap(), vec![Token::Number(1.0), Token::Plus, Token::Number(2.0)]);
}
#[test]
fn with_spaces() {
    assert_eq!(tokenize("3 * 4").unwrap(), vec![Token::Number(3.0), Token::Star, Token::Number(4.0)]);
}
#[test]
fn parens() {
    let t = tokenize("(1+2)*3").unwrap();
    assert_eq!(t[0], Token::LParen);
    assert_eq!(t[4], Token::RParen);
    assert_eq!(t[5], Token::Star);
}
#[test]
fn float_number() {
    assert_eq!(tokenize("3.14").unwrap(), vec![Token::Number(3.14)]);
}
#[test]
fn negative() {
    let t = tokenize("-5").unwrap();
    assert_eq!(t, vec![Token::Minus, Token::Number(5.0)]);
}
#[test]
fn complex() {
    let t = tokenize("(10.5 + 2) / 3 - 1").unwrap();
    assert_eq!(t.len(), 9);
}
#[test]
fn empty() { assert_eq!(tokenize("").unwrap(), vec![]); }
#[test]
fn invalid_char() { assert!(tokenize("1 @ 2").is_err()); }"#,
        ),
        problem(
            "opus-csv-parser",
            "tier1",
            "Parse CSV data into a vector of hashmaps. First row is headers. Handle quoted fields with commas inside.",
            r#"use std::collections::HashMap;
pub fn parse_csv(input: &str) -> Vec<HashMap<String, String>> { todo!() }"#,
            r#"use opus_csv_parser::*;

#[test]
fn basic() {
    let csv = "name,age\nAlice,30\nBob,25";
    let rows = parse_csv(csv);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "Alice");
    assert_eq!(rows[1]["age"], "25");
}
#[test]
fn quoted_field() {
    let csv = "name,bio\nAlice,\"likes cats, dogs\"";
    let rows = parse_csv(csv);
    assert_eq!(rows[0]["bio"], "likes cats, dogs");
}
#[test]
fn empty_input() { assert_eq!(parse_csv("").len(), 0); }
#[test]
fn headers_only() { assert_eq!(parse_csv("a,b,c").len(), 0); }
#[test]
fn single_column() {
    let rows = parse_csv("x\n1\n2\n3");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2]["x"], "3");
}"#,
        ),
        problem(
            "opus-median-stream",
            "tier1",
            "Implement a data structure that efficiently computes the running median as numbers are added.",
            r#"pub struct MedianFinder { /* your fields */ }

impl MedianFinder {
    pub fn new() -> Self { todo!() }
    pub fn add(&mut self, num: f64) { todo!() }
    pub fn median(&self) -> Option<f64> { todo!() }
    pub fn count(&self) -> usize { todo!() }
}"#,
            r#"use opus_median_stream::*;

#[test]
fn empty() { assert_eq!(MedianFinder::new().median(), None); }
#[test]
fn single() {
    let mut m = MedianFinder::new();
    m.add(5.0);
    assert_eq!(m.median(), Some(5.0));
}
#[test]
fn odd_count() {
    let mut m = MedianFinder::new();
    for x in [3.0, 1.0, 2.0] { m.add(x); }
    assert_eq!(m.median(), Some(2.0));
}
#[test]
fn even_count() {
    let mut m = MedianFinder::new();
    for x in [1.0, 2.0, 3.0, 4.0] { m.add(x); }
    assert_eq!(m.median(), Some(2.5));
}
#[test]
fn large_sequence() {
    let mut m = MedianFinder::new();
    for i in 1..=100 { m.add(i as f64); }
    assert_eq!(m.median(), Some(50.5));
    assert_eq!(m.count(), 100);
}"#,
        ),
        problem(
            "opus-snake-case",
            "tier1",
            "Convert strings between camelCase, PascalCase, snake_case, and SCREAMING_SNAKE_CASE.",
            r#"pub fn to_snake_case(s: &str) -> String { todo!() }
pub fn to_camel_case(s: &str) -> String { todo!() }
pub fn to_pascal_case(s: &str) -> String { todo!() }
pub fn to_screaming_snake(s: &str) -> String { todo!() }"#,
            r#"use opus_snake_case::*;

#[test]
fn camel_to_snake() { assert_eq!(to_snake_case("helloWorld"), "hello_world"); }
#[test]
fn pascal_to_snake() { assert_eq!(to_snake_case("HelloWorld"), "hello_world"); }
#[test]
fn snake_to_camel() { assert_eq!(to_camel_case("hello_world"), "helloWorld"); }
#[test]
fn snake_to_pascal() { assert_eq!(to_pascal_case("hello_world"), "HelloWorld"); }
#[test]
fn screaming() { assert_eq!(to_screaming_snake("helloWorld"), "HELLO_WORLD"); }
#[test]
fn already_snake() { assert_eq!(to_snake_case("already_snake"), "already_snake"); }
#[test]
fn single_word() { assert_eq!(to_snake_case("hello"), "hello"); }
#[test]
fn consecutive_caps() { assert_eq!(to_snake_case("parseHTTPResponse"), "parse_http_response"); }
#[test]
fn empty() { assert_eq!(to_snake_case(""), ""); }"#,
        ),
        problem(
            "opus-graph-bfs",
            "tier1",
            "Implement an undirected graph with BFS shortest path.",
            r#"use std::collections::HashMap;

pub struct Graph {
    pub adj: HashMap<String, Vec<String>>,
}

impl Graph {
    pub fn new() -> Self { todo!() }
    pub fn add_edge(&mut self, a: &str, b: &str) { todo!() }
    pub fn bfs_path(&self, start: &str, end: &str) -> Option<Vec<String>> { todo!() }
    pub fn has_node(&self, node: &str) -> bool { todo!() }
}"#,
            r#"use opus_graph_bfs::*;

#[test]
fn direct_edge() {
    let mut g = Graph::new();
    g.add_edge("a", "b");
    let path = g.bfs_path("a", "b").unwrap();
    assert_eq!(path, vec!["a", "b"]);
}
#[test]
fn two_hop() {
    let mut g = Graph::new();
    g.add_edge("a", "b");
    g.add_edge("b", "c");
    let path = g.bfs_path("a", "c").unwrap();
    assert_eq!(path.len(), 3);
}
#[test]
fn no_path() {
    let mut g = Graph::new();
    g.add_edge("a", "b");
    g.add_edge("c", "d");
    assert!(g.bfs_path("a", "d").is_none());
}
#[test]
fn same_node() {
    let mut g = Graph::new();
    g.add_edge("a", "b");
    let path = g.bfs_path("a", "a").unwrap();
    assert_eq!(path, vec!["a"]);
}
#[test]
fn shortest_path() {
    let mut g = Graph::new();
    g.add_edge("a", "b");
    g.add_edge("b", "d");
    g.add_edge("a", "c");
    g.add_edge("c", "d");
    let path = g.bfs_path("a", "d").unwrap();
    assert_eq!(path.len(), 3); // a->b->d or a->c->d, both length 3
}"#,
        ),
        problem(
            "opus-priority-queue",
            "tier1",
            "Implement a min-heap priority queue.",
            r#"pub struct MinHeap<T> { /* your fields */ }

impl<T: Ord> MinHeap<T> {
    pub fn new() -> Self { todo!() }
    pub fn push(&mut self, item: T) { todo!() }
    pub fn pop(&mut self) -> Option<T> { todo!() }
    pub fn peek(&self) -> Option<&T> { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
}"#,
            r#"use opus_priority_queue::*;

#[test]
fn basic() {
    let mut h = MinHeap::new();
    h.push(3);
    h.push(1);
    h.push(2);
    assert_eq!(h.pop(), Some(1));
    assert_eq!(h.pop(), Some(2));
    assert_eq!(h.pop(), Some(3));
}
#[test]
fn peek() {
    let mut h = MinHeap::new();
    h.push(5);
    h.push(3);
    assert_eq!(h.peek(), Some(&3));
    assert_eq!(h.len(), 2);
}
#[test]
fn empty() {
    let mut h = MinHeap::<i32>::new();
    assert!(h.is_empty());
    assert_eq!(h.pop(), None);
    assert_eq!(h.peek(), None);
}
#[test]
fn duplicates() {
    let mut h = MinHeap::new();
    h.push(2);
    h.push(2);
    h.push(1);
    assert_eq!(h.pop(), Some(1));
    assert_eq!(h.pop(), Some(2));
    assert_eq!(h.pop(), Some(2));
}
#[test]
fn large() {
    let mut h = MinHeap::new();
    for i in (0..100).rev() { h.push(i); }
    for i in 0..100 { assert_eq!(h.pop(), Some(i)); }
}"#,
        ),
        problem(
            "opus-json-value",
            "tier1",
            "Implement a simple JSON value type that can represent null, bool, number, string, array, and object. \
             Implement Display to produce valid JSON output.",
            r#"use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl std::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { todo!() }
}"#,
            r#"use opus_json_value::*;
use std::collections::BTreeMap;

#[test]
fn null() { assert_eq!(format!("{}", JsonValue::Null), "null"); }
#[test]
fn bool_true() { assert_eq!(format!("{}", JsonValue::Bool(true)), "true"); }
#[test]
fn number() { assert_eq!(format!("{}", JsonValue::Number(42.0)), "42"); }
#[test]
fn float_number() { assert_eq!(format!("{}", JsonValue::Number(3.14)), "3.14"); }
#[test]
fn string() { assert_eq!(format!("{}", JsonValue::Str("hello".into())), "\"hello\""); }
#[test]
fn empty_array() { assert_eq!(format!("{}", JsonValue::Array(vec![])), "[]"); }
#[test]
fn array() {
    let a = JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Bool(false)]);
    assert_eq!(format!("{}", a), "[1,false]");
}
#[test]
fn object() {
    let mut m = BTreeMap::new();
    m.insert("a".into(), JsonValue::Number(1.0));
    assert_eq!(format!("{}", JsonValue::Object(m)), "{\"a\":1}");
}
#[test]
fn nested() {
    let inner = JsonValue::Array(vec![JsonValue::Null]);
    let mut m = BTreeMap::new();
    m.insert("x".into(), inner);
    assert_eq!(format!("{}", JsonValue::Object(m)), "{\"x\":[null]}");
}"#,
        ),
    ]
}
