use super::problem;
use crate::benchmark::BenchmarkProblem;

pub(super) fn tier3_ext2() -> Vec<BenchmarkProblem> {
    vec![
        problem(
            "opus-mystery-encode",
            "tier3",
            "Infer the encoding from tests.",
            "",
            r#"use opus_mystery_encode::encode;
#[test] fn t1() { assert_eq!(encode("abc"), "123"); }
#[test] fn t2() { assert_eq!(encode("z"), "26"); }
#[test] fn t3() { assert_eq!(encode(""), ""); }
#[test] fn t4() { assert_eq!(encode("az"), "126"); }
#[test] fn t5() { assert_eq!(encode("hello"), "85121215"); }"#,
        ),
        problem(
            "opus-mystery-pairs",
            "tier3",
            "Infer what this function does.",
            "",
            r#"use opus_mystery_pairs::pairs;
#[test] fn t1() { assert_eq!(pairs(&[1,2,3,4], 5), vec![(1,4),(2,3)]); }
#[test] fn t2() { assert_eq!(pairs(&[1,1,1], 2), vec![(1,1)]); }
#[test] fn t3() { assert_eq!(pairs(&[1,2,3], 10), Vec::<(i32,i32)>::new()); }
#[test] fn t4() { assert_eq!(pairs(&[], 5), Vec::<(i32,i32)>::new()); }
#[test] fn t5() { let mut r = pairs(&[0,5,3,2], 5); r.sort(); assert_eq!(r, vec![(0,5),(2,3)]); }"#,
        ),
        problem(
            "opus-mystery-grid",
            "tier3",
            "Infer the grid operation.",
            "",
            r#"use opus_mystery_grid::count;
#[test] fn t1() { assert_eq!(count(&[&[1,0],[0,1]]), 2); }
#[test] fn t2() { assert_eq!(count(&[&[1,1],[1,1]]), 1); }
#[test] fn t3() { assert_eq!(count(&[&[0,0],[0,0]]), 0); }
#[test] fn t4() { assert_eq!(count(&[&[1,0,1],[0,0,0],[1,0,1]]), 4); }
#[test] fn t5() { assert_eq!(count(&[&[1,1,0],[1,0,0],[0,0,1]]), 2); }"#,
        ),
        problem(
            "opus-mystery-bits",
            "tier3",
            "Infer the bit operation.",
            "",
            r#"use opus_mystery_bits::process;
#[test] fn t1() { assert_eq!(process(0), 0); }
#[test] fn t2() { assert_eq!(process(1), 1); }
#[test] fn t3() { assert_eq!(process(7), 3); }
#[test] fn t4() { assert_eq!(process(255), 8); }
#[test] fn t5() { assert_eq!(process(1024), 1); }
#[test] fn t6() { assert_eq!(process(15), 4); }"#,
        ),
        problem(
            "opus-mystery-nest",
            "tier3",
            "Infer the nesting function.",
            "",
            r#"use opus_mystery_nest::depth;
#[test] fn t1() { assert_eq!(depth("()"), 1); }
#[test] fn t2() { assert_eq!(depth("(())"), 2); }
#[test] fn t3() { assert_eq!(depth("((()))"), 3); }
#[test] fn t4() { assert_eq!(depth("()()"), 1); }
#[test] fn t5() { assert_eq!(depth("(()())"), 2); }
#[test] fn t6() { assert_eq!(depth(""), 0); }"#,
        ),
        problem(
            "opus-mystery-spiral",
            "tier3",
            "Infer the spiral function.",
            "",
            r#"use opus_mystery_spiral::spiral;
#[test] fn t1() { assert_eq!(spiral(&[&[1,2],[3,4]]), vec![1,2,4,3]); }
#[test] fn t2() { assert_eq!(spiral(&[&[1,2,3],[4,5,6],[7,8,9]]), vec![1,2,3,6,9,8,7,4,5]); }
#[test] fn t3() { assert_eq!(spiral(&[&[1]]), vec![1]); }
#[test] fn t4() { assert_eq!(spiral(&[&[1,2,3]]), vec![1,2,3]); }
#[test] fn t5() { assert_eq!(spiral(&[&[1],[2],[3]]), vec![1,2,3]); }"#,
        ),
        problem(
            "opus-mystery-digit-sum",
            "tier3",
            "Infer the digit operation.",
            "",
            r#"use opus_mystery_digit_sum::reduce;
#[test] fn t1() { assert_eq!(reduce(0), 0); }
#[test] fn t2() { assert_eq!(reduce(5), 5); }
#[test] fn t3() { assert_eq!(reduce(38), 2); }
#[test] fn t4() { assert_eq!(reduce(999), 9); }
#[test] fn t5() { assert_eq!(reduce(100), 1); }
#[test] fn t6() { assert_eq!(reduce(19), 1); }"#,
        ),
    ]
}
