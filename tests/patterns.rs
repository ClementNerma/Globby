use std::path::Path;

use globby::{Pattern, PatternOpts};

#[test]
fn building_unix_patterns() {
    for valid in [
        "", ".", "./", "/", "//", "/./.", "///", ".///", "a", "a/b", "a//b", "/a/b/", "/a//b//",
    ] {
        assert!(Pattern::new(valid).is_ok());
    }

    for invalid in ["./..", "/..", "../a/..", "a/..", "/a/.."] {
        assert!(Pattern::new(invalid).is_err());
    }
}

#[test]
fn matching_unix_patterns() {
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

#[test]
fn building_windows_patterns() {
    for valid in [
        "", ".", "./", "/", "//", "/./.", "///", ".///", "a", "a/b", "a//b", "/a/b/", "/a//b//",
        "", ".", ".\\", "\\", "\\", "\\.\\.", "\\", ".\\", "a", "a\\b", "a\\b", "\\a\\b\\",
        "\\a\\b\\",
    ] {
        assert!(Pattern::new(valid).is_ok());
    }

    for invalid in [
        "./..",
        "/..",
        "../a/..",
        "a/..",
        "/a/..",
        ".\\..",
        "\\..",
        "..\\a\\..",
        "a\\..",
        "\\a\\..",
    ] {
        assert!(Pattern::new(invalid).is_err());
    }
}

#[test]
fn testing_windows_prefixes() {
    use globby::{PathPrefix, WindowsDrive};

    fn expect_prefix(pattern: &str, expected_prefix: PathPrefix) {
        let pattern = compile_pattern(pattern);

        assert!(
            pattern
                .prefix()
                .is_some_and(|prefix| prefix == expected_prefix),
            "Expected prefix {expected_prefix:?}, got {:?}",
            pattern.prefix()
        )
    }

    for prefix in ["\\\\?\\c:", "\\\\?\\C:", "\\\\?\\c:\\", "\\\\?\\C:\\"] {
        expect_prefix(
            prefix,
            PathPrefix::WindowsDrive(WindowsDrive::try_from('c').unwrap()),
        );
    }

    for prefix in [
        "\\\\?",
        "\\\\?\\UNC\\server\\share",
        "\\\\.\\device",
        "\\\\server\\share",
        "\\\\?a",
        "\\\\?\\UNC\\server",
        "\\\\?\\UNC\\server\\share*",
        "\\\\?\\C:a",
        "\\\\a",
        "\\\\a\\",
        "C:a",
        "c:a",
    ] {
        assert!(
            Pattern::new(prefix).is_err(),
            "Invalid pattern '{prefix}' is unexpectedly considered valid"
        );
    }
}

#[test]
fn matching_windows_patterns() {
    test_pattern(PatternTest {
        pattern_str: "*",
        should_match: &["a", "ab", "abc", "a\\", "a/"],
        should_not_match: &["", "\\", "\\a"],
    });

    test_pattern(PatternTest {
        pattern_str: "?",
        should_match: &["a", "é", "?", " "],
        should_not_match: &["", "ab", "\\"],
    });

    test_pattern(PatternTest {
        pattern_str: "??",
        should_match: &["aa", "ab", "aé", "  "],
        should_not_match: &["", "a", "aaa", "\\ab"],
    });

    test_pattern(PatternTest {
        pattern_str: "*?*",
        should_match: &["a", "ab", "abc", "abcd", "abcde"],
        should_not_match: &[""],
    });

    test_pattern(PatternTest {
        pattern_str: "literal",
        should_match: &["literal"],
        should_not_match: &["litera", "literall", "", "\\"],
    });

    for pattern_str in ["**", "**\\**", "**\\**\\**"] {
        test_pattern(PatternTest {
            pattern_str,
            should_match: &["", "a", "a\\b", "a\\b\\c", "a\\", "a\\b\\"],
            should_not_match: &["\\", "\\a"],
        });
    }

    test_pattern(PatternTest {
        pattern_str: "**\\*",
        should_match: &["a", "a\\b", "a\\b\\c", "a\\", "a\\b\\"],
        should_not_match: &["", "\\", "\\a"],
    });

    test_pattern(PatternTest {
        pattern_str: "**\\*",
        should_match: &["a", "a\\b", "a\\b\\c", "a\\", "a\\b\\"],
        should_not_match: &["", "\\", "\\a"],
    });

    test_pattern(PatternTest {
        pattern_str: "*\\**\\*",
        should_match: &["a\\b", "a\\b\\c", "a\\b\\"],
        should_not_match: &["", "a", "a\\", "\\a", "\\"],
    });

    for pattern_str in ["\\**", "\\**\\**", "\\**\\**\\**"] {
        test_pattern(PatternTest {
            pattern_str,
            should_match: &["\\a", "\\a\\b", "\\a\\b\\c", "\\a\\", "\\a\\b\\"],
            should_not_match: &["", "a", "a\\b", "a\\b\\c", "a\\", "a\\b\\"],
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

#[test]
fn case_sensitivity() {
    assert!(Pattern::new("hello").unwrap().is_match(Path::new("hello")));
    assert!(!Pattern::new("hello").unwrap().is_match(Path::new("Hello")));

    assert!(
        Pattern::new_with_opts(
            "hello",
            PatternOpts {
                case_insensitive: true
            }
        )
        .unwrap()
        .is_match(Path::new("hello"))
    );

    assert!(
        Pattern::new_with_opts(
            "hello",
            PatternOpts {
                case_insensitive: true
            }
        )
        .unwrap()
        .is_match(Path::new("Hello"))
    );
}

fn compile_pattern(pattern: &str) -> Pattern {
    Pattern::new(pattern)
        .unwrap_or_else(|err| panic!("Failed to compile pattern '{pattern}':\n  > {err:?}"))
}

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

    let pattern = compile_pattern(pattern_str);

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
