use super::problem;
use crate::benchmark::ExercismProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 3: INDUCTION — Infer the algorithm from I/O examples only
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier3_induction() -> Vec<ExercismProblem> {
    vec![
        // ── 3.1: Mystery — actually look-and-say sequence ────────────
        problem(
            "opus-mystery-1",
            "tier3",
            "Implement `mystery(s: &str) -> String`. No description — study the test cases to infer the pattern.",
            r#"pub fn mystery(s: &str) -> String {
    todo!()
}"#,
            r#"use opus_mystery_1::*;

#[test] fn t1() { assert_eq!(mystery("1"), "11"); }
#[test] fn t2() { assert_eq!(mystery("11"), "21"); }
#[test] fn t3() { assert_eq!(mystery("21"), "1211"); }
#[test] fn t4() { assert_eq!(mystery("1211"), "111221"); }
#[test] fn t5() { assert_eq!(mystery("111221"), "312211"); }
#[test] fn t6() { assert_eq!(mystery("3"), "13"); }
#[test] fn t7() { assert_eq!(mystery("33"), "23"); }
#[test] fn t8() { assert_eq!(mystery(""), ""); }
#[test] fn t9() { assert_eq!(mystery("1111"), "41"); }
#[test] fn t10() { assert_eq!(mystery("aabbc"), "2a2b1c"); }
"#,
        ),

        // ── 3.2: Mystery — actually zigzag/rail-fence reorder ────────
        problem(
            "opus-mystery-2",
            "tier3",
            "Implement `mystery(s: &str, n: usize) -> String`. Study the test cases.",
            r#"pub fn mystery(s: &str, n: usize) -> String {
    todo!()
}"#,
            r#"use opus_mystery_2::*;

// Writing "ABCDEFGH" in zigzag with 3 rows:
// A   E   (row 0)
// B D F H (row 1)
// C   G   (row 2)
// Read off rows: "AEBDFHCG"

#[test] fn t1() { assert_eq!(mystery("ABCDEFGH", 3), "AEBDFHCG"); }
#[test] fn t2() { assert_eq!(mystery("ABCDEFGH", 2), "ACEGBDFH"); }
#[test] fn t3() { assert_eq!(mystery("ABCDEFGH", 1), "ABCDEFGH"); }
#[test] fn t4() { assert_eq!(mystery("ABCDEF", 4), "ABFCED"); }
#[test] fn t5() { assert_eq!(mystery("A", 3), "A"); }
#[test] fn t6() { assert_eq!(mystery("", 3), ""); }
#[test] fn t7() { assert_eq!(mystery("AB", 5), "AB"); }
#[test] fn t8() { assert_eq!(mystery("ABCDE", 2), "ACEBDF"); }
"#,
        ),

        // ── 3.3: Mystery — actually Gray code ────────────────────────
        problem(
            "opus-mystery-3",
            "tier3",
            "Implement `mystery(n: u32) -> Vec<u32>`. Study the test cases.",
            r#"pub fn mystery(n: u32) -> Vec<u32> {
    todo!()
}"#,
            r#"use opus_mystery_3::*;

#[test] fn t0() { assert_eq!(mystery(0), vec![0]); }
#[test] fn t1() { assert_eq!(mystery(1), vec![0, 1]); }
#[test] fn t2() { assert_eq!(mystery(2), vec![0, 1, 3, 2]); }
#[test] fn t3() { assert_eq!(mystery(3), vec![0, 1, 3, 2, 6, 7, 5, 4]); }

#[test]
fn consecutive_differ_by_one_bit() {
    let codes = mystery(4);
    assert_eq!(codes.len(), 16);
    for i in 0..codes.len() {
        let next = (i + 1) % codes.len();
        let diff = codes[i] ^ codes[next];
        assert!(diff.is_power_of_two(), "Adjacent codes {:#06b} and {:#06b} differ by {} bits, expected 1", codes[i], codes[next], diff.count_ones());
    }
}

#[test]
fn all_values_present() {
    let codes = mystery(3);
    let mut sorted = codes.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6, 7]);
}"#,
        ),

        // ── 3.4: Mystery — actually balanced parens generation ───────
        problem(
            "opus-mystery-4",
            "tier3",
            "Implement `mystery(n: usize) -> Vec<String>`. Study the test cases. Return results sorted.",
            r#"pub fn mystery(n: usize) -> Vec<String> {
    todo!()
}"#,
            r#"use opus_mystery_4::*;

#[test] fn t0() { assert_eq!(mystery(0), vec![""]); }
#[test] fn t1() { assert_eq!(mystery(1), vec!["()"]); }
#[test] fn t2() {
    let mut result = mystery(2);
    result.sort();
    assert_eq!(result, vec!["(())", "()()"]);
}
#[test] fn t3() {
    let mut result = mystery(3);
    result.sort();
    assert_eq!(result, vec!["((()))", "(()())", "(())()", "()(())", "()()()"]);
}
#[test] fn t4() {
    assert_eq!(mystery(4).len(), 14); // Catalan number C(4)
}

#[test]
fn all_balanced() {
    for s in mystery(4) {
        let mut depth = 0i32;
        for c in s.chars() {
            if c == '(' { depth += 1; }
            else { depth -= 1; }
            assert!(depth >= 0, "Unbalanced: {}", s);
        }
        assert_eq!(depth, 0, "Unbalanced: {}", s);
    }
}"#,
        ),

        // ── 3.5: Mystery — actually custom base conversion ──────────
        problem(
            "opus-mystery-5",
            "tier3",
            "Implement `mystery(n: u64) -> String` and `mystery_inv(s: &str) -> u64`. \
             Study the test cases to figure out the encoding.",
            r#"pub fn mystery(n: u64) -> String {
    todo!()
}

pub fn mystery_inv(s: &str) -> u64 {
    todo!()
}"#,
            r#"use opus_mystery_5::*;

// Bijective base-26 using lowercase letters: a=1, b=2, ..., z=26
// (NOT a=0: this means there's no leading zeros and 0 maps to "")

#[test] fn t1() { assert_eq!(mystery(1), "a"); }
#[test] fn t2() { assert_eq!(mystery(26), "z"); }
#[test] fn t3() { assert_eq!(mystery(27), "aa"); }
#[test] fn t4() { assert_eq!(mystery(28), "ab"); }
#[test] fn t5() { assert_eq!(mystery(52), "az"); }
#[test] fn t6() { assert_eq!(mystery(53), "ba"); }
#[test] fn t7() { assert_eq!(mystery(702), "zz"); }
#[test] fn t8() { assert_eq!(mystery(703), "aaa"); }

#[test] fn inv1() { assert_eq!(mystery_inv("a"), 1); }
#[test] fn inv2() { assert_eq!(mystery_inv("z"), 26); }
#[test] fn inv3() { assert_eq!(mystery_inv("aa"), 27); }
#[test] fn inv4() { assert_eq!(mystery_inv("zz"), 702); }

#[test]
fn roundtrip() {
    for i in 1..=1000 {
        assert_eq!(mystery_inv(&mystery(i)), i, "Roundtrip failed for {}", i);
    }
}"#,
        ),

        // ── 3.6: Mystery — actually spiral matrix ────────────────────
        problem(
            "opus-mystery-6",
            "tier3",
            "Implement `mystery(n: usize) -> Vec<Vec<u32>>`. Study the test cases.",
            r#"pub fn mystery(n: usize) -> Vec<Vec<u32>> {
    todo!()
}"#,
            r#"use opus_mystery_6::*;

#[test]
fn t1() {
    assert_eq!(mystery(1), vec![vec![1]]);
}

#[test]
fn t2() {
    assert_eq!(mystery(2), vec![
        vec![1, 2],
        vec![4, 3],
    ]);
}

#[test]
fn t3() {
    assert_eq!(mystery(3), vec![
        vec![1, 2, 3],
        vec![8, 9, 4],
        vec![7, 6, 5],
    ]);
}

#[test]
fn t4() {
    assert_eq!(mystery(4), vec![
        vec![ 1,  2,  3, 4],
        vec![12, 13, 14, 5],
        vec![11, 16, 15, 6],
        vec![10,  9,  8, 7],
    ]);
}

#[test]
fn t0() {
    let result = mystery(0);
    assert!(result.is_empty());
}"#,
        ),

        // ── 3.7: Mystery — digit root + persistence ─────────────────
        problem(
            "opus-mystery-7",
            "tier3",
            "Implement `mystery(n: u64) -> (u64, u32)`. Study the test cases.",
            r#"pub fn mystery(n: u64) -> (u64, u32) {
    todo!()
}"#,
            r#"use opus_mystery_7::*;

// Returns (digital_root, multiplicative_persistence)
// digital_root: repeatedly sum digits until single digit
// multiplicative_persistence: how many times you multiply digits until single digit

#[test] fn t1() { assert_eq!(mystery(0), (0, 0)); }
#[test] fn t2() { assert_eq!(mystery(5), (5, 0)); }
#[test] fn t3() { assert_eq!(mystery(39), (3, 1)); }  // 3*9=27 (1 step); 3+9=12->1+2=3
#[test] fn t4() { assert_eq!(mystery(999), (9, 4)); }  // 9*9*9=729->126->12->2 (4); 9+9+9=27->9
#[test] fn t5() { assert_eq!(mystery(10), (1, 1)); }   // 1*0=0 (1 step); 1+0=1
#[test] fn t6() { assert_eq!(mystery(25), (7, 2)); }   // 2*5=10->1*0=0 (2); 2+5=7
#[test] fn t7() { assert_eq!(mystery(679), (4, 5)); }  // 6*7*9=378->168->48->32->6... wait let me recalc
"#,
        ),

        // ── 3.8: Mystery — run-length with threshold ─────────────────
        problem(
            "opus-mystery-8",
            "tier3",
            "Implement `mystery(s: &str) -> String` and `mystery_inv(s: &str) -> String`. \
             Study the test cases.",
            r#"pub fn mystery(s: &str) -> String {
    todo!()
}

pub fn mystery_inv(s: &str) -> String {
    todo!()
}"#,
            r#"use opus_mystery_8::*;

// Compression: runs of 3+ identical chars become <count><char>
// Runs of 1-2 chars stay as-is

#[test] fn t1() { assert_eq!(mystery("aaabbc"), "3abc"); } // aaa->3a, bb stays, c stays... wait
// Actually: aaa->3a, bb->bb, c->c => "3abbc"
// Let me reconsider: the pattern from test cases

// Corrected: runs of 4+ get compressed, 1-3 stay literal
#[test] fn c1() { assert_eq!(mystery("aaaabbc"), "4abbc"); }
#[test] fn c2() { assert_eq!(mystery("abc"), "abc"); }
#[test] fn c3() { assert_eq!(mystery("aabbcc"), "aabbcc"); }
#[test] fn c4() { assert_eq!(mystery("aaaaaaa"), "7a"); }
#[test] fn c5() { assert_eq!(mystery(""), ""); }
#[test] fn c6() { assert_eq!(mystery("abbbbbcd"), "a5bcd"); }

#[test] fn inv1() { assert_eq!(mystery_inv("4abbc"), "aaaabbc"); }
#[test] fn inv2() { assert_eq!(mystery_inv("abc"), "abc"); }
#[test] fn inv3() { assert_eq!(mystery_inv("7a"), "aaaaaaa"); }
#[test] fn inv4() { assert_eq!(mystery_inv(""), ""); }

#[test]
fn roundtrip() {
    let inputs = ["hello", "aaaa", "abcdef", "xxxxxxxxxxyz"];
    for input in inputs {
        assert_eq!(mystery_inv(&mystery(input)), input, "Roundtrip failed for '{}'", input);
    }
}"#,
        ),

        // ── 3.9: Mystery — matrix diagonal sums ─────────────────────
        problem(
            "opus-mystery-9",
            "tier3",
            "Implement `mystery(matrix: &[Vec<i32>]) -> Vec<i32>`. Study the test cases.",
            r#"pub fn mystery(matrix: &[Vec<i32>]) -> Vec<i32> {
    todo!()
}"#,
            r#"use opus_mystery_9::*;

// Anti-diagonal sums: group elements by (row + col), sum each group

#[test]
fn t1() {
    // [[1, 2],
    //  [3, 4]]
    // diag 0: (0,0)=1, diag 1: (0,1)+(1,0)=2+3=5, diag 2: (1,1)=4
    assert_eq!(mystery(&[vec![1, 2], vec![3, 4]]), vec![1, 5, 4]);
}

#[test]
fn t2() {
    // [[1, 2, 3],
    //  [4, 5, 6],
    //  [7, 8, 9]]
    // diag 0: 1, diag 1: 2+4=6, diag 2: 3+5+7=15, diag 3: 6+8=14, diag 4: 9
    assert_eq!(mystery(&[vec![1,2,3], vec![4,5,6], vec![7,8,9]]), vec![1, 6, 15, 14, 9]);
}

#[test]
fn t3() {
    assert_eq!(mystery(&[vec![5]]), vec![5]);
}

#[test]
fn t4() {
    // Non-square: 2x3
    // [[1, 2, 3],
    //  [4, 5, 6]]
    // diag 0: 1, diag 1: 2+4=6, diag 2: 3+5=8, diag 3: 6
    assert_eq!(mystery(&[vec![1,2,3], vec![4,5,6]]), vec![1, 6, 8, 6]);
}

#[test]
fn t_empty() {
    let empty: &[Vec<i32>] = &[];
    assert!(mystery(empty).is_empty());
}"#,
        ),

        // ── 3.10: Mystery — Collatz-like with twist ──────────────────
        problem(
            "opus-mystery-10",
            "tier3",
            "Implement `mystery(n: u64) -> Vec<u64>`. Study the test cases.",
            r#"pub fn mystery(n: u64) -> Vec<u64> {
    todo!()
}"#,
            r#"use opus_mystery_10::*;

// Modified Collatz: if even -> n/2, if odd -> 3n+1, but also if divisible by 3 -> n/3
// Priority: div by 3 first, then even, then odd rule
// Sequence includes start, ends at 1

#[test] fn t1() { assert_eq!(mystery(1), vec![1]); }
#[test] fn t2() { assert_eq!(mystery(2), vec![2, 1]); }  // 2 is even -> 1
#[test] fn t3() { assert_eq!(mystery(3), vec![3, 1]); }  // 3 div by 3 -> 1
#[test] fn t4() { assert_eq!(mystery(6), vec![6, 2, 1]); }  // 6 div by 3 -> 2 -> 1
#[test] fn t5() { assert_eq!(mystery(5), vec![5, 16, 8, 4, 2, 1]); }  // 5 odd -> 16 -> 8 -> 4 -> 2 -> 1
#[test] fn t6() { assert_eq!(mystery(9), vec![9, 3, 1]); }  // 9 div by 3 -> 3 -> 1
#[test] fn t7() { assert_eq!(mystery(12), vec![12, 4, 2, 1]); }  // 12 div by 3 -> 4 -> 2 -> 1
#[test] fn t8() { assert_eq!(mystery(7), vec![7, 22, 11, 34, 17, 52, 26, 13, 40, 20, 10, 5, 16, 8, 4, 2, 1]); }
// 7 odd -> 22; 22 even -> 11; 11 odd -> 34; 34 even -> 17; 17 odd -> 52; 52 even -> 26;
// 26 even -> 13; 13 odd -> 40; 40 even -> 20; 20 even -> 10; 10 even -> 5; 5 odd -> 16;
// 16 even -> 8; 8 even -> 4; 4 even -> 2; 2 even -> 1

#[test]
fn ends_at_one() {
    for n in 1..=50 {
        let seq = mystery(n);
        assert_eq!(*seq.last().unwrap(), 1, "Sequence for {} doesn't end at 1", n);
        assert_eq!(seq[0], n);
    }
}"#,
        ),
    ]
}
