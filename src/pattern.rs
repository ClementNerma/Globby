use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf, PrefixComponent},
};

use parsy::ParsingError;

use crate::{
    compiler::{CaseSensitivity, Component, compile_component},
    parser::{PATTERN_PARSER, PatternType, RawComponent, RawPattern},
};

/// Options for pattern matching
#[derive(Debug, Default, Clone, Copy)]
pub struct PatternOpts {
    /// Ignore case sensitivity during matching
    ///
    /// This makes `a` match both lowercase `a` and uppercase `A`
    ///
    /// Disabled by default
    pub case_insensitive: bool,
}

/// A pattern that can be matched against filesystem paths
///
/// # Syntax
///
/// The syntax for patterns is similar to [Linux' glob](https://man7.org/linux/man-pages/man7/glob.7.html), with a few differences.
///
/// * Normal characters behave as expected
/// * `?` matches any character
/// * `*` matches any suite of characters, or no character at all
/// * `[abc]` matches any of `a`, `b` or `c`
/// * `[!abc]` matches any character except `a`, `b` and `c`
/// * `[\[]` matches `[`. The list of escapable characters is `[`, `]`, `{`, `}`, `*`, `?`, `\`, `/`, `|` and ':'
///     - `[abc\[]` matches any of `a`, `b`, `c` or `[`
/// * `[[:alpha:]]` will match any alphabetic character. The list of character classes are:
///     - `:alpha:` for any alphabetic character
///     - `:digit:` for any digit
///     - `:alphanumeric:` for any alphabetic character or digit
///     - `:uppercase:` for any uppercase character
///     - `:lowercase:` for any lowercase character
///     - `:whitespace:` for any whitespace character
/// * `[![:alpha:]]` will match any non-alphabetic character
/// * `{a|bc}` will match any of `a` or `bc`
///     - This can be combined with other matchers, e.g. `{[[:alpha:]][![:digit]]|[[:digit:]]*}` will match any alphabetic character followed by a non-digit character, OR a digit followed by anything
///
/// Matches are performed against path components, e.g. in `/path/to/item` components are `path`, `to` and `item`.
/// Matchers **cannot** match path separators.
///
/// * Path separators can be written as `/` or `\` no matter the platform (Windows, Linux, macOS, ...)
/// * In addition, `**` will match any possible combination of directories. For instance, `/**/*.txt` will match any of `/file.txt`, `/dir/file.txt`, `/dir/dir2/file.txt`, and so on.
/// * Absolute patterns can only be matched against absolute paths. e.g. `/dir` will not match `dir`. Note that using a [`crate::Walker`] will not cause this problem as a base directory is used.

#[derive(Debug, Clone)]
pub struct Pattern {
    /// Type of the pattern
    pattern_type: PatternType,

    /// Root directory that's common to all possible matches of a pattern
    ///
    /// e.g. The root directory of `/a/{b, c}` will be `/a`
    ///
    /// This is useful to determine where to start directory traversal from
    common_root_dir: PathBuf,

    /// The components that make up the pattern
    ///
    /// Each of them match a single path component (except the wildcard matcher)
    components: Vec<Component>,
}

impl Pattern {
    /// Parse a pattern with the default options
    pub fn new(input: &str) -> Result<Self, ParsingError> {
        Self::new_with_opts(input, PatternOpts::default())
    }

    /// Parse a pattern
    pub fn new_with_opts(input: &str, opts: PatternOpts) -> Result<Self, ParsingError> {
        let PatternOpts { case_insensitive } = opts;

        let RawPattern {
            pattern_type,
            components,
        } = PATTERN_PARSER.parse_str(input).map(|parsed| parsed.data)?;

        // Get all deterministic components at the beginning of the pattern
        // These will be used to compute the common root directory
        let mut common_root_dir_components = components
            .iter()
            .map_while(|component| match component {
                // Only get literal components, as these will always match the exact same path components
                RawComponent::Literal(lit) => Some(lit.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        // If the entire pattern is deterministic, match from the parent directory to allow yielding
        // that specific child item
        if common_root_dir_components.len() == components.len() {
            common_root_dir_components.pop();
        }

        // Build the common root directory
        let common_root_dir = common_root_dir_components.join(MAIN_SEPARATOR_STR);

        let common_root_dir = match pattern_type {
            PatternType::Absolute => format!("/{common_root_dir}"),
            PatternType::RelativeToParent { depth } => format!(
                "{}{MAIN_SEPARATOR_STR}{common_root_dir}",
                // Prefix the common directory with repeated "../"
                format!("..{}", MAIN_SEPARATOR_STR).repeat(depth.into()),
            ),
            PatternType::Relative => common_root_dir,
        };

        // Compile each individual comopnent
        let components = components
            .into_iter()
            .map(|component| {
                compile_component(
                    component,
                    // Provide compilation options
                    if case_insensitive {
                        CaseSensitivity::Insensitive
                    } else {
                        CaseSensitivity::Sensitive
                    },
                )
            })
            .collect();

        Ok(Self {
            pattern_type,
            common_root_dir: PathBuf::from(common_root_dir),
            components,
        })
    }

    /// Match the pattern against a path
    ///
    /// Note that the path should be normalized.
    /// For instance, '..' components in the pattern will be matched against literal '..' in the path.
    pub fn is_match(&self, path: &Path) -> bool {
        matches!(self.match_against(path), PatternMatchResult::Matched)
    }

    pub fn match_against(&self, path: &Path) -> PatternMatchResult {
        if matches!(self.pattern_type, PatternType::Absolute) && !path.is_absolute() {
            return PatternMatchResult::PathNotAbsolute;
        }

        if !matches!(self.pattern_type, PatternType::Absolute) && path.is_absolute() {
            return PatternMatchResult::PathIsAbsolute;
        }

        let (prefix, path_components) = simplify_path_components(path);

        if prefix.is_some() {
            todo!("TODO: handle Windows prefixes");
        }

        match_components(&self.components, &path_components)
    }

    pub fn common_root_dir(&self) -> &Path {
        &self.common_root_dir
    }

    pub fn pattern_type(&self) -> PatternType {
        self.pattern_type
    }
}

fn simplify_path_components(path: &Path) -> (Option<PrefixComponent>, Vec<&OsStr>) {
    use std::path::Component;

    let mut components_iter = path.components();

    let Some(first_component) = components_iter.next() else {
        // Cannot match against empty paths
        return (None, vec![]);
    };

    let mut normalized_components = vec![];

    let prefix = match first_component {
        Component::Prefix(prefix) => Some(prefix),
        Component::RootDir | Component::CurDir => None,

        Component::ParentDir => {
            normalized_components.push(OsStr::new(".."));
            None
        }

        Component::Normal(os_str) => {
            normalized_components.push(os_str);
            None
        }
    };

    for component in components_iter {
        match component {
            Component::Prefix(_) | Component::RootDir => unreachable!(),
            Component::CurDir => continue,
            Component::ParentDir => {
                normalized_components.push(OsStr::new(".."));
            }
            Component::Normal(os_str) => normalized_components.push(os_str),
        }
    }

    (prefix, normalized_components)
}

fn match_components(components: &[Component], mut path: &[&OsStr]) -> PatternMatchResult {
    for i in 0..components.len() {
        match &components[i] {
            Component::Wildcard => {
                if components[i + 1..].is_empty() {
                    return PatternMatchResult::Matched;
                }

                if path.is_empty() {
                    return if components[i + 1..].iter().any(|component| match component {
                        Component::Regex(_) | Component::Literal(_) => true,
                        Component::Wildcard => false,
                    }) {
                        PatternMatchResult::Starved
                    } else {
                        PatternMatchResult::Matched
                    };
                }

                for j in 0..path.len() {
                    match match_components(&components[i + 1..], &path[j..]) {
                        PatternMatchResult::PathNotAbsolute
                        | PatternMatchResult::PathIsAbsolute => unreachable!(),
                        PatternMatchResult::Matched => return PatternMatchResult::Matched,
                        PatternMatchResult::NotMatched | PatternMatchResult::Starved => {}
                    }
                }

                return PatternMatchResult::Starved;
            }

            Component::Literal(lit) => {
                let Some(part) = path.first() else {
                    return PatternMatchResult::Starved;
                };

                path = &path[1..];

                if *part != OsStr::new(lit) {
                    return PatternMatchResult::NotMatched;
                }
            }

            Component::Regex(regex) => {
                let Some(part) = path.first() else {
                    return PatternMatchResult::Starved;
                };

                path = &path[1..];

                if !regex.is_match(part.as_bytes()) {
                    return PatternMatchResult::NotMatched;
                }
            }
        }
    }

    if path.is_empty() {
        PatternMatchResult::Matched
    } else {
        PatternMatchResult::NotMatched
    }
}

/// Result of a pattern matching against a path
#[derive(Debug, Clone, Copy)]
pub enum PatternMatchResult {
    /// Failed as the provided path is relative while the pattern only matches absolute paths
    PathNotAbsolute,

    /// Failed as the provided path is absolute while the pattern only matches relative paths
    PathIsAbsolute,

    /// Pattern matched against the provided path
    Matched,

    /// Pattern did not match against the provided path
    NotMatched,

    /// Pattern did not match against the provided path because of starvation
    /// This means the pattern *may* match against a descendant of the provided path
    Starved,
}
