use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier1_ext6() -> Vec<BenchmarkProblem> {
    vec![
        problem("opus-crc32", "tier1", "Implement CRC-32 checksum calculation.",
            r#"pub fn crc32(data: &[u8]) -> u32 { todo!() }"#,
            r#"use opus_crc32::*;
#[test] fn empty() { assert_eq!(crc32(b""), 0x00000000); }
#[test] fn hello() { assert_eq!(crc32(b"hello"), 0x3610A686); }
#[test] fn test_str() { assert_eq!(crc32(b"123456789"), 0xCBF43926); }
#[test] fn single() { assert_ne!(crc32(b"a"), 0); }"#),

        problem("opus-brainfuck", "tier1", "Implement a Brainfuck interpreter. Supports + - > < [ ] . ,",
            r#"pub fn interpret(code: &str, input: &[u8]) -> Vec<u8> { todo!() }"#,
            r#"use opus_brainfuck::*;
#[test] fn hello() { let out = interpret("+++++++++[>++++++++<-]>.", &[]); assert_eq!(out, vec![72]); } // H
#[test] fn cat() { let out = interpret(",.,.", &[65, 66]); assert_eq!(out, vec![65, 66]); }
#[test] fn add() { let out = interpret("+++++[>+++<-]>.", &[]); assert_eq!(out, vec![15]); }
#[test] fn empty() { assert_eq!(interpret("", &[]), vec![]); }
#[test] fn noop() { assert_eq!(interpret(">>><<<", &[]), vec![]); }"#),

        problem("opus-huffman", "tier1", "Implement Huffman encoding: build tree from frequencies, encode to bits, decode back.",
            r#"use std::collections::HashMap;
pub fn build_codes(text: &str) -> HashMap<char, String> { todo!() }
pub fn encode(text: &str, codes: &HashMap<char, String>) -> String { todo!() }
pub fn decode(bits: &str, codes: &HashMap<char, String>) -> String { todo!() }"#,
            r#"use opus_huffman::*;
#[test] fn single_char() { let codes = build_codes("aaa"); assert_eq!(codes.len(), 1); }
#[test] fn two_chars() { let codes = build_codes("aab"); assert_eq!(codes.len(), 2); }
#[test] fn roundtrip() { let text = "hello world"; let codes = build_codes(text); let enc = encode(text, &codes); let dec = decode(&enc, &codes); assert_eq!(dec, text); }
#[test] fn compression() { let text = "aaaaaabbbbcc"; let codes = build_codes(text); let enc = encode(text, &codes); assert!(enc.len() < text.len() * 8); }"#),

        problem("opus-state-machine", "tier1", "Implement a simple finite state machine that processes transitions.",
            r#"use std::collections::HashMap;
pub struct StateMachine {
    pub current: String,
    pub transitions: HashMap<(String, String), String>,
    pub accept_states: Vec<String>,
}
impl StateMachine {
    pub fn new(initial: &str) -> Self { todo!() }
    pub fn add_transition(&mut self, from: &str, input: &str, to: &str) { todo!() }
    pub fn add_accept(&mut self, state: &str) { todo!() }
    pub fn process(&mut self, input: &str) -> bool { todo!() }
    pub fn run(&mut self, inputs: &[&str]) -> bool { todo!() }
}"#,
            r#"use opus_state_machine::*;
#[test] fn basic() {
    let mut sm = StateMachine::new("q0");
    sm.add_transition("q0", "a", "q1");
    sm.add_transition("q1", "b", "q2");
    sm.add_accept("q2");
    assert!(sm.run(&["a", "b"]));
}
#[test] fn reject() {
    let mut sm = StateMachine::new("q0");
    sm.add_transition("q0", "a", "q1");
    sm.add_accept("q1");
    assert!(!sm.run(&["b"]));
}
#[test] fn loop_state() {
    let mut sm = StateMachine::new("q0");
    sm.add_transition("q0", "a", "q0");
    sm.add_accept("q0");
    assert!(sm.run(&["a", "a", "a"]));
}
#[test] fn empty_input() {
    let mut sm = StateMachine::new("q0");
    sm.add_accept("q0");
    assert!(sm.run(&[]));
}"#),

        problem("opus-equation-solver", "tier1", "Solve simple linear equations like '2x + 3 = 7' for x.",
            r#"pub fn solve(equation: &str) -> Option<f64> { todo!() }"#,
            r#"use opus_equation_solver::*;
#[test] fn simple() { assert!((solve("2x + 3 = 7").unwrap() - 2.0).abs() < 0.001); }
#[test] fn negative() { assert!((solve("x - 5 = -3").unwrap() - 2.0).abs() < 0.001); }
#[test] fn both_sides() { assert!((solve("3x + 1 = x + 5").unwrap() - 2.0).abs() < 0.001); }
#[test] fn zero() { assert!((solve("x = 0").unwrap()).abs() < 0.001); }
#[test] fn no_solution() { assert_eq!(solve("0x = 5"), None); }"#),

        problem("opus-matrix-multiply", "tier1", "Multiply two matrices.",
            r#"pub fn multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> { todo!() }"#,
            r#"use opus_matrix_multiply::*;
#[test] fn basic() {
    let a = vec![vec![1.0,2.0],vec![3.0,4.0]];
    let b = vec![vec![5.0,6.0],vec![7.0,8.0]];
    let c = multiply(&a, &b).unwrap();
    assert_eq!(c[0][0], 19.0);
    assert_eq!(c[1][1], 50.0);
}
#[test] fn identity() {
    let a = vec![vec![1.0,2.0],vec![3.0,4.0]];
    let i = vec![vec![1.0,0.0],vec![0.0,1.0]];
    let c = multiply(&a, &i).unwrap();
    assert_eq!(c, a);
}
#[test] fn incompatible() { assert!(multiply(&[vec![1.0,2.0]], &[vec![1.0,2.0]]).is_none()); }
#[test] fn rect() {
    let a = vec![vec![1.0,2.0,3.0]];
    let b = vec![vec![4.0],vec![5.0],vec![6.0]];
    let c = multiply(&a, &b).unwrap();
    assert_eq!(c, vec![vec![32.0]]);
}"#),

        problem("opus-binary-tree", "tier1", "Implement a binary search tree with insert, search, in-order traversal, and height.",
            r#"pub struct BST { /* fields */ }
impl BST {
    pub fn new() -> Self { todo!() }
    pub fn insert(&mut self, val: i64) { todo!() }
    pub fn contains(&self, val: i64) -> bool { todo!() }
    pub fn in_order(&self) -> Vec<i64> { todo!() }
    pub fn height(&self) -> usize { todo!() }
    pub fn len(&self) -> usize { todo!() }
}"#,
            r#"use opus_binary_tree::*;
#[test] fn insert_search() { let mut t = BST::new(); t.insert(5); t.insert(3); t.insert(7); assert!(t.contains(3)); assert!(!t.contains(4)); }
#[test] fn in_order() { let mut t = BST::new(); for v in [5,3,7,1,4] { t.insert(v); } assert_eq!(t.in_order(), vec![1,3,4,5,7]); }
#[test] fn height() { let mut t = BST::new(); t.insert(2); t.insert(1); t.insert(3); assert_eq!(t.height(), 2); }
#[test] fn empty() { let t = BST::new(); assert_eq!(t.len(), 0); assert_eq!(t.height(), 0); assert!(!t.contains(1)); }
#[test] fn len() { let mut t = BST::new(); t.insert(1); t.insert(2); t.insert(3); assert_eq!(t.len(), 3); }"#),

        problem("opus-range-sum", "tier1", "Implement a data structure for efficient range sum queries on a static array.",
            r#"pub struct RangeSum { /* fields */ }
impl RangeSum {
    pub fn new(data: &[i64]) -> Self { todo!() }
    pub fn query(&self, left: usize, right: usize) -> i64 { todo!() }
}"#,
            r#"use opus_range_sum::*;
#[test] fn basic() { let rs = RangeSum::new(&[1,2,3,4,5]); assert_eq!(rs.query(0, 4), 15); }
#[test] fn single() { let rs = RangeSum::new(&[1,2,3]); assert_eq!(rs.query(1, 1), 2); }
#[test] fn partial() { let rs = RangeSum::new(&[1,2,3,4,5]); assert_eq!(rs.query(1, 3), 9); }
#[test] fn full() { let rs = RangeSum::new(&[10,20,30]); assert_eq!(rs.query(0, 2), 60); }
#[test] fn large() { let data: Vec<i64> = (1..=100).collect(); let rs = RangeSum::new(&data); assert_eq!(rs.query(0, 99), 5050); }"#),

        problem("opus-string-compression", "tier1", "Compress a string by counting consecutive chars: 'aabcccccaaa' → 'a2b1c5a3'. Return original if compressed isn't shorter.",
            r#"pub fn compress(s: &str) -> String { todo!() }"#,
            r#"use opus_string_compression::*;
#[test] fn basic() { assert_eq!(compress("aabcccccaaa"), "a2b1c5a3"); }
#[test] fn no_compress() { assert_eq!(compress("abc"), "abc"); }
#[test] fn single() { assert_eq!(compress("a"), "a"); }
#[test] fn empty() { assert_eq!(compress(""), ""); }
#[test] fn all_same() { assert_eq!(compress("aaaa"), "a4"); }
#[test] fn pairs() { assert_eq!(compress("aabb"), "aabb"); } // a2b2 is same length"#),

        problem("opus-dutch-flag", "tier1", "Sort an array containing only 0s, 1s, and 2s in-place (Dutch National Flag problem).",
            r#"pub fn sort_colors(arr: &mut [u8]) { todo!() }"#,
            r#"use opus_dutch_flag::*;
#[test] fn basic() { let mut a = vec![2,0,1,2,1,0]; sort_colors(&mut a); assert_eq!(a, vec![0,0,1,1,2,2]); }
#[test] fn sorted() { let mut a = vec![0,0,1,1,2,2]; sort_colors(&mut a); assert_eq!(a, vec![0,0,1,1,2,2]); }
#[test] fn reversed() { let mut a = vec![2,2,1,1,0,0]; sort_colors(&mut a); assert_eq!(a, vec![0,0,1,1,2,2]); }
#[test] fn single() { let mut a = vec![1]; sort_colors(&mut a); assert_eq!(a, vec![1]); }
#[test] fn empty() { let mut a: Vec<u8> = vec![]; sort_colors(&mut a); assert!(a.is_empty()); }
#[test] fn all_same() { let mut a = vec![2,2,2]; sort_colors(&mut a); assert_eq!(a, vec![2,2,2]); }"#),
    ]
}
