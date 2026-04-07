use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier1_ext7() -> Vec<BenchmarkProblem> {
    vec![
        // 1. Caesar cipher — character rotation
        problem("opus-caesar-shift", "tier1",
            "Implement a Caesar cipher. Shift each letter by `n` positions (wrapping). Preserve case. Non-alpha characters unchanged.",
            r#"pub fn encrypt(text: &str, shift: i32) -> String { todo!() }
pub fn decrypt(text: &str, shift: i32) -> String { todo!() }"#,
            r#"use opus_caesar_shift::*;
#[test] fn basic() { assert_eq!(encrypt("abc", 3), "def"); }
#[test] fn wrap() { assert_eq!(encrypt("xyz", 3), "abc"); }
#[test] fn upper() { assert_eq!(encrypt("Hello", 13), "Uryyb"); }
#[test] fn non_alpha() { assert_eq!(encrypt("a-b-c!", 1), "b-c-d!"); }
#[test] fn roundtrip() { assert_eq!(decrypt(&encrypt("Hello, World!", 7), 7), "Hello, World!"); }
#[test] fn negative() { assert_eq!(encrypt("def", -3), "abc"); }
"#),

        // 2. Matrix transpose
        problem("opus-matrix-transpose", "tier1",
            "Transpose a 2D matrix (swap rows and columns). An empty matrix returns empty. All rows have equal length.",
            r#"pub fn transpose(matrix: &[Vec<i32>]) -> Vec<Vec<i32>> { todo!() }"#,
            r#"use opus_matrix_transpose::*;
#[test] fn square() { assert_eq!(transpose(&[vec![1,2],vec![3,4]]), vec![vec![1,3],vec![2,4]]); }
#[test] fn rect() { assert_eq!(transpose(&[vec![1,2,3]]), vec![vec![1],vec![2],vec![3]]); }
#[test] fn empty() { let e: Vec<Vec<i32>> = vec![]; assert_eq!(transpose(&e), e); }
#[test] fn single() { assert_eq!(transpose(&[vec![42]]), vec![vec![42]]); }
#[test] fn three_by_two() { assert_eq!(transpose(&[vec![1,2],vec![3,4],vec![5,6]]), vec![vec![1,3,5],vec![2,4,6]]); }
"#),

        // 3. Run-length encoding
        problem("opus-rle-compress", "tier1",
            "Run-length encode a string: consecutive identical chars compressed to count+char. E.g. \"AAABBC\" -> \"3A2B1C\". Decode reverses it.",
            r#"pub fn encode(input: &str) -> String { todo!() }
pub fn decode(input: &str) -> String { todo!() }"#,
            r#"use opus_rle_compress::*;
#[test] fn basic_encode() { assert_eq!(encode("AAABBC"), "3A2B1C"); }
#[test] fn single_chars() { assert_eq!(encode("ABCD"), "1A1B1C1D"); }
#[test] fn repeated() { assert_eq!(encode("aaaaaa"), "6a"); }
#[test] fn empty_str() { assert_eq!(encode(""), ""); }
#[test] fn roundtrip() { let s = "AAABBCCCCDD"; assert_eq!(decode(&encode(s)), s); }
#[test] fn decode_basic() { assert_eq!(decode("3A2B1C"), "AAABBC"); }
"#),

        // 4. Balanced parentheses
        problem("opus-balanced-parens", "tier1",
            "Check if a string has balanced parentheses, brackets, and braces. Only check ()[]{}. Other characters are ignored.",
            r#"pub fn is_balanced(input: &str) -> bool { todo!() }"#,
            r#"use opus_balanced_parens::*;
#[test] fn empty() { assert!(is_balanced("")); }
#[test] fn simple() { assert!(is_balanced("()")); }
#[test] fn nested() { assert!(is_balanced("{[()]}")); }
#[test] fn unbalanced() { assert!(!is_balanced("([)]")); }
#[test] fn unclosed() { assert!(!is_balanced("(((")); }
#[test] fn with_text() { assert!(is_balanced("fn main() { let v = vec![1, 2]; }")); }
#[test] fn extra_close() { assert!(!is_balanced("())")); }
"#),

        // 5. Roman to integer
        problem("opus-roman-decode", "tier1",
            "Convert a Roman numeral string to an integer. Handle subtractive notation (IV=4, IX=9, XL=40, XC=90, CD=400, CM=900). Input is always valid.",
            r#"pub fn roman_to_int(s: &str) -> u32 { todo!() }"#,
            r#"use opus_roman_decode::*;
#[test] fn three() { assert_eq!(roman_to_int("III"), 3); }
#[test] fn four() { assert_eq!(roman_to_int("IV"), 4); }
#[test] fn nine() { assert_eq!(roman_to_int("IX"), 9); }
#[test] fn complex() { assert_eq!(roman_to_int("MCMXCIV"), 1994); }
#[test] fn thousand() { assert_eq!(roman_to_int("M"), 1000); }
#[test] fn fifty_eight() { assert_eq!(roman_to_int("LVIII"), 58); }
"#),

        // 6. Integer to Roman
        problem("opus-roman-encode", "tier1",
            "Convert an integer (1..=3999) to a Roman numeral string. Use subtractive notation where standard.",
            r#"pub fn int_to_roman(num: u32) -> String { todo!() }"#,
            r#"use opus_roman_encode::*;
#[test] fn three() { assert_eq!(int_to_roman(3), "III"); }
#[test] fn four() { assert_eq!(int_to_roman(4), "IV"); }
#[test] fn nine() { assert_eq!(int_to_roman(9), "IX"); }
#[test] fn complex() { assert_eq!(int_to_roman(1994), "MCMXCIV"); }
#[test] fn max() { assert_eq!(int_to_roman(3999), "MMMCMXCIX"); }
#[test] fn fifty_eight() { assert_eq!(int_to_roman(58), "LVIII"); }
"#),

        // 7. Merge overlapping intervals
        problem("opus-merge-intervals", "tier1",
            "Given a list of intervals [start, end], merge all overlapping intervals and return sorted non-overlapping intervals.",
            r#"pub fn merge(intervals: Vec<(i32, i32)>) -> Vec<(i32, i32)> { todo!() }"#,
            r#"use opus_merge_intervals::*;
#[test] fn basic() { assert_eq!(merge(vec![(1,3),(2,6),(8,10),(15,18)]), vec![(1,6),(8,10),(15,18)]); }
#[test] fn all_overlap() { assert_eq!(merge(vec![(1,4),(2,5),(3,6)]), vec![(1,6)]); }
#[test] fn no_overlap() { assert_eq!(merge(vec![(1,2),(3,4),(5,6)]), vec![(1,2),(3,4),(5,6)]); }
#[test] fn single() { assert_eq!(merge(vec![(1,5)]), vec![(1,5)]); }
#[test] fn empty() { assert_eq!(merge(vec![]), vec![]); }
#[test] fn unsorted() { assert_eq!(merge(vec![(5,6),(1,3),(2,4)]), vec![(1,4),(5,6)]); }
"#),

        // 8. Spiral order matrix generation
        problem("opus-spiral-gen", "tier1",
            "Generate an n x n matrix filled with values 1..=n*n in spiral order (clockwise from top-left).",
            r#"pub fn spiral_matrix(n: usize) -> Vec<Vec<u32>> { todo!() }"#,
            r#"use opus_spiral_gen::*;
#[test] fn one() { assert_eq!(spiral_matrix(1), vec![vec![1]]); }
#[test] fn two() { assert_eq!(spiral_matrix(2), vec![vec![1,2],vec![4,3]]); }
#[test] fn three() { assert_eq!(spiral_matrix(3), vec![vec![1,2,3],vec![8,9,4],vec![7,6,5]]); }
#[test] fn zero() { let e: Vec<Vec<u32>> = vec![]; assert_eq!(spiral_matrix(0), e); }
#[test] fn four_corners() { let m = spiral_matrix(4); assert_eq!(m[0][0], 1); assert_eq!(m[0][3], 4); assert_eq!(m[3][3], 8); assert_eq!(m[3][0], 11); }
"#),

        // 9. Word frequency count
        problem("opus-word-frequency", "tier1",
            "Count word frequencies in text. Words are separated by whitespace. Convert to lowercase. Return a Vec of (word, count) sorted by count descending, then alphabetically.",
            r#"pub fn word_freq(text: &str) -> Vec<(String, usize)> { todo!() }"#,
            r#"use opus_word_frequency::*;
#[test] fn basic() { let r = word_freq("the cat sat on the mat"); assert_eq!(r[0], ("the".into(), 2)); }
#[test] fn empty() { assert_eq!(word_freq(""), vec![]); }
#[test] fn case() { let r = word_freq("Go go GO"); assert_eq!(r, vec![("go".into(), 3)]); }
#[test] fn alpha_sort() { let r = word_freq("b a c"); assert_eq!(r, vec![("a".into(),1),("b".into(),1),("c".into(),1)]); }
#[test] fn single_word() { let r = word_freq("hello"); assert_eq!(r, vec![("hello".into(), 1)]); }
"#),

        // 10. Zigzag cipher
        problem("opus-zigzag-cipher", "tier1",
            "Encode a string in zigzag (rail fence) pattern with `n` rails and return the string read row by row. Also decode.",
            r#"pub fn zigzag_encode(s: &str, rails: usize) -> String { todo!() }
pub fn zigzag_decode(s: &str, rails: usize) -> String { todo!() }"#,
            r#"use opus_zigzag_cipher::*;
#[test] fn encode_basic() { assert_eq!(zigzag_encode("HELLO WORLD", 3), "HOREL WLOLD"); }
#[test] fn encode_two() { assert_eq!(zigzag_encode("ABCDEF", 2), "ACEBDF"); }
#[test] fn roundtrip() { assert_eq!(zigzag_decode(&zigzag_encode("Test message!", 4), 4), "Test message!"); }
#[test] fn one_rail() { assert_eq!(zigzag_encode("ABC", 1), "ABC"); }
#[test] fn rails_gt_len() { assert_eq!(zigzag_encode("AB", 5), "AB"); }
"#),

        // 11. T9 phone keypad letter combinations
        problem("opus-t9-combos", "tier1",
            "Given a string of digits 2-9, return all possible letter combinations that the number could represent on a T9 phone keypad. 2=abc, 3=def, 4=ghi, 5=jkl, 6=mno, 7=pqrs, 8=tuv, 9=wxyz. Return sorted.",
            r#"pub fn letter_combinations(digits: &str) -> Vec<String> { todo!() }"#,
            r#"use opus_t9_combos::*;
#[test] fn two_three() { let r = letter_combinations("23"); assert_eq!(r, vec!["ad","ae","af","bd","be","bf","cd","ce","cf"]); }
#[test] fn empty() { assert_eq!(letter_combinations(""), Vec::<String>::new()); }
#[test] fn single() { assert_eq!(letter_combinations("2"), vec!["a","b","c"]); }
#[test] fn seven() { assert_eq!(letter_combinations("7"), vec!["p","q","r","s"]); }
#[test] fn count() { assert_eq!(letter_combinations("234").len(), 27); }
"#),

        // 12. Validate sudoku
        problem("opus-valid-sudoku", "tier1",
            "Validate a partially filled 9x9 sudoku board. Board is Vec<Vec<u8>> where 0 means empty. Check that no row, column, or 3x3 box has duplicate non-zero values.",
            r#"pub fn is_valid_sudoku(board: &[Vec<u8>]) -> bool { todo!() }"#,
            r#"use opus_valid_sudoku::*;
#[test] fn valid() {
    let b = vec![
        vec![5,3,0,0,7,0,0,0,0], vec![6,0,0,1,9,5,0,0,0], vec![0,9,8,0,0,0,0,6,0],
        vec![8,0,0,0,6,0,0,0,3], vec![4,0,0,8,0,3,0,0,1], vec![7,0,0,0,2,0,0,0,6],
        vec![0,6,0,0,0,0,2,8,0], vec![0,0,0,4,1,9,0,0,5], vec![0,0,0,0,8,0,0,7,9],
    ];
    assert!(is_valid_sudoku(&b));
}
#[test] fn dup_row() {
    let mut b = vec![vec![0u8;9];9]; b[0][0] = 1; b[0][1] = 1;
    assert!(!is_valid_sudoku(&b));
}
#[test] fn dup_col() {
    let mut b = vec![vec![0u8;9];9]; b[0][0] = 1; b[1][0] = 1;
    assert!(!is_valid_sudoku(&b));
}
#[test] fn dup_box() {
    let mut b = vec![vec![0u8;9];9]; b[0][0] = 1; b[1][1] = 1;
    assert!(!is_valid_sudoku(&b));
}
#[test] fn all_empty() { assert!(is_valid_sudoku(&vec![vec![0u8;9];9])); }
"#),

        // 13. Deep flatten nested structures
        problem("opus-deep-flatten", "tier1",
            "Implement a NestedList enum (Elem(i64) or List(Vec<NestedList>)) and flatten it to Vec<i64> in order.",
            r#"#[derive(Debug, Clone)]
pub enum NestedList {
    Elem(i64),
    List(Vec<NestedList>),
}
pub fn flatten(nested: &NestedList) -> Vec<i64> { todo!() }"#,
            r#"use opus_deep_flatten::*;
#[test] fn flat() { assert_eq!(flatten(&NestedList::Elem(1)), vec![1]); }
#[test] fn one_level() { assert_eq!(flatten(&NestedList::List(vec![NestedList::Elem(1), NestedList::Elem(2)])), vec![1,2]); }
#[test] fn deep() {
    let n = NestedList::List(vec![
        NestedList::Elem(1),
        NestedList::List(vec![NestedList::Elem(2), NestedList::List(vec![NestedList::Elem(3)])]),
        NestedList::Elem(4),
    ]);
    assert_eq!(flatten(&n), vec![1,2,3,4]);
}
#[test] fn empty_list() { assert_eq!(flatten(&NestedList::List(vec![])), vec![]); }
#[test] fn nested_empty() {
    let n = NestedList::List(vec![NestedList::List(vec![]), NestedList::Elem(5)]);
    assert_eq!(flatten(&n), vec![5]);
}
"#),

        // 14. LRU cache
        problem("opus-lru-map", "tier1",
            "Implement an LRU cache with capacity. get(key) returns Option<i32> and marks as recently used. put(key, value) inserts/updates. Evict least recently used when over capacity.",
            r#"pub struct LruCache { /* fields */ }
impl LruCache {
    pub fn new(capacity: usize) -> Self { todo!() }
    pub fn get(&mut self, key: i32) -> Option<i32> { todo!() }
    pub fn put(&mut self, key: i32, value: i32) { todo!() }
}"#,
            r#"use opus_lru_map::*;
#[test] fn basic() {
    let mut c = LruCache::new(2);
    c.put(1, 10); c.put(2, 20);
    assert_eq!(c.get(1), Some(10));
    c.put(3, 30); // evicts 2
    assert_eq!(c.get(2), None);
    assert_eq!(c.get(3), Some(30));
}
#[test] fn update() {
    let mut c = LruCache::new(2);
    c.put(1, 10); c.put(1, 100);
    assert_eq!(c.get(1), Some(100));
}
#[test] fn evict_order() {
    let mut c = LruCache::new(2);
    c.put(1, 1); c.put(2, 2);
    c.get(1); // 1 is now most recent
    c.put(3, 3); // evicts 2
    assert_eq!(c.get(2), None);
    assert_eq!(c.get(1), Some(1));
}
#[test] fn miss() { let mut c = LruCache::new(1); assert_eq!(c.get(99), None); }
"#),

        // 15. Prefix trie
        problem("opus-prefix-trie", "tier1",
            "Implement a trie (prefix tree) with insert, search (exact), and starts_with (prefix match).",
            r#"pub struct Trie { /* fields */ }
impl Trie {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, word: &str) { todo!() }
    pub fn search(&self, word: &str) -> bool { todo!() }
    pub fn starts_with(&self, prefix: &str) -> bool { todo!() }
}"#,
            r#"use opus_prefix_trie::*;
#[test] fn insert_search() { let mut t = Trie::new(); t.insert("hello"); assert!(t.search("hello")); }
#[test] fn not_found() { let t = Trie::new(); assert!(!t.search("hello")); }
#[test] fn prefix() { let mut t = Trie::new(); t.insert("hello"); assert!(t.starts_with("hel")); assert!(!t.starts_with("hey")); }
#[test] fn prefix_not_word() { let mut t = Trie::new(); t.insert("hello"); assert!(!t.search("hel")); }
#[test] fn multiple() { let mut t = Trie::new(); t.insert("app"); t.insert("apple"); assert!(t.search("app")); assert!(t.search("apple")); }
#[test] fn empty_prefix() { let mut t = Trie::new(); t.insert("a"); assert!(t.starts_with("")); }
"#),

        // 16. Running median
        problem("opus-running-median", "tier1",
            "Implement a MedianFinder that accepts numbers one at a time and can return the current median at any point. Use sorted insertion or two-heap approach.",
            r#"pub struct MedianFinder { /* fields */ }
impl MedianFinder {
    pub fn new() -> Self { todo!() }
    pub fn add(&mut self, num: f64) { todo!() }
    pub fn median(&self) -> Option<f64> { todo!() }
}"#,
            r#"use opus_running_median::*;
#[test] fn single() { let mut m = MedianFinder::new(); m.add(5.0); assert_eq!(m.median(), Some(5.0)); }
#[test] fn two() { let mut m = MedianFinder::new(); m.add(1.0); m.add(3.0); assert_eq!(m.median(), Some(2.0)); }
#[test] fn three() { let mut m = MedianFinder::new(); m.add(1.0); m.add(3.0); m.add(2.0); assert_eq!(m.median(), Some(2.0)); }
#[test] fn empty() { let m = MedianFinder::new(); assert_eq!(m.median(), None); }
#[test] fn sequence() {
    let mut m = MedianFinder::new();
    for x in [5.0, 2.0, 8.0, 1.0, 9.0] { m.add(x); }
    assert_eq!(m.median(), Some(5.0));
}
"#),

        // 17. Token bucket rate limiter
        problem("opus-token-bucket", "tier1",
            "Implement a token bucket rate limiter. Initialize with capacity and refill_rate (tokens per second). allow(timestamp_secs) returns true if a token is available (consuming it). Tokens refill over time up to capacity.",
            r#"pub struct TokenBucket { /* fields */ }
impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self { todo!() }
    pub fn allow(&mut self, timestamp: f64) -> bool { todo!() }
}"#,
            r#"use opus_token_bucket::*;
#[test] fn initial() { let mut tb = TokenBucket::new(3.0, 1.0); assert!(tb.allow(0.0)); assert!(tb.allow(0.0)); assert!(tb.allow(0.0)); assert!(!tb.allow(0.0)); }
#[test] fn refill() { let mut tb = TokenBucket::new(1.0, 1.0); assert!(tb.allow(0.0)); assert!(!tb.allow(0.0)); assert!(tb.allow(1.0)); }
#[test] fn cap() { let mut tb = TokenBucket::new(2.0, 1.0); assert!(tb.allow(0.0)); assert!(tb.allow(0.0)); assert!(!tb.allow(0.0)); assert!(tb.allow(10.0)); assert!(tb.allow(10.0)); assert!(!tb.allow(10.0)); }
#[test] fn partial_refill() { let mut tb = TokenBucket::new(2.0, 2.0); assert!(tb.allow(0.0)); assert!(tb.allow(0.0)); assert!(!tb.allow(0.0)); assert!(tb.allow(0.5)); }
"#),

        // 18. Simple JSON parser (subset)
        problem("opus-json-parser", "tier1",
            r#"Parse a subset of JSON: strings ("..."), integers, arrays [...], and booleans (true/false). Return a JsonValue enum. Whitespace between tokens should be ignored."#,
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Array(Vec<JsonValue>),
    Null,
}
pub fn parse_json(input: &str) -> Option<JsonValue> { todo!() }"#,
            r##"use opus_json_parser::*;
#[test] fn string() { assert_eq!(parse_json(r#""hello""#), Some(JsonValue::Str("hello".into()))); }
#[test] fn int() { assert_eq!(parse_json("42"), Some(JsonValue::Int(42))); }
#[test] fn negative() { assert_eq!(parse_json("-7"), Some(JsonValue::Int(-7))); }
#[test] fn bool_true() { assert_eq!(parse_json("true"), Some(JsonValue::Bool(true))); }
#[test] fn null() { assert_eq!(parse_json("null"), Some(JsonValue::Null)); }
#[test] fn array() { assert_eq!(parse_json("[1, 2, 3]"), Some(JsonValue::Array(vec![JsonValue::Int(1), JsonValue::Int(2), JsonValue::Int(3)]))); }
#[test] fn nested() { assert_eq!(parse_json(r#"["a", [1, true]]"#), Some(JsonValue::Array(vec![JsonValue::Str("a".into()), JsonValue::Array(vec![JsonValue::Int(1), JsonValue::Bool(true)])]))); }
#[test] fn empty_array() { assert_eq!(parse_json("[]"), Some(JsonValue::Array(vec![]))); }
"##),

        // 19. Cron expression parser
        problem("opus-cron-parser", "tier1",
            "Parse a simplified cron expression (5 fields: minute hour day_of_month month day_of_week) and check if a given time matches. Support * (any), specific numbers, and comma-separated lists. Fields: minute(0-59), hour(0-23), day(1-31), month(1-12), dow(0-6, 0=Sunday).",
            r#"pub struct CronExpr { /* fields */ }
impl CronExpr {
    pub fn parse(expr: &str) -> Option<Self> { todo!() }
    pub fn matches(&self, minute: u8, hour: u8, day: u8, month: u8, dow: u8) -> bool { todo!() }
}"#,
            r#"use opus_cron_parser::*;
#[test] fn every_minute() { let c = CronExpr::parse("* * * * *").unwrap(); assert!(c.matches(0, 0, 1, 1, 0)); assert!(c.matches(59, 23, 31, 12, 6)); }
#[test] fn specific() { let c = CronExpr::parse("30 9 * * *").unwrap(); assert!(c.matches(30, 9, 15, 6, 3)); assert!(!c.matches(31, 9, 15, 6, 3)); }
#[test] fn list() { let c = CronExpr::parse("0,30 * * * *").unwrap(); assert!(c.matches(0, 5, 1, 1, 0)); assert!(c.matches(30, 5, 1, 1, 0)); assert!(!c.matches(15, 5, 1, 1, 0)); }
#[test] fn invalid() { assert!(CronExpr::parse("* *").is_none()); }
#[test] fn dow() { let c = CronExpr::parse("* * * * 1").unwrap(); assert!(c.matches(0, 0, 1, 1, 1)); assert!(!c.matches(0, 0, 1, 1, 0)); }
"#),

        // 20. Fixed-size bitset
        problem("opus-bitfield", "tier1",
            "Implement a fixed-size bitset backed by a Vec<u64>. Support set, clear, test, count (popcount), union, and intersection. Bits are indexed from 0.",
            r#"#[derive(Clone, Debug, PartialEq)]
pub struct BitField {
    bits: Vec<u64>,
    size: usize,
}
impl BitField {
    pub fn new(size: usize) -> Self { todo!() }
    pub fn set(&mut self, idx: usize) { todo!() }
    pub fn clear(&mut self, idx: usize) { todo!() }
    pub fn test(&self, idx: usize) -> bool { todo!() }
    pub fn count(&self) -> usize { todo!() }
    pub fn union(&self, other: &Self) -> Self { todo!() }
    pub fn intersection(&self, other: &Self) -> Self { todo!() }
}"#,
            r#"use opus_bitfield::*;
#[test] fn set_test() { let mut b = BitField::new(128); b.set(0); b.set(64); assert!(b.test(0)); assert!(b.test(64)); assert!(!b.test(1)); }
#[test] fn clear_bit() { let mut b = BitField::new(64); b.set(5); b.clear(5); assert!(!b.test(5)); }
#[test] fn popcount() { let mut b = BitField::new(256); for i in (0..256).step_by(2) { b.set(i); } assert_eq!(b.count(), 128); }
#[test] fn union_op() { let mut a = BitField::new(64); let mut b = BitField::new(64); a.set(0); b.set(1); let u = a.union(&b); assert!(u.test(0)); assert!(u.test(1)); assert_eq!(u.count(), 2); }
#[test] fn intersect() { let mut a = BitField::new(64); let mut b = BitField::new(64); a.set(0); a.set(1); b.set(1); b.set(2); let i = a.intersection(&b); assert!(!i.test(0)); assert!(i.test(1)); assert!(!i.test(2)); }
#[test] fn empty() { let b = BitField::new(100); assert_eq!(b.count(), 0); }
"#),
    ]
}
