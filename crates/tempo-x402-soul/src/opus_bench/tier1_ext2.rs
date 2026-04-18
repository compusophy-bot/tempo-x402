use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier1_ext2() -> Vec<BenchmarkProblem> {
    vec![
        problem("opus-stack", "tier1",
            "Implement a stack with push, pop, peek, and min operations. min() must be O(1).",
            r#"pub struct MinStack { /* fields */ }
impl MinStack {
    pub fn new() -> Self { todo!() }
    pub fn push(&mut self, val: i64) { todo!() }
    pub fn pop(&mut self) -> Option<i64> { todo!() }
    pub fn peek(&self) -> Option<i64> { todo!() }
    pub fn min(&self) -> Option<i64> { todo!() }
    pub fn len(&self) -> usize { todo!() }
}"#,
            r#"use opus_stack::*;
#[test] fn basic() { let mut s = MinStack::new(); s.push(3); s.push(1); s.push(2); assert_eq!(s.min(), Some(1)); }
#[test] fn min_after_pop() { let mut s = MinStack::new(); s.push(2); s.push(1); s.pop(); assert_eq!(s.min(), Some(2)); }
#[test] fn empty() { let s = MinStack::new(); assert_eq!(s.min(), None); assert_eq!(s.pop(), None); }
#[test] fn peek() { let mut s = MinStack::new(); s.push(5); assert_eq!(s.peek(), Some(5)); assert_eq!(s.len(), 1); }
#[test] fn duplicates() { let mut s = MinStack::new(); s.push(1); s.push(1); s.pop(); assert_eq!(s.min(), Some(1)); }"#),

        problem("opus-deque", "tier1",
            "Implement a double-ended queue supporting push/pop from both front and back.",
            r#"pub struct Deque<T> { /* fields */ }
impl<T> Deque<T> {
    pub fn new() -> Self { todo!() }
    pub fn push_front(&mut self, val: T) { todo!() }
    pub fn push_back(&mut self, val: T) { todo!() }
    pub fn pop_front(&mut self) -> Option<T> { todo!() }
    pub fn pop_back(&mut self) -> Option<T> { todo!() }
    pub fn len(&self) -> usize { todo!() }
    pub fn is_empty(&self) -> bool { todo!() }
}"#,
            r#"use opus_deque::*;
#[test] fn push_pop_back() { let mut d = Deque::new(); d.push_back(1); d.push_back(2); assert_eq!(d.pop_back(), Some(2)); }
#[test] fn push_pop_front() { let mut d = Deque::new(); d.push_front(1); d.push_front(2); assert_eq!(d.pop_front(), Some(2)); }
#[test] fn mixed() { let mut d = Deque::new(); d.push_back(1); d.push_front(2); assert_eq!(d.pop_front(), Some(2)); assert_eq!(d.pop_front(), Some(1)); }
#[test] fn empty() { let mut d = Deque::<i32>::new(); assert!(d.is_empty()); assert_eq!(d.pop_front(), None); }
#[test] fn len() { let mut d = Deque::new(); d.push_back(1); d.push_back(2); d.push_front(3); assert_eq!(d.len(), 3); }"#),

        problem("opus-base64", "tier1",
            "Implement base64 encoding and decoding (standard alphabet, = padding).",
            r#"pub fn encode(input: &[u8]) -> String { todo!() }
pub fn decode(input: &str) -> Result<Vec<u8>, String> { todo!() }"#,
            r#"use opus_base64::*;
#[test] fn encode_hello() { assert_eq!(encode(b"Hello"), "SGVsbG8="); }
#[test] fn encode_empty() { assert_eq!(encode(b""), ""); }
#[test] fn encode_one() { assert_eq!(encode(b"a"), "YQ=="); }
#[test] fn encode_two() { assert_eq!(encode(b"ab"), "YWI="); }
#[test] fn encode_three() { assert_eq!(encode(b"abc"), "YWJj"); }
#[test] fn decode_hello() { assert_eq!(decode("SGVsbG8=").unwrap(), b"Hello"); }
#[test] fn decode_empty() { assert_eq!(decode("").unwrap(), b""); }
#[test] fn roundtrip() { let data = b"The quick brown fox"; assert_eq!(decode(&encode(data)).unwrap(), data); }
#[test] fn decode_invalid() { assert!(decode("!!!").is_err()); }"#),

        problem("opus-interval-merge", "tier1",
            "Merge overlapping intervals. Input: list of (start, end) pairs. Output: merged non-overlapping intervals.",
            r#"pub fn merge(intervals: &[(i32, i32)]) -> Vec<(i32, i32)> { todo!() }"#,
            r#"use opus_interval_merge::*;
#[test] fn basic() { assert_eq!(merge(&[(1,3),(2,6),(8,10)]), vec![(1,6),(8,10)]); }
#[test] fn no_overlap() { assert_eq!(merge(&[(1,2),(3,4),(5,6)]), vec![(1,2),(3,4),(5,6)]); }
#[test] fn all_overlap() { assert_eq!(merge(&[(1,10),(2,5),(3,7)]), vec![(1,10)]); }
#[test] fn single() { assert_eq!(merge(&[(1,5)]), vec![(1,5)]); }
#[test] fn empty() { assert_eq!(merge(&[]), Vec::<(i32,i32)>::new()); }
#[test] fn touching() { assert_eq!(merge(&[(1,2),(2,3)]), vec![(1,3)]); }
#[test] fn unsorted() { assert_eq!(merge(&[(5,6),(1,3),(2,4)]), vec![(1,4),(5,6)]); }"#),

        problem("opus-valid-ip", "tier1",
            "Validate IPv4 and IPv6 addresses.",
            r#"pub fn is_valid_ipv4(s: &str) -> bool { todo!() }
pub fn is_valid_ipv6(s: &str) -> bool { todo!() }"#,
            r#"use opus_valid_ip::*;
#[test] fn v4_valid() { assert!(is_valid_ipv4("192.168.1.1")); }
#[test] fn v4_zeros() { assert!(is_valid_ipv4("0.0.0.0")); }
#[test] fn v4_max() { assert!(is_valid_ipv4("255.255.255.255")); }
#[test] fn v4_leading_zero() { assert!(!is_valid_ipv4("01.02.03.04")); }
#[test] fn v4_too_high() { assert!(!is_valid_ipv4("256.1.1.1")); }
#[test] fn v4_too_few() { assert!(!is_valid_ipv4("1.2.3")); }
#[test] fn v4_empty() { assert!(!is_valid_ipv4("")); }
#[test] fn v6_valid() { assert!(is_valid_ipv6("2001:0db8:85a3:0000:0000:8a2e:0370:7334")); }
#[test] fn v6_short() { assert!(is_valid_ipv6("::1")); }
#[test] fn v6_invalid() { assert!(!is_valid_ipv6("not:an:ipv6")); }"#),

        problem("opus-word-count", "tier1",
            "Count word frequencies in text. Words are lowercased, punctuation stripped.",
            r#"use std::collections::HashMap;
pub fn word_count(text: &str) -> HashMap<String, usize> { todo!() }"#,
            r#"use opus_word_count::*;
#[test] fn basic() { let c = word_count("one fish two fish"); assert_eq!(c["fish"], 2); assert_eq!(c["one"], 1); }
#[test] fn case() { let c = word_count("Hello hello HELLO"); assert_eq!(c["hello"], 3); }
#[test] fn punctuation() { let c = word_count("hello, world!"); assert_eq!(c["hello"], 1); assert_eq!(c["world"], 1); }
#[test] fn empty() { assert!(word_count("").is_empty()); }
#[test] fn whitespace() { assert!(word_count("   ").is_empty()); }"#),

        problem("opus-pangram", "tier1",
            "Check if a string is a pangram (contains every letter a-z at least once). \
             Also return the missing letters if not a pangram.",
            r#"pub fn is_pangram(s: &str) -> bool { todo!() }
pub fn missing_letters(s: &str) -> Vec<char> { todo!() }"#,
            r#"use opus_pangram::*;
#[test] fn classic() { assert!(is_pangram("the quick brown fox jumps over the lazy dog")); }
#[test] fn missing() { assert!(!is_pangram("hello world")); }
#[test] fn empty() { assert!(!is_pangram("")); }
#[test] fn case_insensitive() { assert!(is_pangram("THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG")); }
#[test] fn missing_letters_test() { let m = missing_letters("abcde"); assert!(m.contains(&'f')); assert!(m.contains(&'z')); assert_eq!(m.len(), 21); }
#[test] fn pangram_missing_empty() { let m = missing_letters("the quick brown fox jumps over the lazy dog"); assert!(m.is_empty()); }"#),

        problem("opus-bracket-gen", "tier1",
            "Generate all valid combinations of n pairs of parentheses.",
            r#"pub fn generate_parens(n: u32) -> Vec<String> { todo!() }"#,
            r#"use opus_bracket_gen::*;
#[test] fn zero() { assert_eq!(generate_parens(0), vec![""]); }
#[test] fn one() { assert_eq!(generate_parens(1), vec!["()"]); }
#[test] fn two() { let mut r = generate_parens(2); r.sort(); assert_eq!(r, vec!["(())", "()()"]); }
#[test] fn three() { let r = generate_parens(3); assert_eq!(r.len(), 5); assert!(r.contains(&"((()))".to_string())); assert!(r.contains(&"(()())".to_string())); }
#[test] fn four() { assert_eq!(generate_parens(4).len(), 14); }"#),

        problem("opus-flatten", "tier1",
            "Implement a flatten function for nested vectors. Also implement a depth-limited flatten.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum Nested {
    Val(i32),
    List(Vec<Nested>),
}
pub fn flatten(n: &Nested) -> Vec<i32> { todo!() }
pub fn flatten_depth(n: &Nested, max_depth: usize) -> Vec<Nested> { todo!() }"#,
            r#"use opus_flatten::*;
#[test] fn flat_val() { assert_eq!(flatten(&Nested::Val(1)), vec![1]); }
#[test] fn flat_list() { assert_eq!(flatten(&Nested::List(vec![Nested::Val(1), Nested::Val(2)])), vec![1, 2]); }
#[test] fn flat_nested() {
    let n = Nested::List(vec![Nested::Val(1), Nested::List(vec![Nested::Val(2), Nested::Val(3)])]);
    assert_eq!(flatten(&n), vec![1, 2, 3]);
}
#[test] fn flat_deep() {
    let n = Nested::List(vec![Nested::List(vec![Nested::List(vec![Nested::Val(42)])])]);
    assert_eq!(flatten(&n), vec![42]);
}
#[test] fn flat_empty() { assert_eq!(flatten(&Nested::List(vec![])), Vec::<i32>::new()); }
#[test] fn depth_zero() {
    let n = Nested::List(vec![Nested::Val(1), Nested::List(vec![Nested::Val(2)])]);
    let r = flatten_depth(&n, 0);
    assert_eq!(r.len(), 2); // [Val(1), List([Val(2)])]
}"#),

        problem("opus-zigzag", "tier1",
            "Convert a string to zigzag pattern with n rows, then read line by line. \
             Example: 'PAYPALISHIRING' with 3 rows -> 'PAHNAPLSIIGYIR'.",
            r#"pub fn zigzag(s: &str, rows: usize) -> String { todo!() }"#,
            r#"use opus_zigzag::*;
#[test] fn three_rows() { assert_eq!(zigzag("PAYPALISHIRING", 3), "PAHNAPLSIIGYIR"); }
#[test] fn four_rows() { assert_eq!(zigzag("PAYPALISHIRING", 4), "PINALSIGYAHRPI"); }
#[test] fn one_row() { assert_eq!(zigzag("HELLO", 1), "HELLO"); }
#[test] fn same_as_len() { assert_eq!(zigzag("ABC", 3), "ABC"); }
#[test] fn more_rows() { assert_eq!(zigzag("AB", 5), "AB"); }
#[test] fn empty() { assert_eq!(zigzag("", 3), ""); }"#),

        problem("opus-anagram", "tier1",
            "Find all anagram groups in a list of words.",
            r#"pub fn group_anagrams(words: &[&str]) -> Vec<Vec<String>> { todo!() }"#,
            r#"use opus_anagram::*;
#[test] fn basic() {
    let mut groups = group_anagrams(&["eat","tea","tan","ate","nat","bat"]);
    for g in &mut groups { g.sort(); }
    groups.sort_by_key(|g| g[0].clone());
    assert_eq!(groups.len(), 3);
    assert!(groups.iter().any(|g| g == &["ate", "eat", "tea"]));
    assert!(groups.iter().any(|g| g == &["nat", "tan"]));
    assert!(groups.iter().any(|g| g == &["bat"]));
}
#[test] fn empty() { assert!(group_anagrams(&[]).is_empty()); }
#[test] fn single() { assert_eq!(group_anagrams(&["a"]).len(), 1); }
#[test] fn no_anagrams() { assert_eq!(group_anagrams(&["abc","def","ghi"]).len(), 3); }"#),

        problem("opus-powers-of-two", "tier1",
            "Given an integer, determine if it's a power of two. Also find the next power of two >= n.",
            r#"pub fn is_power_of_two(n: u64) -> bool { todo!() }
pub fn next_power_of_two(n: u64) -> u64 { todo!() }"#,
            r#"use opus_powers_of_two::*;
#[test] fn pow_1() { assert!(is_power_of_two(1)); }
#[test] fn pow_2() { assert!(is_power_of_two(2)); }
#[test] fn pow_1024() { assert!(is_power_of_two(1024)); }
#[test] fn not_3() { assert!(!is_power_of_two(3)); }
#[test] fn not_0() { assert!(!is_power_of_two(0)); }
#[test] fn next_1() { assert_eq!(next_power_of_two(1), 1); }
#[test] fn next_3() { assert_eq!(next_power_of_two(3), 4); }
#[test] fn next_5() { assert_eq!(next_power_of_two(5), 8); }
#[test] fn next_exact() { assert_eq!(next_power_of_two(16), 16); }"#),

        problem("opus-rot13", "tier1",
            "Implement ROT13 cipher (rotate each letter by 13 positions, preserve case and non-alpha).",
            r#"pub fn rot13(s: &str) -> String { todo!() }"#,
            r#"use opus_rot13::*;
#[test] fn basic() { assert_eq!(rot13("Hello"), "Uryyb"); }
#[test] fn roundtrip() { assert_eq!(rot13(&rot13("Test 123!")), "Test 123!"); }
#[test] fn all_alpha() { assert_eq!(rot13("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"), "NOPQRSTUVWXYZABCDEFGHIJKLMnopqrstuvwxyzabcdefghijklm"); }
#[test] fn non_alpha() { assert_eq!(rot13("123!@#"), "123!@#"); }
#[test] fn empty() { assert_eq!(rot13(""), ""); }"#),

        problem("opus-caesar-cipher", "tier1",
            "Implement Caesar cipher with configurable shift for encrypt and decrypt.",
            r#"pub fn encrypt(text: &str, shift: u8) -> String { todo!() }
pub fn decrypt(text: &str, shift: u8) -> String { todo!() }"#,
            r#"use opus_caesar_cipher::*;
#[test] fn enc_basic() { assert_eq!(encrypt("abc", 3), "def"); }
#[test] fn enc_wrap() { assert_eq!(encrypt("xyz", 3), "abc"); }
#[test] fn enc_preserve() { assert_eq!(encrypt("Hello, World!", 5), "Mjqqt, Btwqi!"); }
#[test] fn dec_basic() { assert_eq!(decrypt("def", 3), "abc"); }
#[test] fn roundtrip() { assert_eq!(decrypt(&encrypt("Test!", 13), 13), "Test!"); }
#[test] fn zero_shift() { assert_eq!(encrypt("hello", 0), "hello"); }
#[test] fn full_shift() { assert_eq!(encrypt("hello", 26), "hello"); }"#),

        problem("opus-matrix-rotate", "tier1",
            "Rotate a square matrix 90 degrees clockwise in-place.",
            r#"pub fn rotate_90(matrix: &mut Vec<Vec<i32>>) { todo!() }"#,
            r#"use opus_matrix_rotate::*;
#[test] fn two() { let mut m = vec![vec![1,2],vec![3,4]]; rotate_90(&mut m); assert_eq!(m, vec![vec![3,1],vec![4,2]]); }
#[test] fn three() { let mut m = vec![vec![1,2,3],vec![4,5,6],vec![7,8,9]]; rotate_90(&mut m); assert_eq!(m, vec![vec![7,4,1],vec![8,5,2],vec![9,6,3]]); }
#[test] fn one() { let mut m = vec![vec![1]]; rotate_90(&mut m); assert_eq!(m, vec![vec![1]]); }
#[test] fn four_rotations() { let orig = vec![vec![1,2],vec![3,4]]; let mut m = orig.clone(); for _ in 0..4 { rotate_90(&mut m); } assert_eq!(m, orig); }"#),

        problem("opus-sparse-vector", "tier1",
            "Implement a sparse vector that only stores non-zero elements. Support dot product.",
            r#"pub struct SparseVec { /* fields */ }
impl SparseVec {
    pub fn new(size: usize) -> Self { todo!() }
    pub fn set(&mut self, idx: usize, val: f64) { todo!() }
    pub fn get(&self, idx: usize) -> f64 { todo!() }
    pub fn dot(&self, other: &SparseVec) -> f64 { todo!() }
    pub fn nnz(&self) -> usize { todo!() }
}"#,
            r#"use opus_sparse_vector::*;
#[test] fn basic() { let mut v = SparseVec::new(100); v.set(5, 3.0); assert_eq!(v.get(5), 3.0); assert_eq!(v.get(0), 0.0); }
#[test] fn dot_product() {
    let mut a = SparseVec::new(100);
    let mut b = SparseVec::new(100);
    a.set(0, 1.0); a.set(50, 2.0);
    b.set(0, 3.0); b.set(50, 4.0);
    assert_eq!(a.dot(&b), 11.0); // 1*3 + 2*4
}
#[test] fn nnz() { let mut v = SparseVec::new(1000); v.set(1, 1.0); v.set(999, 2.0); assert_eq!(v.nnz(), 2); }
#[test] fn zero_removes() { let mut v = SparseVec::new(10); v.set(5, 3.0); v.set(5, 0.0); assert_eq!(v.nnz(), 0); }"#),
    ]
}
