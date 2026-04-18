use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier1_ext3() -> Vec<BenchmarkProblem> {
    vec![
        problem("opus-atbash", "tier1",
            "Implement the Atbash cipher: a↔z, b↔y, c��x, etc. Preserve case, pass through non-alpha.",
            r#"pub fn encode(s: &str) -> String { todo!() }
pub fn decode(s: &str) -> String { todo!() }"#,
            r#"use opus_atbash::*;
#[test] fn enc() { assert_eq!(encode("abc"), "zyx"); }
#[test] fn enc_upper() { assert_eq!(encode("ABC"), "ZYX"); }
#[test] fn enc_mixed() { assert_eq!(encode("Hello!"), "Svool!"); }
#[test] fn dec() { assert_eq!(decode("zyx"), "abc"); }
#[test] fn roundtrip() { assert_eq!(decode(&encode("Test 123")), "Test 123"); }
#[test] fn empty() { assert_eq!(encode(""), ""); }"#),

        problem("opus-isbn", "tier1",
            "Validate ISBN-10 and ISBN-13 check digits.",
            r#"pub fn is_valid_isbn10(s: &str) -> bool { todo!() }
pub fn is_valid_isbn13(s: &str) -> bool { todo!() }"#,
            r#"use opus_isbn::*;
#[test] fn isbn10_valid() { assert!(is_valid_isbn10("0306406152")); }
#[test] fn isbn10_x() { assert!(is_valid_isbn10("080442957X")); }
#[test] fn isbn10_invalid() { assert!(!is_valid_isbn10("0306406153")); }
#[test] fn isbn10_short() { assert!(!is_valid_isbn10("123")); }
#[test] fn isbn13_valid() { assert!(is_valid_isbn13("9780306406157")); }
#[test] fn isbn13_invalid() { assert!(!is_valid_isbn13("9780306406158")); }
#[test] fn isbn13_short() { assert!(!is_valid_isbn13("978")); }"#),

        problem("opus-luhn", "tier1",
            "Implement the Luhn algorithm for credit card number validation.",
            r#"pub fn is_valid(s: &str) -> bool { todo!() }"#,
            r#"use opus_luhn::*;
#[test] fn valid_card() { assert!(is_valid("4539 3195 0343 6467")); }
#[test] fn single_zero() { assert!(is_valid("0")); }
#[test] fn single_nonzero() { assert!(!is_valid("1")); }
#[test] fn two_digit() { assert!(is_valid("59")); }
#[test] fn invalid() { assert!(!is_valid("8273 1232 7352 0569")); }
#[test] fn non_digit() { assert!(!is_valid("abc")); }
#[test] fn empty() { assert!(!is_valid("")); }
#[test] fn spaces_only() { assert!(!is_valid("   ")); }"#),

        problem("opus-collatz", "tier1",
            "Compute the Collatz sequence from n to 1. Count the steps.",
            r#"pub fn collatz_sequence(n: u64) -> Vec<u64> { todo!() }
pub fn collatz_steps(n: u64) -> u64 { todo!() }"#,
            r#"use opus_collatz::*;
#[test] fn seq_1() { assert_eq!(collatz_sequence(1), vec![1]); }
#[test] fn seq_6() { assert_eq!(collatz_sequence(6), vec![6,3,10,5,16,8,4,2,1]); }
#[test] fn steps_1() { assert_eq!(collatz_steps(1), 0); }
#[test] fn steps_6() { assert_eq!(collatz_steps(6), 8); }
#[test] fn steps_27() { assert_eq!(collatz_steps(27), 111); }"#),

        problem("opus-diamond", "tier1",
            "Generate a diamond shape from a letter. 'A' gives a single A. 'C' gives a diamond A..C..A.",
            r#"pub fn diamond(c: char) -> String { todo!() }"#,
            r#"use opus_diamond::*;
#[test] fn a() { assert_eq!(diamond('A'), "A\n"); }
#[test] fn b() { assert_eq!(diamond('B'), " A\nB B\n A\n"); }
#[test] fn c() {
    let d = diamond('C');
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(lines.len(), 5);
    assert!(lines[0].contains('A'));
    assert!(lines[2].contains('C'));
}"#),

        problem("opus-hamming", "tier1",
            "Compute the Hamming distance between two strings of equal length.",
            r#"pub fn hamming_distance(a: &str, b: &str) -> Result<usize, String> { todo!() }"#,
            r#"use opus_hamming::*;
#[test] fn same() { assert_eq!(hamming_distance("GGACT", "GGACT").unwrap(), 0); }
#[test] fn diff() { assert_eq!(hamming_distance("GAGCCTACTAACGGGAT", "CATCGTAATGACGGCCT").unwrap(), 7); }
#[test] fn single() { assert_eq!(hamming_distance("A", "G").unwrap(), 1); }
#[test] fn empty() { assert_eq!(hamming_distance("", "").unwrap(), 0); }
#[test] fn unequal() { assert!(hamming_distance("AB", "ABC").is_err()); }"#),

        problem("opus-sieve", "tier1",
            "Implement the Sieve of Eratosthenes to find all primes up to n.",
            r#"pub fn primes_up_to(n: u64) -> Vec<u64> { todo!() }"#,
            r#"use opus_sieve::*;
#[test] fn small() { assert_eq!(primes_up_to(10), vec![2,3,5,7]); }
#[test] fn one() { assert_eq!(primes_up_to(1), vec![]); }
#[test] fn two() { assert_eq!(primes_up_to(2), vec![2]); }
#[test] fn thirty() { assert_eq!(primes_up_to(30), vec![2,3,5,7,11,13,17,19,23,29]); }
#[test] fn zero() { assert_eq!(primes_up_to(0), vec![]); }"#),

        problem("opus-phone-words", "tier1",
            "Convert a phone number to all possible letter combinations (like T9). \
             2=abc, 3=def, 4=ghi, 5=jkl, 6=mno, 7=pqrs, 8=tuv, 9=wxyz.",
            r#"pub fn letter_combinations(digits: &str) -> Vec<String> { todo!() }"#,
            r#"use opus_phone_words::*;
#[test] fn two_three() { let mut r = letter_combinations("23"); r.sort(); assert_eq!(r[0], "ad"); assert_eq!(r.len(), 9); }
#[test] fn single() { let r = letter_combinations("2"); assert_eq!(r, vec!["a","b","c"]); }
#[test] fn empty() { assert_eq!(letter_combinations(""), Vec::<String>::new()); }
#[test] fn seven() { let r = letter_combinations("7"); assert_eq!(r.len(), 4); } // pqrs
#[test] fn three_digits() { assert_eq!(letter_combinations("234").len(), 27); }"#),

        problem("opus-look-say", "tier1",
            "Generate the look-and-say sequence. 1 → 11 → 21 �� 1211 → 111221 → ...",
            r#"pub fn look_and_say(s: &str) -> String { todo!() }
pub fn sequence(start: &str, n: usize) -> Vec<String> { todo!() }"#,
            r#"use opus_look_say::*;
#[test] fn step1() { assert_eq!(look_and_say("1"), "11"); }
#[test] fn step2() { assert_eq!(look_and_say("11"), "21"); }
#[test] fn step3() { assert_eq!(look_and_say("21"), "1211"); }
#[test] fn step4() { assert_eq!(look_and_say("1211"), "111221"); }
#[test] fn complex() { assert_eq!(look_and_say("3322251"), "23321511"); }
#[test] fn seq() { assert_eq!(sequence("1", 4), vec!["1","11","21","1211"]); }
#[test] fn seq_one() { assert_eq!(sequence("1", 1), vec!["1"]); }"#),

        problem("opus-hex-convert", "tier1",
            "Convert between hex strings and byte arrays.",
            r#"pub fn to_hex(bytes: &[u8]) -> String { todo!() }
pub fn from_hex(s: &str) -> Result<Vec<u8>, String> { todo!() }"#,
            r#"use opus_hex_convert::*;
#[test] fn to_hex_basic() { assert_eq!(to_hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef"); }
#[test] fn to_hex_empty() { assert_eq!(to_hex(&[]), ""); }
#[test] fn to_hex_zeros() { assert_eq!(to_hex(&[0, 0]), "0000"); }
#[test] fn from_hex_basic() { assert_eq!(from_hex("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]); }
#[test] fn from_hex_upper() { assert_eq!(from_hex("DEADBEEF").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]); }
#[test] fn from_hex_empty() { assert_eq!(from_hex("").unwrap(), vec![]); }
#[test] fn from_hex_odd() { assert!(from_hex("abc").is_err()); }
#[test] fn from_hex_invalid() { assert!(from_hex("xyz").is_err()); }
#[test] fn roundtrip() { let data = vec![1,2,3,255]; assert_eq!(from_hex(&to_hex(&data)).unwrap(), data); }"#),

        problem("opus-pascal-triangle", "tier1",
            "Generate the first n rows of Pascal's triangle.",
            r#"pub fn pascals_triangle(n: usize) -> Vec<Vec<u64>> { todo!() }"#,
            r#"use opus_pascal_triangle::*;
#[test] fn zero() { assert_eq!(pascals_triangle(0), Vec::<Vec<u64>>::new()); }
#[test] fn one() { assert_eq!(pascals_triangle(1), vec![vec![1]]); }
#[test] fn four() { assert_eq!(pascals_triangle(4), vec![vec![1], vec![1,1], vec![1,2,1], vec![1,3,3,1]]); }
#[test] fn row_six() { let t = pascals_triangle(7); assert_eq!(t[6], vec![1,6,15,20,15,6,1]); }"#),

        problem("opus-string-calc", "tier1",
            "Implement a string calculator that takes comma or newline separated numbers and returns their sum. \
             Negative numbers throw an error listing all negatives.",
            r#"pub fn add(input: &str) -> Result<i64, String> { todo!() }"#,
            r#"use opus_string_calc::*;
#[test] fn empty() { assert_eq!(add("").unwrap(), 0); }
#[test] fn single() { assert_eq!(add("1").unwrap(), 1); }
#[test] fn two() { assert_eq!(add("1,2").unwrap(), 3); }
#[test] fn newline() { assert_eq!(add("1\n2,3").unwrap(), 6); }
#[test] fn negative() { let e = add("-1,2,-3").unwrap_err(); assert!(e.contains("-1")); assert!(e.contains("-3")); }
#[test] fn ignore_big() { assert_eq!(add("2,1001").unwrap(), 2); }"#),

        problem("opus-roman-calc", "tier1",
            "Add two Roman numeral strings and return the result as a Roman numeral.",
            r#"pub fn roman_add(a: &str, b: &str) -> String { todo!() }"#,
            r#"use opus_roman_calc::*;
#[test] fn basic() { assert_eq!(roman_add("I", "I"), "II"); }
#[test] fn five() { assert_eq!(roman_add("II", "III"), "V"); }
#[test] fn complex() { assert_eq!(roman_add("XIV", "LX"), "LXXIV"); }
#[test] fn large() { assert_eq!(roman_add("M", "CM"), "MCM"); }"#),

        problem("opus-bowling", "tier1",
            "Score a bowling game given a sequence of rolls.",
            r#"pub fn score(rolls: &[u16]) -> Result<u16, String> { todo!() }"#,
            r#"use opus_bowling::*;
#[test] fn gutter() { assert_eq!(score(&[0; 20]).unwrap(), 0); }
#[test] fn all_ones() { assert_eq!(score(&[1; 20]).unwrap(), 20); }
#[test] fn spare() { assert_eq!(score(&[5,5,3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]).unwrap(), 16); }
#[test] fn strike() { assert_eq!(score(&[10,3,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]).unwrap(), 24); }
#[test] fn perfect() { assert_eq!(score(&[10,10,10,10,10,10,10,10,10,10,10,10]).unwrap(), 300); }"#),
    ]
}
