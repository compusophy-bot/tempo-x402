use super::problem;
use crate::benchmark::ExercismProblem;

pub(super) fn tier2_ext3() -> Vec<ExercismProblem> {
    vec![
        problem("opus-bugfix-bfs", "tier2",
            "This BFS finds shortest path but returns the wrong path — it builds the path incorrectly from the parent map. Fix it.",
            r#"use std::collections::{HashMap, HashSet, VecDeque};

pub fn bfs_path(adj: &HashMap<String, Vec<String>>, start: &str, end: &str) -> Option<Vec<String>> {
    let mut visited = HashSet::new();
    let mut parent: HashMap<String, String> = HashMap::new();
    let mut queue = VecDeque::new();
    visited.insert(start.to_string());
    queue.push_back(start.to_string());
    while let Some(node) = queue.pop_front() {
        if node == end {
            // Reconstruct path
            let mut path = vec![end.to_string()];
            let mut current = end.to_string();
            while current != start {
                current = parent[&current].clone();
                path.push(current.clone());
            }
            // BUG: path is reversed, need to reverse it
            return Some(path);
        }
        if let Some(neighbors) = adj.get(&node) {
            for next in neighbors {
                if !visited.contains(next) {
                    visited.insert(next.clone());
                    parent.insert(next.clone(), node.clone());
                    queue.push_back(next.clone());
                }
            }
        }
    }
    None
}"#,
            r#"use opus_bugfix_bfs::*;
use std::collections::HashMap;
fn graph(edges: &[(&str, &str)]) -> HashMap<String, Vec<String>> {
    let mut adj = HashMap::new();
    for &(a, b) in edges {
        adj.entry(a.to_string()).or_insert_with(Vec::new).push(b.to_string());
        adj.entry(b.to_string()).or_insert_with(Vec::new).push(a.to_string());
    }
    adj
}
#[test] fn direct() { let g = graph(&[("a","b")]); assert_eq!(bfs_path(&g, "a", "b").unwrap(), vec!["a","b"]); }
#[test] fn two_hop() { let g = graph(&[("a","b"),("b","c")]); assert_eq!(bfs_path(&g, "a", "c").unwrap(), vec!["a","b","c"]); }
#[test] fn same() { let g = graph(&[("a","b")]); assert_eq!(bfs_path(&g, "a", "a").unwrap(), vec!["a"]); }
#[test] fn no_path() { let g = graph(&[("a","b"),("c","d")]); assert!(bfs_path(&g, "a", "d").is_none()); }"#),

        problem("opus-bugfix-quicksort", "tier2",
            "This quicksort implementation doesn't handle duplicates correctly and stack-overflows on sorted input. Fix it.",
            r#"pub fn quicksort(arr: &mut [i32]) {
    if arr.len() <= 1 { return; }
    let pivot_idx = arr.len() - 1; // BUG: always picking last element = O(n^2) on sorted input
    let pivot = arr[pivot_idx];
    let mut i = 0;
    for j in 0..arr.len() - 1 {
        if arr[j] < pivot { // BUG: should be <= to handle duplicates
            arr.swap(i, j);
            i += 1;
        }
    }
    arr.swap(i, pivot_idx);
    quicksort(&mut arr[..i]);
    quicksort(&mut arr[i + 1..]);
}"#,
            r#"use opus_bugfix_quicksort::*;
#[test] fn basic() { let mut v = vec![3,1,2]; quicksort(&mut v); assert_eq!(v, vec![1,2,3]); }
#[test] fn sorted() { let mut v = vec![1,2,3,4,5]; quicksort(&mut v); assert_eq!(v, vec![1,2,3,4,5]); }
#[test] fn reversed() { let mut v = vec![5,4,3,2,1]; quicksort(&mut v); assert_eq!(v, vec![1,2,3,4,5]); }
#[test] fn duplicates() { let mut v = vec![3,1,3,1,3]; quicksort(&mut v); assert_eq!(v, vec![1,1,3,3,3]); }
#[test] fn all_same() { let mut v = vec![5,5,5,5]; quicksort(&mut v); assert_eq!(v, vec![5,5,5,5]); }
#[test] fn empty() { let mut v: Vec<i32> = vec![]; quicksort(&mut v); assert_eq!(v, Vec::<i32>::new()); }
#[test] fn large_sorted() { let mut v: Vec<i32> = (0..1000).collect(); quicksort(&mut v); assert_eq!(v, (0..1000).collect::<Vec<_>>()); }"#),

        problem("opus-bugfix-tokenizer", "tier2",
            "This tokenizer for arithmetic expressions mishandles negative numbers and multi-digit numbers. Fix it.",
            r#"#[derive(Debug, Clone, PartialEq)]
pub enum Token { Num(f64), Op(char), LParen, RParen }

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = vec![];
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' => { chars.next(); }
            '0'..='9' => {
                let mut num = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_digit(10) { num.push(d); chars.next(); }
                    else { break; }
                }
                // BUG: doesn't handle decimal points
                tokens.push(Token::Num(num.parse().unwrap()));
            }
            '+' | '-' | '*' | '/' => {
                // BUG: doesn't handle negative numbers (e.g., "-5" at start or after operator)
                tokens.push(Token::Op(c));
                chars.next();
            }
            '(' => { tokens.push(Token::LParen); chars.next(); }
            ')' => { tokens.push(Token::RParen); chars.next(); }
            _ => return Err(format!("unexpected char: {}", c)),
        }
    }
    Ok(tokens)
}"#,
            r#"use opus_bugfix_tokenizer::*;
#[test] fn simple() { assert_eq!(tokenize("1+2").unwrap(), vec![Token::Num(1.0), Token::Op('+'), Token::Num(2.0)]); }
#[test] fn multi_digit() { assert_eq!(tokenize("123+456").unwrap(), vec![Token::Num(123.0), Token::Op('+'), Token::Num(456.0)]); }
#[test] fn decimal() { assert_eq!(tokenize("3.14").unwrap(), vec![Token::Num(3.14)]); }
#[test] fn negative_start() { assert_eq!(tokenize("-5+3").unwrap(), vec![Token::Num(-5.0), Token::Op('+'), Token::Num(3.0)]); }
#[test] fn negative_after_op() { assert_eq!(tokenize("3*-2").unwrap(), vec![Token::Num(3.0), Token::Op('*'), Token::Num(-2.0)]); }
#[test] fn parens() { let t = tokenize("(1+2)").unwrap(); assert_eq!(t.len(), 5); assert_eq!(t[0], Token::LParen); }
#[test] fn spaces() { assert_eq!(tokenize("1 + 2").unwrap(), vec![Token::Num(1.0), Token::Op('+'), Token::Num(2.0)]); }"#),

        problem("opus-bugfix-trie-delete", "tier2",
            "This trie's delete function corrupts the tree — it removes the word marker but doesn't clean up empty branches. Fix it.",
            r#"use std::collections::HashMap;

pub struct Trie { children: HashMap<char, Trie>, is_end: bool }

impl Trie {
    pub fn new() -> Self { Trie { children: HashMap::new(), is_end: false } }
    pub fn insert(&mut self, word: &str) {
        let mut node = self;
        for c in word.chars() { node = node.children.entry(c).or_insert_with(Trie::new); }
        node.is_end = true;
    }
    pub fn search(&self, word: &str) -> bool {
        let mut node = self;
        for c in word.chars() {
            match node.children.get(&c) { Some(n) => node = n, None => return false }
        }
        node.is_end
    }
    pub fn delete(&mut self, word: &str) -> bool {
        // BUG: just unsets is_end, doesn't clean up orphan nodes
        let mut node = self;
        for c in word.chars() {
            match node.children.get_mut(&c) { Some(n) => node = n, None => return false }
        }
        if node.is_end { node.is_end = false; true } else { false }
    }
    pub fn starts_with(&self, prefix: &str) -> bool {
        let mut node = self;
        for c in prefix.chars() {
            match node.children.get(&c) { Some(n) => node = n, None => return false }
        }
        true
    }
}"#,
            r#"use opus_bugfix_trie_delete::*;
#[test] fn insert_search() { let mut t = Trie::new(); t.insert("hello"); assert!(t.search("hello")); }
#[test] fn delete_basic() { let mut t = Trie::new(); t.insert("hello"); assert!(t.delete("hello")); assert!(!t.search("hello")); }
#[test] fn delete_preserves_prefix() { let mut t = Trie::new(); t.insert("hello"); t.insert("help"); t.delete("hello"); assert!(t.search("help")); assert!(!t.search("hello")); }
#[test] fn delete_cleans_orphans() { let mut t = Trie::new(); t.insert("abc"); t.delete("abc"); assert!(!t.starts_with("a")); }
#[test] fn delete_missing() { let mut t = Trie::new(); t.insert("abc"); assert!(!t.delete("xyz")); }"#),

        problem("opus-bugfix-regex-match", "tier2",
            "This simple regex matcher supports . and * but has bugs with greedy matching and empty patterns. Fix it.",
            r#"pub fn is_match(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    matches(&t, &p, 0, 0)
}

fn matches(t: &[char], p: &[char], ti: usize, pi: usize) -> bool {
    if pi == p.len() { return ti == t.len(); }

    let has_star = pi + 1 < p.len() && p[pi + 1] == '*';

    if has_star {
        // Try zero occurrences
        if matches(t, p, ti, pi + 2) { return true; }
        // Try one or more occurrences
        // BUG: doesn't check bounds before comparing
        if ti < t.len() && (p[pi] == '.' || p[pi] == t[ti]) {
            return matches(t, p, ti + 1, pi); // keep trying current pattern
        }
        false
    } else {
        if ti >= t.len() { return false; } // BUG: should still check if remaining pattern is all x* pairs
        if p[pi] == '.' || p[pi] == t[ti] {
            matches(t, p, ti + 1, pi + 1)
        } else {
            false
        }
    }
}"#,
            r#"use opus_bugfix_regex_match::*;
#[test] fn exact() { assert!(is_match("abc", "abc")); }
#[test] fn dot() { assert!(is_match("abc", "a.c")); }
#[test] fn star() { assert!(is_match("aab", "a*b")); }
#[test] fn dot_star() { assert!(is_match("anything", ".*")); }
#[test] fn no_match() { assert!(!is_match("abc", "abd")); }
#[test] fn star_zero() { assert!(is_match("b", "a*b")); }
#[test] fn empty_both() { assert!(is_match("", "")); }
#[test] fn empty_pattern_star() { assert!(is_match("", "a*")); }
#[test] fn complex() { assert!(is_match("aab", "c*a*b")); }"#),
    ]
}
