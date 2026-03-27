use super::problem;
use crate::benchmark::ExercismProblem;

// ══════════════════════════════════════════════════════════════════════
// TIER 5: ADVERSARIAL — Exploit known LLM failure modes
// ══════════════════════════════════════════════════════════════════════

pub(super) fn tier5_adversarial() -> Vec<ExercismProblem> {
    vec![
        // ── 5.1: Almost Fibonacci ────────────────────────────────────
        problem(
            "opus-almost-fibonacci",
            "tier5",
            "Implement `almost_fib(n: u64) -> u64` which is like Fibonacci but every number \
             at a position divisible by 5 (0-indexed: positions 0, 5, 10, ...) is DOUBLED. \
             f(0)=0, f(1)=1, then f(n) = f(n-1) + f(n-2), then if n%5==0, double the result.",
            r#"pub fn almost_fib(n: u64) -> u64 {
    todo!()
}"#,
            r#"use opus_almost_fibonacci::*;

// Standard fib: 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, ...
// Almost fib:   0, 1, 1, 2, 3, 10, 13, 23, 36, 59, 190, ...
//               ^              ^                    ^
//               doubled        doubled              doubled
// Wait — the doubling affects subsequent terms because f(n) uses f(n-1) and f(n-2)!

// Let's trace: f(0) = 0*2 = 0, f(1) = 1, f(2) = 1+0 = 1, f(3) = 1+1 = 2, f(4) = 2+1 = 3
// f(5) = (3+2)*2 = 10, f(6) = 10+3 = 13, f(7) = 13+10 = 23, f(8) = 23+13 = 36
// f(9) = 36+23 = 59, f(10) = (59+36)*2 = 190

#[test] fn f0() { assert_eq!(almost_fib(0), 0); }
#[test] fn f1() { assert_eq!(almost_fib(1), 1); }
#[test] fn f2() { assert_eq!(almost_fib(2), 1); }
#[test] fn f3() { assert_eq!(almost_fib(3), 2); }
#[test] fn f4() { assert_eq!(almost_fib(4), 3); }
#[test] fn f5() { assert_eq!(almost_fib(5), 10); }
#[test] fn f6() { assert_eq!(almost_fib(6), 13); }
#[test] fn f7() { assert_eq!(almost_fib(7), 23); }
#[test] fn f8() { assert_eq!(almost_fib(8), 36); }
#[test] fn f9() { assert_eq!(almost_fib(9), 59); }
#[test] fn f10() { assert_eq!(almost_fib(10), 190); }
"#,
        ),

        // ── 5.2: Sort by English Name ────────────────────────────────
        problem(
            "opus-english-sort",
            "tier5",
            "Sort a list of integers (0-999) by their English name alphabetically. \
             For example: 8 (eight) < 5 (five) < 4 (four) < 1 (one) < 3 (three) < 2 (two) < 0 (zero).",
            r#"pub fn sort_by_english(nums: &mut Vec<u32>) {
    todo!()
}

pub fn to_english(n: u32) -> String {
    todo!()
}"#,
            r#"use opus_english_sort::*;

#[test]
fn single_digits() {
    let mut v = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    sort_by_english(&mut v);
    assert_eq!(v, vec![8, 5, 4, 9, 1, 7, 6, 3, 2, 0]);
    // eight, five, four, nine, one, seven, six, three, two, zero
}

#[test]
fn teens() {
    let mut v = vec![11, 12, 13, 14, 15];
    sort_by_english(&mut v);
    assert_eq!(v, vec![15, 14, 13, 12, 11]);
    // fifteen, fourteen, thirteen, twelve, eleven
}

#[test]
fn to_english_basic() {
    assert_eq!(to_english(0), "zero");
    assert_eq!(to_english(1), "one");
    assert_eq!(to_english(11), "eleven");
    assert_eq!(to_english(20), "twenty");
    assert_eq!(to_english(42), "forty two");
    assert_eq!(to_english(100), "one hundred");
    assert_eq!(to_english(999), "nine hundred ninety nine");
}

#[test]
fn hundreds() {
    let mut v = vec![100, 200, 300];
    sort_by_english(&mut v);
    // "one hundred", "three hundred", "two hundred"
    assert_eq!(v, vec![100, 300, 200]);
}

#[test]
fn mixed() {
    let mut v = vec![3, 30, 300];
    sort_by_english(&mut v);
    // "three", "thirty", "three hundred"
    assert_eq!(v, vec![3, 30, 300]);
}"#,
        ),

        // ── 5.3: Base Negative Two ───────────────────────────────────
        problem(
            "opus-base-neg2",
            "tier5",
            "Convert between base -2 (negabinary) and decimal. \
             In base -2, each position i has value (-2)^i. \
             Digits are 0 and 1 only. String representation is MSB first.",
            r#"pub fn to_neg2(n: i64) -> String {
    todo!()
}

pub fn from_neg2(s: &str) -> i64 {
    todo!()
}"#,
            r#"use opus_base_neg2::*;

#[test] fn zero() { assert_eq!(to_neg2(0), "0"); }
#[test] fn one() { assert_eq!(to_neg2(1), "1"); }
#[test] fn neg_one() { assert_eq!(to_neg2(-1), "11"); }
// -1 = 1*(-2)^1 + 1*(-2)^0 = -2 + 1 = -1 ✓

#[test] fn two() { assert_eq!(to_neg2(2), "110"); }
// 2 = 1*4 + 1*(-2) + 0*1 = 4-2 = 2 ✓

#[test] fn neg_two() { assert_eq!(to_neg2(-2), "10"); }
// -2 = 1*(-2)^1 = -2 ✓

#[test] fn three() { assert_eq!(to_neg2(3), "111"); }
// 3 = 1*4 + 1*(-2) + 1*1 = 4-2+1 = 3 ✓

#[test] fn six() { assert_eq!(to_neg2(6), "11010"); }
// 6 = 1*16 + 1*(-8) + 0*4 + 1*(-2) + 0*1 = 16-8-2 = 6 ✓

#[test]
fn from_neg2_basic() {
    assert_eq!(from_neg2("0"), 0);
    assert_eq!(from_neg2("1"), 1);
    assert_eq!(from_neg2("11"), -1);
    assert_eq!(from_neg2("110"), 2);
}

#[test]
fn roundtrip() {
    for n in -50..=50 {
        assert_eq!(from_neg2(&to_neg2(n)), n, "Roundtrip failed for {}", n);
    }
}

#[test]
fn only_binary_digits() {
    for n in -100..=100 {
        let s = to_neg2(n);
        assert!(s.chars().all(|c| c == '0' || c == '1'), "Non-binary digit in '{}'", s);
    }
}"#,
        ),

        // ── 5.4: 1-Indexed Everything ────────────────────────────────
        problem(
            "opus-one-indexed",
            "tier5",
            "Implement array operations where ALL indices are 1-based (not 0-based). \
             `get(arr, i)` returns element at position i (1 = first). \
             `set(arr, i, val)` sets element at position i. \
             `slice(arr, from, to)` returns elements from..=to (inclusive on both ends). \
             `find(arr, val)` returns the 1-based index of the first occurrence, or 0 if not found.",
            r#"pub fn get(arr: &[i32], i: usize) -> Option<i32> {
    todo!()
}

pub fn set(arr: &mut Vec<i32>, i: usize, val: i32) -> bool {
    todo!()
}

pub fn slice(arr: &[i32], from: usize, to: usize) -> Vec<i32> {
    todo!()
}

pub fn find(arr: &[i32], val: i32) -> usize {
    todo!()
}"#,
            r#"use opus_one_indexed::*;

#[test]
fn get_first() {
    assert_eq!(get(&[10, 20, 30], 1), Some(10));
}

#[test]
fn get_last() {
    assert_eq!(get(&[10, 20, 30], 3), Some(30));
}

#[test]
fn get_zero_invalid() {
    assert_eq!(get(&[10, 20, 30], 0), None);
}

#[test]
fn get_out_of_bounds() {
    assert_eq!(get(&[10, 20, 30], 4), None);
}

#[test]
fn set_first() {
    let mut v = vec![10, 20, 30];
    assert!(set(&mut v, 1, 99));
    assert_eq!(v, vec![99, 20, 30]);
}

#[test]
fn set_zero_invalid() {
    let mut v = vec![10, 20, 30];
    assert!(!set(&mut v, 0, 99));
    assert_eq!(v, vec![10, 20, 30]); // unchanged
}

#[test]
fn slice_middle() {
    assert_eq!(slice(&[10, 20, 30, 40, 50], 2, 4), vec![20, 30, 40]);
}

#[test]
fn slice_single() {
    assert_eq!(slice(&[10, 20, 30], 2, 2), vec![20]);
}

#[test]
fn slice_full() {
    assert_eq!(slice(&[10, 20, 30], 1, 3), vec![10, 20, 30]);
}

#[test]
fn find_present() {
    assert_eq!(find(&[10, 20, 30], 20), 2);
}

#[test]
fn find_first() {
    assert_eq!(find(&[10, 20, 30], 10), 1);
}

#[test]
fn find_absent() {
    assert_eq!(find(&[10, 20, 30], 99), 0);
}"#,
        ),

        // ── 5.5: Unicode Trap ────────────────────────────────────────
        problem(
            "opus-unicode-trap",
            "tier5",
            "Implement string functions that handle Unicode correctly:\n\
             `char_count` — number of Unicode scalar values (NOT bytes, NOT grapheme clusters)\n\
             `byte_count` — number of UTF-8 bytes\n\
             `nth_char` — return the nth character (0-indexed by chars, not bytes)\n\
             `truncate_chars` — truncate to at most n characters, preserving valid UTF-8",
            r#"pub fn char_count(s: &str) -> usize { todo!() }
pub fn byte_count(s: &str) -> usize { todo!() }
pub fn nth_char(s: &str, n: usize) -> Option<char> { todo!() }
pub fn truncate_chars(s: &str, n: usize) -> &str { todo!() }"#,
            r#"use opus_unicode_trap::*;

#[test]
fn ascii() {
    assert_eq!(char_count("hello"), 5);
    assert_eq!(byte_count("hello"), 5);
}

#[test]
fn emoji() {
    assert_eq!(char_count("🌍"), 1);
    assert_eq!(byte_count("🌍"), 4);
}

#[test]
fn mixed() {
    // "café" — the é can be 1 char (U+00E9) encoded as 2 bytes
    let s = "caf\u{00E9}";
    assert_eq!(char_count(s), 4);
    assert_eq!(byte_count(s), 5);
}

#[test]
fn cjk() {
    let s = "日本語";
    assert_eq!(char_count(s), 3);
    assert_eq!(byte_count(s), 9); // 3 bytes per CJK char
}

#[test]
fn nth_char_ascii() {
    assert_eq!(nth_char("hello", 0), Some('h'));
    assert_eq!(nth_char("hello", 4), Some('o'));
    assert_eq!(nth_char("hello", 5), None);
}

#[test]
fn nth_char_unicode() {
    assert_eq!(nth_char("日本語", 1), Some('本'));
}

#[test]
fn truncate_ascii() {
    assert_eq!(truncate_chars("hello world", 5), "hello");
}

#[test]
fn truncate_unicode() {
    assert_eq!(truncate_chars("日本語", 2), "日本");
}

#[test]
fn truncate_beyond_length() {
    assert_eq!(truncate_chars("hi", 100), "hi");
}

#[test]
fn empty_string() {
    assert_eq!(char_count(""), 0);
    assert_eq!(byte_count(""), 0);
    assert_eq!(nth_char("", 0), None);
    assert_eq!(truncate_chars("", 5), "");
}"#,
        ),

        // ── 5.6: Floating Point Traps ────────────────────────────────
        problem(
            "opus-float-trap",
            "tier5",
            "Implement functions that handle IEEE 754 edge cases correctly:\n\
             `safe_div(a, b)` — divide, returning None for 0/0, Some(inf) for n/0\n\
             `approx_eq(a, b, epsilon)` — NaN != NaN, handles infinities\n\
             `sum_stable(values)` — Kahan summation for numerical stability\n\
             `classify(x)` — return \"zero\", \"normal\", \"subnormal\", \"infinite\", or \"nan\"",
            r#"pub fn safe_div(a: f64, b: f64) -> Option<f64> { todo!() }
pub fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool { todo!() }
pub fn sum_stable(values: &[f64]) -> f64 { todo!() }
pub fn classify(x: f64) -> &'static str { todo!() }"#,
            r#"use opus_float_trap::*;

#[test]
fn div_normal() {
    assert_eq!(safe_div(10.0, 2.0), Some(5.0));
}

#[test]
fn div_by_zero() {
    let r = safe_div(1.0, 0.0).unwrap();
    assert!(r.is_infinite() && r > 0.0);
}

#[test]
fn div_neg_by_zero() {
    let r = safe_div(-1.0, 0.0).unwrap();
    assert!(r.is_infinite() && r < 0.0);
}

#[test]
fn div_zero_by_zero() {
    assert_eq!(safe_div(0.0, 0.0), None);
}

#[test]
fn approx_eq_basic() {
    assert!(approx_eq(1.0, 1.0 + 1e-10, 1e-9));
    assert!(!approx_eq(1.0, 2.0, 0.5));
}

#[test]
fn approx_eq_nan() {
    assert!(!approx_eq(f64::NAN, f64::NAN, 1.0));
}

#[test]
fn approx_eq_inf() {
    assert!(approx_eq(f64::INFINITY, f64::INFINITY, 1.0));
    assert!(!approx_eq(f64::INFINITY, f64::NEG_INFINITY, 1.0));
}

#[test]
fn kahan_sum() {
    // Sum many small numbers — naive sum loses precision
    let values: Vec<f64> = (0..10000).map(|_| 0.1).collect();
    let result = sum_stable(&values);
    assert!((result - 1000.0).abs() < 1e-6, "Got {}", result);
}

#[test]
fn classify_values() {
    assert_eq!(classify(0.0), "zero");
    assert_eq!(classify(-0.0), "zero");
    assert_eq!(classify(1.0), "normal");
    assert_eq!(classify(f64::INFINITY), "infinite");
    assert_eq!(classify(f64::NAN), "nan");
    assert_eq!(classify(5e-324), "subnormal");
}"#,
        ),

        // ── 5.7: Operator Precedence Trap ────────────────────────────
        problem(
            "opus-precedence-trap",
            "tier5",
            "Implement a calculator with UNUSUAL precedence rules:\n\
             1. Parentheses (highest)\n\
             2. Addition and subtraction (HIGHER than multiply/divide!)\n\
             3. Multiplication and division (lowest)\n\n\
             This is the OPPOSITE of standard math. `2 * 3 + 4` = `2 * 7` = `14`, not `10`.",
            r#"pub fn calc(expr: &str) -> Result<i64, String> {
    todo!()
}"#,
            r#"use opus_precedence_trap::*;

#[test]
fn simple() {
    assert_eq!(calc("2 + 3"), Ok(5));
}

#[test]
fn reversed_precedence() {
    // 2 * 3 + 4 = 2 * (3 + 4) = 2 * 7 = 14
    assert_eq!(calc("2 * 3 + 4"), Ok(14));
}

#[test]
fn reversed_precedence_2() {
    // 1 + 2 * 3 + 4 = (1 + 2) * (3 + 4) = 3 * 7 = 21
    assert_eq!(calc("1 + 2 * 3 + 4"), Ok(21));
}

#[test]
fn parens_override() {
    // (2 * 3) + 4 = 6 + 4 = 10
    assert_eq!(calc("(2 * 3) + 4"), Ok(10));
}

#[test]
fn division() {
    // 10 / 2 + 3 = 10 / (2 + 3) = 10 / 5 = 2
    assert_eq!(calc("10 / 2 + 3"), Ok(2));
}

#[test]
fn subtraction() {
    // 2 * 10 - 3 = 2 * (10 - 3) = 2 * 7 = 14
    assert_eq!(calc("2 * 10 - 3"), Ok(14));
}

#[test]
fn all_addition() {
    assert_eq!(calc("1 + 2 + 3"), Ok(6));
}

#[test]
fn all_multiplication() {
    assert_eq!(calc("2 * 3 * 4"), Ok(24));
}

#[test]
fn nested_parens() {
    assert_eq!(calc("(1 + 2) * (3 + 4)"), Ok(21));
}

#[test]
fn complex() {
    // 2 * 3 + 1 * 4 - 2 = 2 * (3 + 1) * (4 - 2) = 2 * 4 * 2 = 16
    assert_eq!(calc("2 * 3 + 1 * 4 - 2"), Ok(16));
}"#,
        ),

        // ── 5.8: Off-by-One Minefield ────────────────────────────────
        problem(
            "opus-fencepost",
            "tier5",
            "Implement these functions — each has a classic off-by-one trap:\n\
             `count_between(a, b)` — how many integers in the INCLUSIVE range [a, b]?\n\
             `fence_posts(length, spacing)` — how many posts for a fence of `length` with `spacing` between posts?\n\
             `pages(total, per_page)` — how many pages to display `total` items at `per_page` per page?\n\
             `zero_pad(n, width)` — format integer with leading zeros to at least `width` characters (handle negatives!)",
            r#"pub fn count_between(a: i64, b: i64) -> u64 { todo!() }
pub fn fence_posts(length: u64, spacing: u64) -> u64 { todo!() }
pub fn pages(total: u64, per_page: u64) -> u64 { todo!() }
pub fn zero_pad(n: i64, width: usize) -> String { todo!() }"#,
            r#"use opus_fencepost::*;

#[test] fn between_positive() { assert_eq!(count_between(3, 7), 5); } // 3,4,5,6,7
#[test] fn between_same() { assert_eq!(count_between(5, 5), 1); }
#[test] fn between_negative() { assert_eq!(count_between(-2, 2), 5); } // -2,-1,0,1,2
#[test] fn between_reversed() { assert_eq!(count_between(7, 3), 0); }
#[test] fn between_zero_span() { assert_eq!(count_between(0, 0), 1); }

#[test] fn fence_basic() { assert_eq!(fence_posts(10, 2), 6); } // |--|--|--|--|--| = 6 posts
#[test] fn fence_zero_length() { assert_eq!(fence_posts(0, 5), 1); } // just one post
#[test] fn fence_exact() { assert_eq!(fence_posts(10, 5), 3); } // |-----|-----| = 3 posts
#[test] fn fence_not_exact() { assert_eq!(fence_posts(11, 5), 4); } // |-----|-----|·| = 4 posts (need extra)

#[test] fn pages_exact() { assert_eq!(pages(100, 10), 10); }
#[test] fn pages_remainder() { assert_eq!(pages(101, 10), 11); }
#[test] fn pages_one_item() { assert_eq!(pages(1, 10), 1); }
#[test] fn pages_zero_items() { assert_eq!(pages(0, 10), 0); }

#[test] fn pad_basic() { assert_eq!(zero_pad(42, 5), "00042"); }
#[test] fn pad_already_wide() { assert_eq!(zero_pad(12345, 3), "12345"); }
#[test] fn pad_negative() { assert_eq!(zero_pad(-42, 5), "-0042"); }
#[test] fn pad_zero() { assert_eq!(zero_pad(0, 3), "000"); }
"#,
        ),

        // ── 5.9: Accumulator Reset Trap ──────────────────────────────
        problem(
            "opus-accumulator",
            "tier5",
            "Implement a running accumulator that tracks a sum BUT resets to zero whenever \
             the sum would cross a threshold boundary (positive or negative). \
             `accumulate(values, threshold)` returns the value after each step.\n\
             When |running_sum| >= threshold, reset to 0 BEFORE adding the next value.",
            r#"pub fn accumulate(values: &[i64], threshold: i64) -> Vec<i64> {
    todo!()
}"#,
            r#"use opus_accumulator::*;

#[test]
fn basic() {
    // threshold = 10, values = [3, 4, 5, 2, 1]
    // step 1: 0+3=3, step 2: 3+4=7, step 3: 7+5=12 >= 10 → reset to 0, then 0+5=5
    // Wait, re-read: reset BEFORE adding. So when |sum| >= threshold before adding next:
    // step 1: sum=0, |0|<10, sum=0+3=3
    // step 2: sum=3, |3|<10, sum=3+4=7
    // step 3: sum=7, |7|<10, sum=7+5=12
    // step 4: sum=12, |12|>=10, reset to 0, sum=0+2=2
    // step 5: sum=2, |2|<10, sum=2+1=3
    assert_eq!(accumulate(&[3, 4, 5, 2, 1], 10), vec![3, 7, 12, 2, 3]);
}

#[test]
fn negative_threshold() {
    // threshold=5, values = [3, 3, -20, 1]
    // step 1: sum=0, ok, sum=3
    // step 2: sum=3, ok, sum=6
    // step 3: sum=6, |6|>=5, reset, sum=0+(-20)=-20
    // step 4: sum=-20, |20|>=5, reset, sum=0+1=1
    assert_eq!(accumulate(&[3, 3, -20, 1], 5), vec![3, 6, -20, 1]);
}

#[test]
fn never_resets() {
    assert_eq!(accumulate(&[1, 1, 1], 100), vec![1, 2, 3]);
}

#[test]
fn always_resets() {
    assert_eq!(accumulate(&[10, 10, 10], 5), vec![10, 10, 10]);
    // each time: previous |sum|>=5, reset, then add 10
}

#[test]
fn empty() {
    assert_eq!(accumulate(&[], 10), vec![]);
}

#[test]
fn threshold_exact() {
    // sum=5 with threshold=5: |5|>=5, so reset before next
    assert_eq!(accumulate(&[5, 1], 5), vec![5, 1]);
    // step 1: |0|<5, sum=5; step 2: |5|>=5, reset, sum=0+1=1
}"#,
        ),

        // ── 5.10: String Math (No BigInt) ────────────────────────────
        problem(
            "opus-string-math",
            "tier5",
            "Implement arbitrary-precision arithmetic using string representation. \
             Numbers can be negative (prefixed with '-'). No leading zeros in output (except \"0\" itself).\n\
             `add(a, b)` — addition\n\
             `multiply(a, b)` — multiplication\n\
             Both inputs and output are decimal strings.",
            r#"pub fn add(a: &str, b: &str) -> String {
    todo!()
}

pub fn multiply(a: &str, b: &str) -> String {
    todo!()
}"#,
            r#"use opus_string_math::*;

#[test] fn add_simple() { assert_eq!(add("123", "456"), "579"); }
#[test] fn add_carry() { assert_eq!(add("999", "1"), "1000"); }
#[test] fn add_different_lengths() { assert_eq!(add("1", "999"), "1000"); }
#[test] fn add_zero() { assert_eq!(add("0", "0"), "0"); }
#[test] fn add_negative() { assert_eq!(add("-5", "3"), "-2"); }
#[test] fn add_both_negative() { assert_eq!(add("-10", "-20"), "-30"); }
#[test] fn add_negative_result_positive() { assert_eq!(add("-3", "5"), "2"); }
#[test] fn add_large() {
    assert_eq!(
        add("99999999999999999999", "1"),
        "100000000000000000000"
    );
}

#[test] fn mul_simple() { assert_eq!(multiply("12", "34"), "408"); }
#[test] fn mul_by_zero() { assert_eq!(multiply("12345", "0"), "0"); }
#[test] fn mul_by_one() { assert_eq!(multiply("12345", "1"), "12345"); }
#[test] fn mul_negative() { assert_eq!(multiply("-3", "4"), "-12"); }
#[test] fn mul_both_negative() { assert_eq!(multiply("-3", "-4"), "12"); }
#[test] fn mul_large() {
    assert_eq!(
        multiply("123456789", "987654321"),
        "121932631112635269"
    );
}
#[test] fn no_leading_zeros() {
    assert_eq!(add("100", "-99"), "1");
    assert_eq!(multiply("10", "0"), "0");
}"#,
        ),
    ]
}
