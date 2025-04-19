use std::path::Path;

use globby::Pattern;

struct PatternTest {
    pattern_str: &'static str,
    should_match: &'static [&'static str],
    should_not_match: &'static [&'static str],
}

fn test_pattern(test: PatternTest) {
    let PatternTest {
        pattern_str,
        should_match,
        should_not_match,
    } = test;

    let pattern = Pattern::new(pattern_str).unwrap();

    for path in should_match {
        assert!(
            pattern.is_match(Path::new(path)),
            "Pattern '{pattern_str}' did not match path '{path}'"
        );
    }

    for path in should_not_match {
        assert!(
            !pattern.is_match(Path::new(path)),
            "Pattern '{pattern_str}' unexpectedly matched path '{path}'"
        );
    }
}

#[cfg(target_family = "unix")]
#[test]
fn patterns() {
    test_pattern(PatternTest {
        pattern_str: "*",
        should_match: &["a", "ab", "abc", "a/", "a\\"],
        should_not_match: &["", "/", "/a"],
    });

    test_pattern(PatternTest {
        pattern_str: "?",
        should_match: &["a", "é", "?", " "],
        should_not_match: &["", "ab", "/"],
    });

    test_pattern(PatternTest {
        pattern_str: "??",
        should_match: &["aa", "ab", "aé", "  "],
        should_not_match: &["", "a", "aaa", "/ab"],
    });

    test_pattern(PatternTest {
        pattern_str: "*?*",
        should_match: &["a", "ab", "abc", "abcd", "abcde"],
        should_not_match: &[""],
    });

    test_pattern(PatternTest {
        pattern_str: "literal",
        should_match: &["literal"],
        should_not_match: &["litera", "literall", "", "/"],
    });

    for pattern_str in ["**", "**/**", "**/**/**"] {
        test_pattern(PatternTest {
            pattern_str,
            should_match: &["", "a", "a/b", "a/b/c", "a/", "a/b/"],
            should_not_match: &["/", "/a"],
        });
    }

    test_pattern(PatternTest {
        pattern_str: "**/*",
        should_match: &["a", "a/b", "a/b/c", "a/", "a/b/"],
        should_not_match: &["", "/", "/a"],
    });

    test_pattern(PatternTest {
        pattern_str: "**/*",
        should_match: &["a", "a/b", "a/b/c", "a/", "a/b/"],
        should_not_match: &["", "/", "/a"],
    });

    test_pattern(PatternTest {
        pattern_str: "*/**/*",
        should_match: &["a/b", "a/b/c", "a/b/"],
        should_not_match: &["", "a", "a/", "/a", "/"],
    });

    for pattern_str in ["/**", "/**/**", "/**/**/**"] {
        test_pattern(PatternTest {
            pattern_str,
            should_match: &["/a", "/a/b", "/a/b/c", "/a/", "/a/b/"],
            should_not_match: &["", "a", "a/b", "a/b/c", "a/", "a/b/"],
        });
    }

    test_pattern(PatternTest {
        pattern_str: "a[bcd]e",
        should_match: &["abe", "ace", "ade"],
        should_not_match: &["ae", "aee", "b", "c", "d", "abbe"],
    });

    test_pattern(PatternTest {
        pattern_str: "a[!bcd]e",
        should_match: &["aee", "a e"],
        should_not_match: &["ae", "abe", "ace", "ade", "aeee"],
    });

    test_pattern(PatternTest {
        pattern_str: "a[b[:alpha:]d]e",
        should_match: &["abe", "ace", "ade"],
        should_not_match: &[
            "a", "ae", "e", "abde", "bd", "bde", "ab1de", "ab de", "abcde",
        ],
    });

    test_pattern(PatternTest {
        pattern_str: "{a|bc}",
        should_match: &["a", "bc"],
        should_not_match: &["", "abc", "b", "c"],
    });

    test_pattern(PatternTest {
        pattern_str: "{a|bc|d}",
        should_match: &["a", "bc", "d"],
        should_not_match: &["", "abc", "b", "c", "ad", "abcd", "bcd"],
    });

    test_pattern(PatternTest {
        pattern_str: "{a|b[[:digit:]]?}",
        should_match: &["a", "b1c", "b1é", "b2 "],
        should_not_match: &["", "ab", "b", "b2", "c2a"],
    });
}
