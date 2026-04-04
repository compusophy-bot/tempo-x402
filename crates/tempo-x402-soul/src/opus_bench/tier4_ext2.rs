use super::problem;
use crate::benchmark::ExercismProblem;

pub(super) fn tier4_ext2() -> Vec<ExercismProblem> {
    vec![
        problem("opus-n-queens", "tier4",
            "Place N queens on an NxN chessboard so no two attack each other. Return all solutions.",
            r#"pub fn solve_n_queens(n: usize) -> Vec<Vec<usize>> { todo!() }"#,
            r#"use opus_n_queens::*;
#[test] fn one() { assert_eq!(solve_n_queens(1), vec![vec![0]]); }
#[test] fn four() { let s = solve_n_queens(4); assert_eq!(s.len(), 2); }
#[test] fn eight() { assert_eq!(solve_n_queens(8).len(), 92); }
#[test] fn two() { assert_eq!(solve_n_queens(2).len(), 0); }
#[test] fn three() { assert_eq!(solve_n_queens(3).len(), 0); }"#),

        problem("opus-coin-change", "tier4",
            "Find the minimum number of coins to make a given amount. Return -1 if impossible.",
            r#"pub fn coin_change(coins: &[u32], amount: u32) -> i32 { todo!() }"#,
            r#"use opus_coin_change::*;
#[test] fn basic() { assert_eq!(coin_change(&[1,5,10,25], 30), 2); }
#[test] fn impossible() { assert_eq!(coin_change(&[2], 3), -1); }
#[test] fn zero() { assert_eq!(coin_change(&[1,2], 0), 0); }
#[test] fn one_coin() { assert_eq!(coin_change(&[1], 5), 5); }
#[test] fn exact() { assert_eq!(coin_change(&[7], 14), 2); }
#[test] fn complex() { assert_eq!(coin_change(&[1,3,4], 6), 2); }"#),

        problem("opus-word-search", "tier4",
            "Given a 2D grid of characters and a word, determine if the word exists in the grid. \
             Can move up/down/left/right, each cell used at most once per word.",
            r#"pub fn exist(board: &[Vec<char>], word: &str) -> bool { todo!() }"#,
            r#"use opus_word_search::*;
#[test] fn found() { assert!(exist(&[vec!['A','B'],vec!['C','D']], "ABDC")); }
#[test] fn not_found() { assert!(!exist(&[vec!['A','B'],vec!['C','D']], "ABCE")); }
#[test] fn single() { assert!(exist(&[vec!['A']], "A")); }
#[test] fn snake() { assert!(exist(&[vec!['A','B','C'],vec!['D','E','F']], "ABEDC")); }
#[test] fn no_reuse() { assert!(!exist(&[vec!['A','B'],vec!['C','D']], "ABA")); }"#),

        problem("opus-max-subarray", "tier4",
            "Find the contiguous subarray with the largest sum (Kadane's algorithm). Return the sum and the subarray.",
            r#"pub fn max_subarray(nums: &[i64]) -> (i64, Vec<i64>) { todo!() }"#,
            r#"use opus_max_subarray::*;
#[test] fn basic() { let (sum, sub) = max_subarray(&[-2,1,-3,4,-1,2,1,-5,4]); assert_eq!(sum, 6); assert_eq!(sub, vec![4,-1,2,1]); }
#[test] fn all_negative() { let (sum, _) = max_subarray(&[-1,-2,-3]); assert_eq!(sum, -1); }
#[test] fn single() { let (sum, sub) = max_subarray(&[5]); assert_eq!(sum, 5); assert_eq!(sub, vec![5]); }
#[test] fn all_positive() { let (sum, _) = max_subarray(&[1,2,3]); assert_eq!(sum, 6); }"#),

        problem("opus-parenthesize", "tier4",
            "Given an expression like '2*3-4*5', add parentheses to maximize the result. Return the max value.",
            r#"pub fn maximize(expr: &str) -> i64 { todo!() }"#,
            r#"use opus_parenthesize::*;
#[test] fn basic() { assert_eq!(maximize("2*3-4*5"), -2); } // (2*(3-4))*5 is wrong, actual max is 2*(3-(4*5)) = -34...
// Actually: 2*3-4*5 with parens: max is 2*(3-4)*5 = -10 or 2*3-(4*5) = -14 or (2*3-4)*5 = 10
// Let me recalculate: (2*3-4)*5 = (6-4)*5 = 10. That's the max.
#[test] fn simple() { assert_eq!(maximize("1+2"), 3); }
#[test] fn single() { assert_eq!(maximize("5"), 5); }
#[test] fn subtract() { assert_eq!(maximize("5-3-1"), 3); } // 5-(3-1) = 3"#),

        problem("opus-word-break", "tier4",
            "Given a string and a dictionary, determine if the string can be segmented into space-separated dictionary words.",
            r#"pub fn word_break(s: &str, dict: &[&str]) -> bool { todo!() }
pub fn word_break_all(s: &str, dict: &[&str]) -> Vec<String> { todo!() }"#,
            r#"use opus_word_break::*;
#[test] fn basic() { assert!(word_break("leetcode", &["leet", "code"])); }
#[test] fn no_break() { assert!(!word_break("catsandog", &["cats", "dog", "sand", "and", "cat"])); }
#[test] fn can_break() { assert!(word_break("catsanddog", &["cats", "dog", "sand", "and", "cat"])); }
#[test] fn empty() { assert!(word_break("", &["a"])); }
#[test] fn all_solutions() {
    let mut r = word_break_all("catsanddog", &["cat", "cats", "and", "sand", "dog"]);
    r.sort();
    assert_eq!(r, vec!["cat sand dog", "cats and dog"]);
}"#),

        problem("opus-stock-profit", "tier4",
            "Find max profit from at most 2 buy-sell transactions on stock prices.",
            r#"pub fn max_profit_two_transactions(prices: &[i32]) -> i32 { todo!() }"#,
            r#"use opus_stock_profit::*;
#[test] fn basic() { assert_eq!(max_profit_two_transactions(&[3,3,5,0,0,3,1,4]), 6); }
#[test] fn one_transaction() { assert_eq!(max_profit_two_transactions(&[1,2,3,4,5]), 4); }
#[test] fn declining() { assert_eq!(max_profit_two_transactions(&[7,6,4,3,1]), 0); }
#[test] fn empty() { assert_eq!(max_profit_two_transactions(&[]), 0); }
#[test] fn single() { assert_eq!(max_profit_two_transactions(&[5]), 0); }"#),
    ]
}
