use super::problem;
use crate::benchmark::BenchmarkProblem;

/// Extended Tier 3: Induction — infer the algorithm from input/output examples only.
pub(super) fn tier3_ext() -> Vec<BenchmarkProblem> {
    vec![
        problem(
            "opus-mystery-transform",
            "tier3",
            "Infer the transformation from the test cases. No instructions — only tests.",
            "",
            r#"use opus_mystery_transform::transform;

#[test] fn t1() { assert_eq!(transform("abc"), "cba"); }
#[test] fn t2() { assert_eq!(transform("hello"), "olleh"); }
#[test] fn t3() { assert_eq!(transform(""), ""); }
#[test] fn t4() { assert_eq!(transform("a"), "a"); }
#[test] fn t5() { assert_eq!(transform("12345"), "54321"); }"#,
        ),
        problem(
            "opus-mystery-sequence",
            "tier3",
            "Infer the sequence generator from the test cases.",
            "",
            r#"use opus_mystery_sequence::generate;

#[test] fn t1() { assert_eq!(generate(1), vec![1]); }
#[test] fn t2() { assert_eq!(generate(2), vec![1, 1]); }
#[test] fn t3() { assert_eq!(generate(3), vec![1, 1, 2]); }
#[test] fn t4() { assert_eq!(generate(5), vec![1, 1, 2, 3, 5]); }
#[test] fn t5() { assert_eq!(generate(8), vec![1, 1, 2, 3, 5, 8, 13, 21]); }"#,
        ),
        problem(
            "opus-mystery-cipher",
            "tier3",
            "Infer the encoding from the test cases.",
            "",
            r#"use opus_mystery_cipher::{encode, decode};

#[test] fn e1() { assert_eq!(encode("abc", 1), "bcd"); }
#[test] fn e2() { assert_eq!(encode("xyz", 3), "abc"); }
#[test] fn e3() { assert_eq!(encode("Hello", 13), "Uryyb"); }
#[test] fn e4() { assert_eq!(encode("a", 0), "a"); }
#[test] fn d1() { assert_eq!(decode("bcd", 1), "abc"); }
#[test] fn d2() { assert_eq!(decode("abc", 3), "xyz"); }
#[test] fn roundtrip() { assert_eq!(decode(&encode("test", 7), 7), "test"); }
#[test] fn preserves_non_alpha() { assert_eq!(encode("hi 123!", 5), "mn 123!"); }"#,
        ),
        problem(
            "opus-mystery-reduce",
            "tier3",
            "Infer what this function does from the test cases.",
            "",
            r#"use opus_mystery_reduce::process;

#[test] fn t1() { assert_eq!(process(&[1, 2, 3, 4, 5]), 15); }
#[test] fn t2() { assert_eq!(process(&[10]), 10); }
#[test] fn t3() { assert_eq!(process(&[]), 0); }
#[test] fn t4() { assert_eq!(process(&[-1, 1]), 0); }
#[test] fn t5() { assert_eq!(process(&[100, 200, 300]), 600); }"#,
        ),
        problem(
            "opus-mystery-filter",
            "tier3",
            "Infer the filtering rule from test cases.",
            "",
            r#"use opus_mystery_filter::filter;

#[test] fn t1() { assert_eq!(filter(&[1, 2, 3, 4, 5, 6]), vec![2, 4, 6]); }
#[test] fn t2() { assert_eq!(filter(&[7, 8, 9]), vec![8]); }
#[test] fn t3() { assert_eq!(filter(&[1, 3, 5]), Vec::<i32>::new()); }
#[test] fn t4() { assert_eq!(filter(&[]), Vec::<i32>::new()); }
#[test] fn t5() { assert_eq!(filter(&[2, 4, 6, 8]), vec![2, 4, 6, 8]); }
#[test] fn t6() { assert_eq!(filter(&[-2, -1, 0, 1, 2]), vec![-2, 0, 2]); }"#,
        ),
        problem(
            "opus-mystery-compress",
            "tier3",
            "Infer the compression and decompression algorithms.",
            "",
            r#"use opus_mystery_compress::{compress, decompress};

#[test] fn c1() { assert_eq!(compress("aaabbc"), "3a2b1c"); }
#[test] fn c2() { assert_eq!(compress("a"), "1a"); }
#[test] fn c3() { assert_eq!(compress(""), ""); }
#[test] fn c4() { assert_eq!(compress("aaa"), "3a"); }
#[test] fn d1() { assert_eq!(decompress("3a2b1c"), "aaabbc"); }
#[test] fn d2() { assert_eq!(decompress("1a"), "a"); }
#[test] fn d3() { assert_eq!(decompress(""), ""); }
#[test] fn round() { assert_eq!(decompress(&compress("xxxyyz")), "xxxyyz"); }"#,
        ),
        problem(
            "opus-mystery-map",
            "tier3",
            "Infer the mapping function from test cases.",
            "",
            r#"use opus_mystery_map::apply;

#[test] fn t1() { assert_eq!(apply(&[1, 2, 3]), vec![1, 4, 9]); }
#[test] fn t2() { assert_eq!(apply(&[0, 5, 10]), vec![0, 25, 100]); }
#[test] fn t3() { assert_eq!(apply(&[-3]), vec![9]); }
#[test] fn t4() { assert_eq!(apply(&[]), Vec::<i64>::new()); }
#[test] fn t5() { assert_eq!(apply(&[1, -1, 2, -2]), vec![1, 1, 4, 4]); }"#,
        ),
        problem(
            "opus-mystery-sort",
            "tier3",
            "Infer the sorting criterion from test cases.",
            "",
            r#"use opus_mystery_sort::special_sort;

#[test] fn t1() { assert_eq!(special_sort(&["banana", "apple", "cherry"]), vec!["apple", "banana", "cherry"]); }
#[test] fn t2() { assert_eq!(special_sort(&["bb", "aaa", "c"]), vec!["c", "bb", "aaa"]); }
#[test] fn t3() { assert_eq!(special_sort(&["zz", "aa", "mm"]), vec!["aa", "mm", "zz"]); }
#[test] fn t4() { assert_eq!(special_sort(&["cat", "at", "a"]), vec!["a", "at", "cat"]); }
#[test] fn t5() { assert_eq!(special_sort(&[]), Vec::<&str>::new()); }"#,
        ),
    ]
}
