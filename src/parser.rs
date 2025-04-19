use std::{collections::HashSet, num::NonZero, sync::LazyLock};

use parsy::{Parser, char, choice, end, filter, just, not, recursive_shared, silent_choice};

/// Parse a glob (pattern) string into a [`RawPattern`]
pub static PATTERN_PARSER: LazyLock<Box<dyn Parser<RawPattern> + Send + Sync>> = LazyLock::new(
    || {
        let normal_char = filter(|c| !SPECIAL_CHARS.contains(&c));

        let chars_matcher = recursive_shared(|chars_matcher| {
            choice::<CharsMatcher, _>((
            //
            // Literal characters
            //
            normal_char
                .repeated_into_container::<String>()
                .at_least(1)
                .map(CharsMatcher::Literal),
                        //
            // Optional universal character (or not)
            //
            char('?').map(|_| CharsMatcher::AnyChar),
            //
            // Wildcard
            //
            char('*')
                .followed_by(not(char('*')).critical(
                    "Wildcard components '**' must be preceded by and followed by a path separator",
                ))
                .map(|_| CharsMatcher::AnyChars),
            //
            // Character alternates
            //
            char('[')
                .ignore_then(char('!').or_not())
                .then(
                    choice::<SingleCharMatcher, _>((
                        //
                        // Normal character
                        //
                        filter(|c| !SPECIAL_CHARS.contains(&c) && c != '/' && c != '\\').map(SingleCharMatcher::Literal),
                        //
                        // Escaped character
                        //
                        char('\\')
                            .ignore_then(
                                filter(|c| SPECIAL_CHARS.contains(&c) && c != '/' && c != '\\')
                                    .critical("expected a special character to escape"),
                            )
                            .map(SingleCharMatcher::Literal),
                        //
                        // Character class
                        //
                        just("[:")
                            .ignore_then(
                                choice::<CharacterClass, _>((
                                    just("alpha").to(CharacterClass::Alpha),
                                    just("digit").to(CharacterClass::Digit),
                                    just("alphanumeric").to(CharacterClass::Alphanumeric),
                                    just("uppercase").to(CharacterClass::Uppercase),
                                    just("lowercase").to(CharacterClass::Lowercase),
                                    just("whitespace").to(CharacterClass::Whitespace),
                                ))
                                .critical("expected a valid character class"),
                            )
                            .then_ignore(just(":]").critical_auto_msg())
                            .map(SingleCharMatcher::Class)
                    ))
                    .repeated_into_vec()
                    .at_least(1)
                    .critical("expected at least one character to match"),
                )
                .then_ignore(char(']').critical_auto_msg())
                .map(|(neg, chars)| {
                    if neg.is_some() {
                        CharsMatcher::NoneOfChars(chars)
                    } else {
                        CharsMatcher::OneOfChars(chars)
                    }
                }),
            //
            // Group alternates
            //
            char('{')
                .ignore_then(
                    chars_matcher
                        .repeated_into_vec()
                        .at_least(1)
                        .separated_by_into_vec(char('|'))
                        .at_least(2)
                        .critical("expected at least 2 alternative matchers"),
                )
                .then_ignore(char('}').critical_auto_msg())
                .map(CharsMatcher::OneOfGroups),
        ))
        });

        let dir_sep = silent_choice((char('/'), char('\\')));

        let component = choice::<RawComponent, _>((
            //
            // Wildcard
            //
            just("**")
                .followed_by(silent_choice((dir_sep, end())).critical(
                    "Wildcard components '**' must be preceded and followed by path separators",
                ))
                .map(|_| RawComponent::Wildcard),
            //
            // Character matchers
            //
            chars_matcher
                .repeated_into_vec()
                .map(|matchers| match matchers.as_slice() {
                    [] => RawComponent::Literal(String::new()),
                    [CharsMatcher::Literal(lit)] => RawComponent::Literal(lit.to_owned()),
                    _ => RawComponent::Suite(matchers),
                })
                .and_then_or_critical(|component| match component {
                    RawComponent::Literal(lit) if lit == "." || lit == ".." => Err(format!(
                        "Cannot match against '{lit}' after the beginning of the pattern"
                    )),

                    _ => Ok(component),
                }),
        ));

        let pattern_type = choice::<PatternType, _>((
            //
            // Absolute path (starts with a path separator)
            //
            dir_sep.to(PatternType::Absolute),
            //
            // Relative path starting from a parent directory (starts with one or more ".." components)
            //
            just(".")
                .then(dir_sep)
                .repeated()
                .ignore_then(
                    just("..")
                        .then(dir_sep)
                        // TODO: use counter container
                        .repeated_into_vec()
                        .map(|matches| matches.len()),
                )
                .try_map(NonZero::new)
                .map(|depth| PatternType::RelativeToParent { depth }),
            //
            // Relative path (does not start with a path separator)
            //
            just(".").then(dir_sep).repeated().to(PatternType::Relative),
        ));

        let pattern = pattern_type
            // TODO: remove empty components
            // TODO: for that, use a "VecWithFilter" alloc container
            .then(component.separated_by_into_vec(dir_sep))
            .map(|(pattern_type, components)| RawPattern {
                pattern_type,
                // TODO: then remove this
                components: components.into_iter().filter(|component| !matches!(component, RawComponent::Literal(str) if str.is_empty())).collect(),
            });

        Box::new(pattern.full())
    },
);

/// List of special characters that must be escaped in order to be matched against
static SPECIAL_CHARS: LazyLock<HashSet<char>> =
    LazyLock::new(|| HashSet::from(['[', ']', '{', '}', '*', '?', '\\', '/', '|', ':']));

/// A parsed raw pattern
///
/// This is intended to be compiled using the [`crate::compiler`] module to improve performance during matching.
#[derive(Debug)]
pub struct RawPattern {
    pub pattern_type: PatternType,
    pub components: Vec<RawComponent>,
}

#[derive(Debug, Clone, Copy)]
pub enum PatternType {
    /// The pattern is absolute (starts with a path separator such as '/')
    Absolute,

    /// The pattern is relative to a parent directory (starts with '../')
    RelativeToParent { depth: NonZero<usize> },

    /// The pattern is relative (does not start with a path separator or a '..' comopnents)
    Relative,
}

#[derive(Debug)]
pub enum RawComponent {
    /// The component matches a literal string
    Literal(String),

    /// The component matches using a suite of matchers
    Suite(Vec<CharsMatcher>),

    /// The component matches any suite of directories
    Wildcard,
}

#[derive(Debug)]
pub enum CharsMatcher {
    /// Match any single character
    AnyChar,

    /// Match any suite of characters, or no character at all
    AnyChars,

    /// Match a specific suite of characters
    Literal(String),

    /// Match a single character using one of the matchers
    OneOfChars(Vec<SingleCharMatcher>),

    /// Match any character that is *not* matched by one of the matchers
    NoneOfChars(Vec<SingleCharMatcher>),

    /// Match one of suites of character matchers
    OneOfGroups(Vec<Vec<CharsMatcher>>),
}

#[derive(Debug, Clone, Copy)]
pub enum SingleCharMatcher {
    /// Match a specific character
    Literal(char),

    /// Match a character using a given character class
    Class(CharacterClass),
}

#[derive(Debug, Clone, Copy)]
pub enum CharacterClass {
    /// Alphabetic characters
    Alpha,

    /// Digits
    Digit,

    /// Alphabetic characters and digits
    Alphanumeric,

    /// Uppercase characters
    Uppercase,

    /// Lowercase characters
    Lowercase,

    /// Whitespace characters
    Whitespace,
}
