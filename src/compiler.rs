use regex::bytes::Regex;

use crate::parser::{CharacterClass, CharsMatcher, RawComponent, SingleCharMatcher};

#[derive(Debug, Clone)]
pub enum Component {
    Regex(Regex),
    Literal(String),
    Wildcard,
    ParentDir,
}

/// Determine if the built regular expressions should use case sensitivity or not
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

/// Compile a parsed component to its final form
///
/// Wildcard and literal components remain the same, while matchers combinations are compiled
/// into regular expressions to accelerate matching.
///
/// The goal of this function is to make pattern matching faster.
pub fn compile_component(component: RawComponent, case_sensitivity: CaseSensitivity) -> Component {
    match component {
        RawComponent::Wildcard => Component::Wildcard,
        RawComponent::ParentDir => Component::ParentDir,

        RawComponent::Literal(lit) => match case_sensitivity {
            CaseSensitivity::Insensitive => {
                Component::Regex(Regex::new(&format!("(?i){}", regex::escape(&lit))).unwrap())
            }
            CaseSensitivity::Sensitive => Component::Literal(lit),
        },

        RawComponent::Suite(chars_matchers) => {
            let mut regex = match case_sensitivity {
                CaseSensitivity::Sensitive => String::new(),
                CaseSensitivity::Insensitive => String::from("(?i)"),
            };

            regex.push('^');

            for matcher in chars_matchers {
                compile_chars_matcher(&matcher, &mut regex);
            }

            regex.push('$');

            Component::Regex(Regex::new(&regex).unwrap())
        }
    }
}

/// Compile a [`CharsMatcher`] to a regular expression
///
/// The resulting expression is appended to the provided mutable string reference
fn compile_chars_matcher(chars_matcher: &CharsMatcher, out: &mut String) {
    match chars_matcher {
        CharsMatcher::AnyChar => out.push('.'),
        CharsMatcher::AnyChars => out.push_str(".*"),
        CharsMatcher::Literal(lit) => out.push_str(&regex::escape(lit)),
        CharsMatcher::OneOfChars(single_char_matchers) => {
            out.push('[');

            for matcher in single_char_matchers {
                compile_single_char_matcher(*matcher, out);
            }

            out.push(']');
        }
        CharsMatcher::NoneOfChars(single_char_matchers) => {
            out.push_str("[^");

            for matcher in single_char_matchers {
                compile_single_char_matcher(*matcher, out);
            }

            out.push(']');
        }
        CharsMatcher::OneOfGroups(matchers) => {
            out.push('(');

            for (i, matchers) in matchers.iter().enumerate() {
                if i > 0 {
                    out.push('|');
                }

                for matcher in matchers {
                    compile_chars_matcher(matcher, out);
                }
            }

            out.push(')');
        }
    }
}

/// Compile a [`SingleCharMatcher`] to a regular expression
///
/// The resulting expression is appended to the provided mutable string reference
fn compile_single_char_matcher(char_matcher: SingleCharMatcher, out: &mut String) {
    match char_matcher {
        SingleCharMatcher::Literal(lit) => out.push_str(&regex::escape(&lit.to_string())),

        SingleCharMatcher::Class(character_class) => out.push_str(match character_class {
            CharacterClass::Alpha => "[:alpha:]",
            CharacterClass::Digit => "[:digit:]",
            CharacterClass::Alphanumeric => "[:alnum:]",
            CharacterClass::Uppercase => "[:upper:]",
            CharacterClass::Lowercase => "[:lower:]",
            CharacterClass::Whitespace => "[:space:]",
        }),
    }
}
