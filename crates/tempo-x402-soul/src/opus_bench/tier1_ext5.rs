use super::problem;
use crate::benchmark::ExercismProblem;

pub(super) fn tier1_ext5() -> Vec<ExercismProblem> {
    vec![
        problem("opus-rpn-calc", "tier1", "Implement a Reverse Polish Notation calculator. Supports +, -, *, /.",
            r#"pub fn rpn_eval(expr: &str) -> Result<f64, String> { todo!() }"#,
            r#"use opus_rpn_calc::*;
#[test] fn add() { assert_eq!(rpn_eval("3 4 +").unwrap(), 7.0); }
#[test] fn complex() { assert_eq!(rpn_eval("5 1 2 + 4 * + 3 -").unwrap(), 14.0); }
#[test] fn single() { assert_eq!(rpn_eval("42").unwrap(), 42.0); }
#[test] fn div() { assert_eq!(rpn_eval("10 2 /").unwrap(), 5.0); }
#[test] fn div_zero() { assert!(rpn_eval("1 0 /").is_err()); }
#[test] fn empty() { assert!(rpn_eval("").is_err()); }
#[test] fn too_few() { assert!(rpn_eval("1 +").is_err()); }"#),

        problem("opus-linked-list", "tier1", "Implement a singly linked list with push, pop, peek, reverse, and len.",
            r#"pub struct LinkedList<T> { /* fields */ }
impl<T> LinkedList<T> {
    pub fn new() -> Self { todo!() }
    pub fn push(&mut self, val: T) { todo!() }
    pub fn pop(&mut self) -> Option<T> { todo!() }
    pub fn peek(&self) -> Option<&T> { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
    pub fn reverse(&mut self) { todo!() }
    pub fn to_vec(&self) -> Vec<&T> { todo!() }
}"#,
            r#"use opus_linked_list::*;
#[test] fn basic() { let mut l = LinkedList::new(); l.push(1); l.push(2); assert_eq!(l.pop(), Some(2)); }
#[test] fn peek() { let mut l = LinkedList::new(); l.push(1); assert_eq!(l.peek(), Some(&1)); }
#[test] fn empty() { let l = LinkedList::<i32>::new(); assert!(l.is_empty()); assert_eq!(l.pop(), None); }
#[test] fn reverse() { let mut l = LinkedList::new(); l.push(1); l.push(2); l.push(3); l.reverse(); assert_eq!(l.pop(), Some(1)); }
#[test] fn len() { let mut l = LinkedList::new(); l.push(1); l.push(2); assert_eq!(l.len(), 2); }
#[test] fn to_vec() { let mut l = LinkedList::new(); l.push(3); l.push(2); l.push(1); assert_eq!(l.to_vec(), vec![&1, &2, &3]); }"#),

        problem("opus-bit-ops", "tier1", "Implement common bit manipulation operations.",
            r#"pub fn count_bits(n: u64) -> u32 { todo!() }
pub fn is_power_of_two(n: u64) -> bool { todo!() }
pub fn highest_bit(n: u64) -> u32 { todo!() }
pub fn reverse_bits(n: u32) -> u32 { todo!() }
pub fn swap_nibbles(b: u8) -> u8 { todo!() }"#,
            r#"use opus_bit_ops::*;
#[test] fn count() { assert_eq!(count_bits(7), 3); assert_eq!(count_bits(0), 0); assert_eq!(count_bits(255), 8); }
#[test] fn pow2() { assert!(is_power_of_two(8)); assert!(!is_power_of_two(6)); assert!(!is_power_of_two(0)); }
#[test] fn highest() { assert_eq!(highest_bit(8), 3); assert_eq!(highest_bit(1), 0); assert_eq!(highest_bit(255), 7); }
#[test] fn rev() { assert_eq!(reverse_bits(0b10110000_00000000_00000000_00000000), 0b00000000_00000000_00000000_00001101); }
#[test] fn nibble() { assert_eq!(swap_nibbles(0xAB), 0xBA); assert_eq!(swap_nibbles(0x12), 0x21); }"#),

        problem("opus-date-parser", "tier1", "Parse dates in multiple formats: YYYY-MM-DD, DD/MM/YYYY, Month DD YYYY.",
            r#"#[derive(Debug, PartialEq)]
pub struct Date { pub year: u32, pub month: u32, pub day: u32 }
pub fn parse_date(s: &str) -> Option<Date> { todo!() }"#,
            r#"use opus_date_parser::*;
#[test] fn iso() { assert_eq!(parse_date("2024-03-15"), Some(Date{year:2024,month:3,day:15})); }
#[test] fn slash() { assert_eq!(parse_date("15/03/2024"), Some(Date{year:2024,month:3,day:15})); }
#[test] fn written() { assert_eq!(parse_date("March 15 2024"), Some(Date{year:2024,month:3,day:15})); }
#[test] fn invalid() { assert_eq!(parse_date("not a date"), None); }
#[test] fn invalid_month() { assert_eq!(parse_date("2024-13-01"), None); }
#[test] fn invalid_day() { assert_eq!(parse_date("2024-02-30"), None); }"#),

        problem("opus-semver", "tier1", "Parse and compare semantic version strings (MAJOR.MINOR.PATCH).",
            r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer { pub major: u32, pub minor: u32, pub patch: u32 }
impl SemVer {
    pub fn parse(s: &str) -> Option<Self> { todo!() }
    pub fn bump_major(&self) -> Self { todo!() }
    pub fn bump_minor(&self) -> Self { todo!() }
    pub fn bump_patch(&self) -> Self { todo!() }
}
impl PartialOrd for SemVer { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { todo!() } }
impl Ord for SemVer { fn cmp(&self, other: &Self) -> std::cmp::Ordering { todo!() } }
impl std::fmt::Display for SemVer { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { todo!() } }"#,
            r#"use opus_semver::*;
#[test] fn parse() { assert_eq!(SemVer::parse("1.2.3"), Some(SemVer{major:1,minor:2,patch:3})); }
#[test] fn parse_invalid() { assert_eq!(SemVer::parse("1.2"), None); }
#[test] fn compare() { assert!(SemVer::parse("2.0.0").unwrap() > SemVer::parse("1.9.9").unwrap()); }
#[test] fn compare_minor() { assert!(SemVer::parse("1.2.0").unwrap() > SemVer::parse("1.1.9").unwrap()); }
#[test] fn bump_major() { assert_eq!(SemVer::parse("1.2.3").unwrap().bump_major(), SemVer{major:2,minor:0,patch:0}); }
#[test] fn bump_minor() { assert_eq!(SemVer::parse("1.2.3").unwrap().bump_minor(), SemVer{major:1,minor:3,patch:0}); }
#[test] fn display() { assert_eq!(format!("{}", SemVer::parse("1.2.3").unwrap()), "1.2.3"); }"#),

        problem("opus-rate-limiter", "tier1", "Implement a token bucket rate limiter.",
            r#"pub struct RateLimiter { /* fields */ }
impl RateLimiter {
    pub fn new(capacity: u32, refill_rate: f64) -> Self { todo!() }
    pub fn try_acquire(&mut self, now_secs: f64) -> bool { todo!() }
    pub fn tokens(&self) -> u32 { todo!() }
}"#,
            r#"use opus_rate_limiter::*;
#[test] fn basic() { let mut r = RateLimiter::new(5, 1.0); assert!(r.try_acquire(0.0)); assert_eq!(r.tokens(), 4); }
#[test] fn exhaust() { let mut r = RateLimiter::new(2, 1.0); assert!(r.try_acquire(0.0)); assert!(r.try_acquire(0.0)); assert!(!r.try_acquire(0.0)); }
#[test] fn refill() { let mut r = RateLimiter::new(2, 1.0); r.try_acquire(0.0); r.try_acquire(0.0); assert!(r.try_acquire(2.0)); }
#[test] fn no_overflow() { let mut r = RateLimiter::new(3, 1.0); assert!(r.try_acquire(100.0)); assert_eq!(r.tokens(), 2); }"#),

        problem("opus-bloom-filter", "tier1", "Implement a simple Bloom filter for string membership testing.",
            r#"pub struct BloomFilter { /* fields */ }
impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self { todo!() }
    pub fn insert(&mut self, item: &str) { todo!() }
    pub fn might_contain(&self, item: &str) -> bool { todo!() }
}"#,
            r#"use opus_bloom_filter::*;
#[test] fn basic() { let mut b = BloomFilter::new(1000, 3); b.insert("hello"); assert!(b.might_contain("hello")); }
#[test] fn not_present() { let b = BloomFilter::new(1000, 3); assert!(!b.might_contain("hello")); }
#[test] fn multiple() { let mut b = BloomFilter::new(1000, 3); b.insert("a"); b.insert("b"); assert!(b.might_contain("a")); assert!(b.might_contain("b")); }
#[test] fn false_positive_rate_low() {
    let mut b = BloomFilter::new(10000, 5);
    for i in 0..100 { b.insert(&format!("item{}", i)); }
    let false_pos = (1000..2000).filter(|i| b.might_contain(&format!("item{}", i))).count();
    assert!(false_pos < 50, "too many false positives: {}", false_pos);
}"#),

        problem("opus-union-find", "tier1", "Implement Union-Find (disjoint set) with path compression and union by rank.",
            r#"pub struct UnionFind { /* fields */ }
impl UnionFind {
    pub fn new(n: usize) -> Self { todo!() }
    pub fn find(&mut self, x: usize) -> usize { todo!() }
    pub fn union(&mut self, x: usize, y: usize) -> bool { todo!() }
    pub fn connected(&mut self, x: usize, y: usize) -> bool { todo!() }
    pub fn components(&mut self) -> usize { todo!() }
}"#,
            r#"use opus_union_find::*;
#[test] fn basic() { let mut uf = UnionFind::new(5); uf.union(0, 1); assert!(uf.connected(0, 1)); assert!(!uf.connected(0, 2)); }
#[test] fn transitive() { let mut uf = UnionFind::new(5); uf.union(0, 1); uf.union(1, 2); assert!(uf.connected(0, 2)); }
#[test] fn components() { let mut uf = UnionFind::new(5); assert_eq!(uf.components(), 5); uf.union(0, 1); assert_eq!(uf.components(), 4); }
#[test] fn already_connected() { let mut uf = UnionFind::new(3); uf.union(0, 1); assert!(!uf.union(0, 1)); }
#[test] fn all_connected() { let mut uf = UnionFind::new(4); uf.union(0,1); uf.union(2,3); uf.union(0,2); assert_eq!(uf.components(), 1); }"#),

        problem("opus-valid-email", "tier1", "Validate email addresses (basic RFC 5322 rules).",
            r#"pub fn is_valid_email(s: &str) -> bool { todo!() }"#,
            r#"use opus_valid_email::*;
#[test] fn valid() { assert!(is_valid_email("user@example.com")); }
#[test] fn valid_dots() { assert!(is_valid_email("first.last@example.com")); }
#[test] fn valid_plus() { assert!(is_valid_email("user+tag@example.com")); }
#[test] fn no_at() { assert!(!is_valid_email("userexample.com")); }
#[test] fn double_at() { assert!(!is_valid_email("user@@example.com")); }
#[test] fn no_domain() { assert!(!is_valid_email("user@")); }
#[test] fn no_local() { assert!(!is_valid_email("@example.com")); }
#[test] fn spaces() { assert!(!is_valid_email("user @example.com")); }
#[test] fn no_tld() { assert!(!is_valid_email("user@localhost")); }"#),

        problem("opus-json-path", "tier1", "Implement simple JSON path queries on serde_json::Value. Support .key and [index] syntax.",
            r#"pub fn query(json: &str, path: &str) -> Option<String> { todo!() }"#,
            r#"use opus_json_path::*;
#[test] fn root_key() { assert_eq!(query("{\"a\":1}", ".a"), Some("1".into())); }
#[test] fn nested() { assert_eq!(query("{\"a\":{\"b\":2}}", ".a.b"), Some("2".into())); }
#[test] fn array() { assert_eq!(query("{\"a\":[1,2,3]}", ".a[1]"), Some("2".into())); }
#[test] fn string_val() { assert_eq!(query("{\"name\":\"alice\"}", ".name"), Some("\"alice\"".into())); }
#[test] fn missing() { assert_eq!(query("{\"a\":1}", ".b"), None); }
#[test] fn root() { assert_eq!(query("42", "."), Some("42".into())); }"#),
    ]
}
