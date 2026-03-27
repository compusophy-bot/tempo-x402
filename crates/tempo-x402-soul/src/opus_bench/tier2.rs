use super::problem;
use crate::benchmark::ExercismProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 2: DEBUGGING — Find and fix the bug
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier2_debugging() -> Vec<ExercismProblem> {
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
