use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier2_ext2() -> Vec<BenchmarkProblem> {
    vec![
        problem("opus-bugfix-dijkstra", "tier2",
            "This Dijkstra implementation has a priority queue bug — it doesn't update distances when a shorter path is found. Fix it.",
            r#"use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;

pub fn shortest_path(edges: &[(usize, usize, u32)], start: usize, end: usize) -> Option<u32> {
    let mut adj: HashMap<usize, Vec<(usize, u32)>> = HashMap::new();
    for &(a, b, w) in edges {
        adj.entry(a).or_default().push((b, w));
        adj.entry(b).or_default().push((a, w));
    }
    let mut dist: HashMap<usize, u32> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(start, 0);
    heap.push(Reverse((0u32, start)));
    while let Some(Reverse((cost, node))) = heap.pop() {
        if node == end { return Some(cost); }
        // BUG: missing check: if cost > dist[node], skip (stale entry)
        for &(next, w) in adj.get(&node).unwrap_or(&vec![]) {
            let new_cost = cost + w;
            if !dist.contains_key(&next) || new_cost < dist[&next] {
                dist.insert(next, new_cost);
                heap.push(Reverse((new_cost, next)));
            }
        }
    }
    dist.get(&end).copied()
}"#,
            r#"use opus_bugfix_dijkstra::*;
#[test] fn direct() { assert_eq!(shortest_path(&[(0,1,5)], 0, 1), Some(5)); }
#[test] fn two_paths() { assert_eq!(shortest_path(&[(0,1,10),(0,2,3),(2,1,2)], 0, 1), Some(5)); }
#[test] fn no_path() { assert_eq!(shortest_path(&[(0,1,5),(2,3,5)], 0, 3), None); }
#[test] fn same_node() { assert_eq!(shortest_path(&[(0,1,5)], 0, 0), Some(0)); }
#[test] fn longer_chain() { assert_eq!(shortest_path(&[(0,1,1),(1,2,1),(2,3,1),(0,3,10)], 0, 3), Some(3)); }"#),

        problem("opus-bugfix-linked-list", "tier2",
            "This singly linked list has bugs in remove and reverse. Fix them.",
            r#"pub struct Node { pub val: i32, pub next: Option<Box<Node>> }
pub struct LinkedList { pub head: Option<Box<Node>> }

impl LinkedList {
    pub fn new() -> Self { LinkedList { head: None } }
    pub fn push_front(&mut self, val: i32) {
        let node = Box::new(Node { val, next: self.head.take() });
        self.head = Some(node);
    }
    pub fn to_vec(&self) -> Vec<i32> {
        let mut result = vec![];
        let mut current = &self.head;
        while let Some(node) = current {
            result.push(node.val);
            current = &node.next;
        }
        result
    }
    pub fn remove(&mut self, val: i32) -> bool {
        // BUG: doesn't handle removing head node correctly
        let mut current = &mut self.head;
        while let Some(node) = current {
            if node.val == val {
                *current = node.next.take();
                return true;
            }
            current = &mut current.as_mut().unwrap().next; // BUG: borrow checker issue
        }
        false
    }
    pub fn reverse(&mut self) {
        let mut prev = None;
        let mut current = self.head.take();
        while let Some(mut node) = current {
            current = node.next.take(); // BUG: loses reference
            node.next = prev;
            prev = Some(node);
        }
        self.head = prev;
    }
    pub fn len(&self) -> usize { self.to_vec().len() }
}"#,
            r#"use opus_bugfix_linked_list::*;
#[test] fn push_and_vec() { let mut l = LinkedList::new(); l.push_front(3); l.push_front(2); l.push_front(1); assert_eq!(l.to_vec(), vec![1,2,3]); }
#[test] fn remove_head() { let mut l = LinkedList::new(); l.push_front(2); l.push_front(1); assert!(l.remove(1)); assert_eq!(l.to_vec(), vec![2]); }
#[test] fn remove_tail() { let mut l = LinkedList::new(); l.push_front(2); l.push_front(1); assert!(l.remove(2)); assert_eq!(l.to_vec(), vec![1]); }
#[test] fn remove_missing() { let mut l = LinkedList::new(); l.push_front(1); assert!(!l.remove(99)); }
#[test] fn reverse_list() { let mut l = LinkedList::new(); l.push_front(3); l.push_front(2); l.push_front(1); l.reverse(); assert_eq!(l.to_vec(), vec![3,2,1]); }
#[test] fn reverse_empty() { let mut l = LinkedList::new(); l.reverse(); assert_eq!(l.to_vec(), Vec::<i32>::new()); }
#[test] fn reverse_single() { let mut l = LinkedList::new(); l.push_front(1); l.reverse(); assert_eq!(l.to_vec(), vec![1]); }"#),

        problem("opus-bugfix-parser", "tier2",
            "This simple arithmetic parser has operator precedence wrong — multiplication should bind tighter than addition. Fix it.",
            r#"pub fn eval(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    parse_expr(&tokens, &mut 0)
}

fn tokenize(s: &str) -> Result<Vec<String>, String> {
    let mut tokens = vec![];
    let mut num = String::new();
    for c in s.chars() {
        if c.is_digit(10) || c == '.' { num.push(c); }
        else {
            if !num.is_empty() { tokens.push(num.clone()); num.clear(); }
            if !c.is_whitespace() { tokens.push(c.to_string()); }
        }
    }
    if !num.is_empty() { tokens.push(num); }
    Ok(tokens)
}

fn parse_expr(tokens: &[String], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_num(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos].as_str() {
            "+" => { *pos += 1; left += parse_num(tokens, pos)?; } // BUG: should parse_term, not parse_num
            "-" => { *pos += 1; left -= parse_num(tokens, pos)?; }
            "*" => { *pos += 1; left *= parse_num(tokens, pos)?; } // BUG: wrong precedence level
            "/" => { *pos += 1; let r = parse_num(tokens, pos)?; if r == 0.0 { return Err("div by zero".into()); } left /= r; }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_num(tokens: &[String], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() { return Err("unexpected end".into()); }
    let s = &tokens[*pos];
    *pos += 1;
    s.parse::<f64>().map_err(|_| format!("not a number: {}", s))
}"#,
            r#"use opus_bugfix_parser::*;
#[test] fn simple_add() { assert_eq!(eval("1 + 2").unwrap(), 3.0); }
#[test] fn precedence() { assert_eq!(eval("2 + 3 * 4").unwrap(), 14.0); }
#[test] fn left_assoc() { assert_eq!(eval("10 - 3 - 2").unwrap(), 5.0); }
#[test] fn mul_div() { assert_eq!(eval("12 / 3 * 2").unwrap(), 8.0); }
#[test] fn complex() { assert_eq!(eval("1 + 2 * 3 + 4").unwrap(), 11.0); }
#[test] fn single() { assert_eq!(eval("42").unwrap(), 42.0); }
#[test] fn div_zero() { assert!(eval("1 / 0").is_err()); }"#),

        problem("opus-bugfix-cache", "tier2",
            "This memoization cache has a bug — it doesn't distinguish between different function arguments. Fix it.",
            r#"use std::collections::HashMap;

pub struct MemoCache {
    cache: HashMap<String, i64>,
}

impl MemoCache {
    pub fn new() -> Self { MemoCache { cache: HashMap::new() } }

    pub fn fibonacci(&mut self, n: u32) -> i64 {
        let key = "fib".to_string(); // BUG: key doesn't include n
        if let Some(&v) = self.cache.get(&key) { return v; }
        let result = if n <= 1 { n as i64 } else {
            self.fibonacci(n - 1) + self.fibonacci(n - 2)
        };
        self.cache.insert(key, result);
        result
    }

    pub fn factorial(&mut self, n: u32) -> i64 {
        let key = "fact".to_string(); // BUG: same issue
        if let Some(&v) = self.cache.get(&key) { return v; }
        let result = if n <= 1 { 1 } else { n as i64 * self.factorial(n - 1) };
        self.cache.insert(key, result);
        result
    }

    pub fn cache_size(&self) -> usize { self.cache.len() }
}"#,
            r#"use opus_bugfix_cache::*;
#[test] fn fib_basic() { let mut c = MemoCache::new(); assert_eq!(c.fibonacci(10), 55); }
#[test] fn fib_different_args() { let mut c = MemoCache::new(); assert_eq!(c.fibonacci(5), 5); assert_eq!(c.fibonacci(10), 55); }
#[test] fn fact_basic() { let mut c = MemoCache::new(); assert_eq!(c.factorial(5), 120); }
#[test] fn fact_different() { let mut c = MemoCache::new(); assert_eq!(c.factorial(3), 6); assert_eq!(c.factorial(5), 120); }
#[test] fn mixed() { let mut c = MemoCache::new(); assert_eq!(c.fibonacci(5), 5); assert_eq!(c.factorial(5), 120); }
#[test] fn caching_works() { let mut c = MemoCache::new(); c.fibonacci(10); assert!(c.cache_size() > 1); }"#),

        problem("opus-bugfix-state-machine", "tier2",
            "This state machine for parsing quoted strings has bugs with escape sequences. Fix it.",
            r#"#[derive(Debug, PartialEq)]
pub enum Token { Text(String), Quoted(String) }

pub fn parse_tokens(input: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;

    for c in input.chars() {
        if escaped {
            current.push(c);
            escaped = false; // BUG: doesn't handle \\ correctly (double backslash)
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' if in_quote => {
                tokens.push(Token::Quoted(current.clone()));
                current.clear();
                in_quote = false;
            }
            '"' => {
                if !current.is_empty() {
                    tokens.push(Token::Text(current.clone()));
                    current.clear();
                }
                in_quote = true;
            }
            ' ' if !in_quote => {
                if !current.is_empty() {
                    tokens.push(Token::Text(current.clone()));
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(if in_quote { Token::Quoted(current) } else { Token::Text(current) });
    }
    tokens
}"#,
            r#"use opus_bugfix_state_machine::*;
#[test] fn simple_text() { assert_eq!(parse_tokens("hello world"), vec![Token::Text("hello".into()), Token::Text("world".into())]); }
#[test] fn quoted() { assert_eq!(parse_tokens("\"hello world\""), vec![Token::Quoted("hello world".into())]); }
#[test] fn mixed() { assert_eq!(parse_tokens("say \"hello world\""), vec![Token::Text("say".into()), Token::Quoted("hello world".into())]); }
#[test] fn escaped_quote() { assert_eq!(parse_tokens("\"he said \\\"hi\\\"\""), vec![Token::Quoted("he said \"hi\"".into())]); }
#[test] fn escaped_backslash() { assert_eq!(parse_tokens("\"path\\\\file\""), vec![Token::Quoted("path\\file".into())]); }
#[test] fn empty() { assert_eq!(parse_tokens(""), Vec::<Token>::new()); }"#),
    ]
}
