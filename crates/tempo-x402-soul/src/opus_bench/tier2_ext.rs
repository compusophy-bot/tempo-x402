use super::problem;
use crate::benchmark::BenchmarkProblem;

/// Extended Tier 2: Debugging — find and fix bugs from failing tests.
pub(super) fn tier2_ext() -> Vec<BenchmarkProblem> {
    vec![
        problem(
            "opus-bugfix-binary-search",
            "tier2",
            "This binary search has an off-by-one error causing it to miss elements at the boundaries. Fix it.",
            r#"pub fn binary_search(arr: &[i32], target: i32) -> Option<usize> {
    if arr.is_empty() { return None; }
    let mut lo = 0;
    let mut hi = arr.len(); // BUG: should be arr.len() - 1
    while lo < hi {
        let mid = (lo + hi) / 2;
        if arr[mid] == target { return Some(mid); }
        if arr[mid] < target { lo = mid + 1; }
        else { hi = mid - 1; } // BUG: this can underflow with usize
    }
    None // BUG: misses the case where lo == hi and arr[lo] == target
}"#,
            r#"use opus_bugfix_binary_search::*;

#[test]
fn find_middle() { assert_eq!(binary_search(&[1,3,5,7,9], 5), Some(2)); }
#[test]
fn find_first() { assert_eq!(binary_search(&[1,3,5,7,9], 1), Some(0)); }
#[test]
fn find_last() { assert_eq!(binary_search(&[1,3,5,7,9], 9), Some(4)); }
#[test]
fn not_found() { assert_eq!(binary_search(&[1,3,5,7,9], 4), None); }
#[test]
fn empty() { assert_eq!(binary_search(&[], 1), None); }
#[test]
fn single_found() { assert_eq!(binary_search(&[5], 5), Some(0)); }
#[test]
fn single_not_found() { assert_eq!(binary_search(&[5], 3), None); }
#[test]
fn two_elements() { assert_eq!(binary_search(&[1, 3], 3), Some(1)); }"#,
        ),
        problem(
            "opus-bugfix-merge-sort",
            "tier2",
            "This merge sort corrupts the output on certain inputs. The merge step has an index error. Fix it.",
            r#"pub fn merge_sort(arr: &mut Vec<i32>) {
    let len = arr.len();
    if len <= 1 { return; }
    let mid = len / 2;
    let mut left = arr[..mid].to_vec();
    let mut right = arr[mid..].to_vec();
    merge_sort(&mut left);
    merge_sort(&mut right);
    // Merge
    let (mut i, mut j, mut k) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        if left[i] <= right[j] {
            arr[k] = left[i];
            i += 1;
        } else {
            arr[k] = right[j];
            j += 1;
        }
        k += 1;
    }
    while i < left.len() { arr[k] = left[i]; i += 1; } // BUG: missing k += 1
    while j < right.len() { arr[k] = right[j]; j += 1; } // BUG: missing k += 1
}"#,
            r#"use opus_bugfix_merge_sort::*;

#[test]
fn basic() { let mut v = vec![3,1,2]; merge_sort(&mut v); assert_eq!(v, vec![1,2,3]); }
#[test]
fn already_sorted() { let mut v = vec![1,2,3]; merge_sort(&mut v); assert_eq!(v, vec![1,2,3]); }
#[test]
fn reversed() { let mut v = vec![5,4,3,2,1]; merge_sort(&mut v); assert_eq!(v, vec![1,2,3,4,5]); }
#[test]
fn duplicates() { let mut v = vec![3,1,3,1]; merge_sort(&mut v); assert_eq!(v, vec![1,1,3,3]); }
#[test]
fn single() { let mut v = vec![1]; merge_sort(&mut v); assert_eq!(v, vec![1]); }
#[test]
fn empty() { let mut v: Vec<i32> = vec![]; merge_sort(&mut v); assert_eq!(v, vec![] as Vec<i32>); }
#[test]
fn large() { let mut v: Vec<i32> = (0..100).rev().collect(); merge_sort(&mut v); let expected: Vec<i32> = (0..100).collect(); assert_eq!(v, expected); }"#,
        ),
        problem(
            "opus-bugfix-iterator",
            "tier2",
            "This custom iterator skips elements and panics on empty inputs. Fix both bugs.",
            r#"pub struct Chunks<'a, T> {
    data: &'a [T],
    size: usize,
    pos: usize,
}

impl<'a, T> Chunks<'a, T> {
    pub fn new(data: &'a [T], size: usize) -> Self {
        Chunks { data, size: size.max(1), pos: 1 } // BUG: pos should start at 0
    }
}

impl<'a, T> Iterator for Chunks<'a, T> {
    type Item = &'a [T];
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() { return None; }
        let end = (self.pos + self.size).min(self.data.len());
        let chunk = &self.data[self.pos..end];
        self.pos = end + 1; // BUG: should be just end, not end + 1
        Some(chunk)
    }
}"#,
            r#"use opus_bugfix_iterator::*;

#[test]
fn basic_chunks() {
    let data = vec![1,2,3,4,5];
    let chunks: Vec<&[i32]> = Chunks::new(&data, 2).collect();
    assert_eq!(chunks, vec![&[1,2][..], &[3,4][..], &[5][..]]);
}
#[test]
fn exact_chunks() {
    let data = vec![1,2,3,4];
    let chunks: Vec<&[i32]> = Chunks::new(&data, 2).collect();
    assert_eq!(chunks, vec![&[1,2][..], &[3,4][..]]);
}
#[test]
fn chunk_size_one() {
    let data = vec![1,2,3];
    let chunks: Vec<&[i32]> = Chunks::new(&data, 1).collect();
    assert_eq!(chunks.len(), 3);
}
#[test]
fn chunk_larger_than_data() {
    let data = vec![1,2];
    let chunks: Vec<&[i32]> = Chunks::new(&data, 10).collect();
    assert_eq!(chunks, vec![&[1,2][..]]);
}
#[test]
fn empty_data() {
    let data: Vec<i32> = vec![];
    let chunks: Vec<&[i32]> = Chunks::new(&data, 3).collect();
    assert_eq!(chunks.len(), 0);
}"#,
        ),
        problem(
            "opus-bugfix-hashmap",
            "tier2",
            "This custom open-addressing HashMap has a probe sequence bug causing infinite loops on full tables. Fix it.",
            r#"pub struct SimpleMap {
    keys: Vec<Option<String>>,
    values: Vec<Option<i32>>,
    capacity: usize,
    len: usize,
}

impl SimpleMap {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        SimpleMap {
            keys: vec![None; cap],
            values: vec![None; cap],
            capacity: cap,
            len: 0,
        }
    }
    pub fn insert(&mut self, key: String, value: i32) {
        let mut idx = self.hash(&key);
        loop { // BUG: no check for full table — infinite loop
            match &self.keys[idx] {
                None => {
                    self.keys[idx] = Some(key);
                    self.values[idx] = Some(value);
                    self.len += 1;
                    return;
                }
                Some(k) if k == &key => { // BUG: missing & comparison
                    self.values[idx] = Some(value);
                    return;
                }
                _ => idx = (idx + 1) % self.capacity,
            }
        }
    }
    pub fn get(&self, key: &str) -> Option<i32> {
        let mut idx = self.hash(key);
        for _ in 0..self.capacity {
            match &self.keys[idx] {
                Some(k) if k == key => return self.values[idx],
                None => return None,
                _ => idx = (idx + 1) % self.capacity,
            }
        }
        None
    }
    pub fn len(&self) -> usize { self.len }
    fn hash(&self, key: &str) -> usize {
        key.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize)) % self.capacity
    }
}"#,
            r#"use opus_bugfix_hashmap::*;

#[test]
fn basic() {
    let mut m = SimpleMap::new(8);
    m.insert("a".into(), 1);
    assert_eq!(m.get("a"), Some(1));
}
#[test]
fn overwrite() {
    let mut m = SimpleMap::new(8);
    m.insert("a".into(), 1);
    m.insert("a".into(), 2);
    assert_eq!(m.get("a"), Some(2));
    assert_eq!(m.len(), 1);
}
#[test]
fn collision() {
    let mut m = SimpleMap::new(4);
    m.insert("a".into(), 1);
    m.insert("b".into(), 2);
    m.insert("c".into(), 3);
    assert_eq!(m.get("a"), Some(1));
    assert_eq!(m.get("b"), Some(2));
    assert_eq!(m.get("c"), Some(3));
}
#[test]
fn not_found() {
    let m = SimpleMap::new(8);
    assert_eq!(m.get("missing"), None);
}
#[test]
fn near_full() {
    let mut m = SimpleMap::new(4);
    m.insert("a".into(), 1);
    m.insert("b".into(), 2);
    m.insert("c".into(), 3);
    // Should not infinite loop even when table is 75% full
    assert_eq!(m.len(), 3);
    assert_eq!(m.get("c"), Some(3));
}"#,
        ),
        problem(
            "opus-bugfix-lifetimes",
            "tier2",
            "This code has lifetime errors. The split_first function tries to return references \
             into a local variable. Fix it so it compiles and works correctly.",
            r#"pub fn longest_word(text: &str) -> &str {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() { return ""; }
    let mut longest = words[0];
    for &w in &words[1..] {
        if w.len() > longest.len() { longest = w; }
    }
    longest
}

pub fn first_n_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn trim_and_lower(s: &str) -> String {
    s.trim().to_lowercase()
}"#,
            r#"use opus_bugfix_lifetimes::*;

#[test]
fn longest() { assert_eq!(longest_word("the quick brown fox"), "quick"); }
#[test]
fn longest_single() { assert_eq!(longest_word("hello"), "hello"); }
#[test]
fn longest_empty() { assert_eq!(longest_word(""), ""); }
#[test]
fn first_n() { assert_eq!(first_n_chars("hello", 3), "hel"); }
#[test]
fn first_n_unicode() { assert_eq!(first_n_chars("héllo", 2), "hé"); }
#[test]
fn first_n_over() { assert_eq!(first_n_chars("hi", 10), "hi"); }
#[test]
fn trim_lower() { assert_eq!(trim_and_lower("  Hello World  "), "hello world"); }"#,
        ),
    ]
}
