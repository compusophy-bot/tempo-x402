use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier1_ext4() -> Vec<BenchmarkProblem> {
    vec![
        problem("opus-gcd-lcm", "tier1", "Compute GCD and LCM of two numbers. Also compute GCD of a list.",
            r#"pub fn gcd(a: u64, b: u64) -> u64 { todo!() }
pub fn lcm(a: u64, b: u64) -> u64 { todo!() }
pub fn gcd_list(nums: &[u64]) -> u64 { todo!() }"#,
            r#"use opus_gcd_lcm::*;
#[test] fn gcd_basic() { assert_eq!(gcd(12, 8), 4); }
#[test] fn gcd_prime() { assert_eq!(gcd(7, 13), 1); }
#[test] fn gcd_same() { assert_eq!(gcd(5, 5), 5); }
#[test] fn gcd_zero() { assert_eq!(gcd(0, 5), 5); }
#[test] fn lcm_basic() { assert_eq!(lcm(4, 6), 12); }
#[test] fn lcm_coprime() { assert_eq!(lcm(3, 7), 21); }
#[test] fn gcd_list_test() { assert_eq!(gcd_list(&[12, 8, 4]), 4); }
#[test] fn gcd_list_single() { assert_eq!(gcd_list(&[7]), 7); }"#),

        problem("opus-permutations", "tier1", "Generate all permutations of a slice.",
            r#"pub fn permutations<T: Clone + Ord>(items: &[T]) -> Vec<Vec<T>> { todo!() }"#,
            r#"use opus_permutations::*;
#[test] fn empty() { assert_eq!(permutations::<i32>(&[]), vec![vec![]]); }
#[test] fn single() { assert_eq!(permutations(&[1]), vec![vec![1]]); }
#[test] fn two() { let mut r = permutations(&[1,2]); r.sort(); assert_eq!(r, vec![vec![1,2], vec![2,1]]); }
#[test] fn three() { assert_eq!(permutations(&[1,2,3]).len(), 6); }
#[test] fn four() { assert_eq!(permutations(&[1,2,3,4]).len(), 24); }"#),

        problem("opus-combinations", "tier1", "Generate all k-combinations from a slice.",
            r#"pub fn combinations<T: Clone>(items: &[T], k: usize) -> Vec<Vec<T>> { todo!() }"#,
            r#"use opus_combinations::*;
#[test] fn choose_2() { assert_eq!(combinations(&[1,2,3], 2).len(), 3); }
#[test] fn choose_0() { assert_eq!(combinations(&[1,2,3], 0), vec![vec![]]); }
#[test] fn choose_all() { assert_eq!(combinations(&[1,2,3], 3).len(), 1); }
#[test] fn choose_1() { assert_eq!(combinations(&[1,2,3,4], 1).len(), 4); }
#[test] fn choose_too_many() { assert_eq!(combinations(&[1,2], 3).len(), 0); }"#),

        problem("opus-binary-to-text", "tier1", "Convert between binary string and text. Each char = 8 bits.",
            r#"pub fn text_to_binary(s: &str) -> String { todo!() }
pub fn binary_to_text(b: &str) -> Result<String, String> { todo!() }"#,
            r#"use opus_binary_to_text::*;
#[test] fn to_bin() { assert_eq!(text_to_binary("A"), "01000001"); }
#[test] fn to_bin_hi() { assert_eq!(text_to_binary("Hi"), "0100100001101001"); }
#[test] fn to_bin_empty() { assert_eq!(text_to_binary(""), ""); }
#[test] fn from_bin() { assert_eq!(binary_to_text("01000001").unwrap(), "A"); }
#[test] fn roundtrip() { assert_eq!(binary_to_text(&text_to_binary("Hello")).unwrap(), "Hello"); }
#[test] fn invalid_len() { assert!(binary_to_text("1010").is_err()); }"#),

        problem("opus-morse-code", "tier1", "Encode and decode Morse code. Letters separated by space, words by ' / '.",
            r#"pub fn to_morse(text: &str) -> String { todo!() }
pub fn from_morse(morse: &str) -> String { todo!() }"#,
            r#"use opus_morse_code::*;
#[test] fn sos() { assert_eq!(to_morse("SOS"), "... --- ..."); }
#[test] fn hello() { assert_eq!(to_morse("HELLO"), ".... . .-.. .-.. ---"); }
#[test] fn words() { assert_eq!(to_morse("HI MOM"), ".... .. / -- --- --"); }
#[test] fn decode_sos() { assert_eq!(from_morse("... --- ..."), "SOS"); }
#[test] fn roundtrip() { assert_eq!(from_morse(&to_morse("TEST")), "TEST"); }
#[test] fn empty() { assert_eq!(to_morse(""), ""); }"#),

        problem("opus-fizzbuzz", "tier1", "FizzBuzz but return a Vec<String>. Multiples of 3→Fizz, 5→Buzz, both→FizzBuzz.",
            r#"pub fn fizzbuzz(n: u32) -> Vec<String> { todo!() }"#,
            r#"use opus_fizzbuzz::*;
#[test] fn fifteen() { let r = fizzbuzz(15); assert_eq!(r[0], "1"); assert_eq!(r[2], "Fizz"); assert_eq!(r[4], "Buzz"); assert_eq!(r[14], "FizzBuzz"); }
#[test] fn one() { assert_eq!(fizzbuzz(1), vec!["1"]); }
#[test] fn zero() { assert!(fizzbuzz(0).is_empty()); }
#[test] fn length() { assert_eq!(fizzbuzz(20).len(), 20); }"#),

        problem("opus-matrix-det", "tier1", "Compute the determinant of a square matrix using cofactor expansion.",
            r#"pub fn determinant(matrix: &[Vec<f64>]) -> f64 { todo!() }"#,
            r#"use opus_matrix_det::*;
#[test] fn one_by_one() { assert_eq!(determinant(&[vec![5.0]]), 5.0); }
#[test] fn two_by_two() { assert_eq!(determinant(&[vec![1.0,2.0],vec![3.0,4.0]]), -2.0); }
#[test] fn three_by_three() { let m = vec![vec![1.0,2.0,3.0],vec![4.0,5.0,6.0],vec![7.0,8.0,0.0]]; assert!((determinant(&m) - 27.0).abs() < 0.001); }
#[test] fn identity() { let m = vec![vec![1.0,0.0],vec![0.0,1.0]]; assert_eq!(determinant(&m), 1.0); }
#[test] fn singular() { let m = vec![vec![1.0,2.0],vec![2.0,4.0]]; assert_eq!(determinant(&m), 0.0); }"#),

        problem("opus-levenshtein", "tier1", "Compute the Levenshtein edit distance between two strings.",
            r#"pub fn distance(a: &str, b: &str) -> usize { todo!() }"#,
            r#"use opus_levenshtein::*;
#[test] fn same() { assert_eq!(distance("kitten", "kitten"), 0); }
#[test] fn classic() { assert_eq!(distance("kitten", "sitting"), 3); }
#[test] fn empty_a() { assert_eq!(distance("", "abc"), 3); }
#[test] fn empty_b() { assert_eq!(distance("abc", ""), 3); }
#[test] fn both_empty() { assert_eq!(distance("", ""), 0); }
#[test] fn single_char() { assert_eq!(distance("a", "b"), 1); }"#),

        problem("opus-topological-sort", "tier1", "Topological sort of a DAG. Return None if cycle detected.",
            r#"pub fn topo_sort(nodes: usize, edges: &[(usize, usize)]) -> Option<Vec<usize>> { todo!() }"#,
            r#"use opus_topological_sort::*;
#[test] fn linear() { assert_eq!(topo_sort(3, &[(0,1),(1,2)]), Some(vec![0,1,2])); }
#[test] fn diamond() { let r = topo_sort(4, &[(0,1),(0,2),(1,3),(2,3)]).unwrap(); assert_eq!(r[0], 0); assert_eq!(r[3], 3); }
#[test] fn cycle() { assert_eq!(topo_sort(3, &[(0,1),(1,2),(2,0)]), None); }
#[test] fn no_edges() { let r = topo_sort(3, &[]).unwrap(); assert_eq!(r.len(), 3); }
#[test] fn single() { assert_eq!(topo_sort(1, &[]), Some(vec![0])); }"#),

        problem("opus-balanced-bst", "tier1", "Build a balanced BST from a sorted array. Return the level-order traversal.",
            r#"pub fn balanced_bst(sorted: &[i32]) -> Vec<i32> { todo!() }"#,
            r#"use opus_balanced_bst::*;
#[test] fn basic() { let r = balanced_bst(&[1,2,3,4,5,6,7]); assert_eq!(r[0], 4); assert_eq!(r.len(), 7); }
#[test] fn single() { assert_eq!(balanced_bst(&[1]), vec![1]); }
#[test] fn empty() { assert!(balanced_bst(&[]).is_empty()); }
#[test] fn two() { let r = balanced_bst(&[1,2]); assert_eq!(r.len(), 2); }"#),

        problem("opus-rain-water", "tier1", "Compute how much rain water can be trapped between bars of given heights.",
            r#"pub fn trap(heights: &[u32]) -> u32 { todo!() }"#,
            r#"use opus_rain_water::*;
#[test] fn basic() { assert_eq!(trap(&[0,1,0,2,1,0,1,3,2,1,2,1]), 6); }
#[test] fn flat() { assert_eq!(trap(&[3,3,3]), 0); }
#[test] fn empty() { assert_eq!(trap(&[]), 0); }
#[test] fn mountain() { assert_eq!(trap(&[0,1,2,3,2,1,0]), 0); }
#[test] fn valley() { assert_eq!(trap(&[3,0,3]), 3); }"#),

        problem("opus-nth-prime", "tier1", "Find the nth prime number (1-indexed: 1st prime = 2).",
            r#"pub fn nth_prime(n: u32) -> u64 { todo!() }"#,
            r#"use opus_nth_prime::*;
#[test] fn first() { assert_eq!(nth_prime(1), 2); }
#[test] fn second() { assert_eq!(nth_prime(2), 3); }
#[test] fn sixth() { assert_eq!(nth_prime(6), 13); }
#[test] fn hundredth() { assert_eq!(nth_prime(100), 541); }
#[test] fn tenth() { assert_eq!(nth_prime(10), 29); }"#),

        problem("opus-spiral-walk", "tier1", "Given a 2D grid, return elements in spiral order (clockwise from top-left).",
            r#"pub fn spiral_order(grid: &[Vec<i32>]) -> Vec<i32> { todo!() }"#,
            r#"use opus_spiral_walk::*;
#[test] fn square() { assert_eq!(spiral_order(&[vec![1,2,3],vec![4,5,6],vec![7,8,9]]), vec![1,2,3,6,9,8,7,4,5]); }
#[test] fn rect() { assert_eq!(spiral_order(&[vec![1,2,3,4],vec![5,6,7,8]]), vec![1,2,3,4,8,7,6,5]); }
#[test] fn single_row() { assert_eq!(spiral_order(&[vec![1,2,3]]), vec![1,2,3]); }
#[test] fn single_col() { assert_eq!(spiral_order(&[vec![1],vec![2],vec![3]]), vec![1,2,3]); }
#[test] fn one() { assert_eq!(spiral_order(&[vec![1]]), vec![1]); }
#[test] fn empty() { assert_eq!(spiral_order(&[]), Vec::<i32>::new()); }"#),

        problem("opus-path-sum", "tier1", "Given a binary tree as an array (1-indexed, children at 2i and 2i+1), find if any root-to-leaf path sums to target.",
            r#"pub fn has_path_sum(tree: &[Option<i32>], target: i32) -> bool { todo!() }"#,
            r#"use opus_path_sum::*;
#[test] fn basic() { assert!(has_path_sum(&[None, Some(5), Some(4), Some(8), Some(11), None, Some(13), Some(4)], 22)); }
#[test] fn no_path() { assert!(!has_path_sum(&[None, Some(1), Some(2), Some(3)], 10)); }
#[test] fn single_match() { assert!(has_path_sum(&[None, Some(5)], 5)); }
#[test] fn single_no_match() { assert!(!has_path_sum(&[None, Some(5)], 3)); }
#[test] fn empty() { assert!(!has_path_sum(&[], 0)); }"#),

        problem("opus-count-islands", "tier1", "Count the number of islands in a 2D grid (1=land, 0=water). Connected horizontally/vertically.",
            r#"pub fn count_islands(grid: &[Vec<u8>]) -> usize { todo!() }"#,
            r#"use opus_count_islands::*;
#[test] fn one_island() { assert_eq!(count_islands(&[vec![1,1],vec![1,1]]), 1); }
#[test] fn two_islands() { assert_eq!(count_islands(&[vec![1,0],vec![0,1]]), 2); }
#[test] fn none() { assert_eq!(count_islands(&[vec![0,0],vec![0,0]]), 0); }
#[test] fn complex() { assert_eq!(count_islands(&[vec![1,1,0,0,0],vec![1,1,0,0,0],vec![0,0,1,0,0],vec![0,0,0,1,1]]), 3); }
#[test] fn empty() { assert_eq!(count_islands(&[]), 0); }"#),
    ]
}
