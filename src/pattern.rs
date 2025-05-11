use std::{
    ffi::OsString,
    path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf},
};

use parsy::ParsingError;

use crate::{
    compiler::{CaseSensitivity, Component, compile_component},
    parser::{PATTERN_PARSER, RawPattern},
    paths::{PathPrefix, normalize_path},
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
/// In addition, note that `**` will match any possible combination of directories. For instance, `/**/*.txt` will match any of `/file.txt`, `/dir/file.txt`, `/dir/dir2/file.txt`, and so on.
///
/// # Platform-specific support
///
/// * `/` and `\` are treated as path separators independently of the platform
/// * Absolute patterns can only be matched against absolute paths. e.g. `/dir` will not match `dir`. Note that using a [`crate::Walker`] will not cause this problem as a base directory is used.
/// * Absolute patterns can be matched against named drives in Windows, e.g. `\dir` will match against `C:\dir` (but not the opposite)
/// * Supported syntaxes for Windows drives are `C:\` and `\\?\C:\`
/// * Other verbatim paths such as `\\?\server\share` or `\\.\device` are unsupported
/// * Paths starting with `\\?\C:\` are normalized like any other path

#[derive(Debug, Clone)]
pub struct Pattern {
    /// Does the pattern start with a prefix component?
    prefix: Option<PathPrefix>,

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

    /// Does the pattern contain a wildcard?
    /// For more informations, see [`Pattern::has_wildcard`]
    has_wildcard: bool,
}

impl Pattern {
    /// Parse a pattern with the default options
    pub fn new(input: &str) -> Result<Self, ParsingError> {
        Self::new_with_opts(input, PatternOpts::default())
    }

    /// Parse a pattern
    pub fn new_with_opts(input: &str, opts: PatternOpts) -> Result<Self, ParsingError> {
        let PatternOpts { case_insensitive } = opts;

        let RawPattern { components, prefix } =
            PATTERN_PARSER.parse_str(input).map(|parsed| parsed.data)?;

        // Compile each individual comopnent
        let components: Vec<_> = components
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
            common_root_dir: build_common_root_dir(prefix, &components),
            prefix,
            has_wildcard: components.iter().any(|c| matches!(c, Component::Wildcard)),
            components,
        })
    }

    /// Check if the pattern is absolute (only matches absolute paths)
    pub fn is_absolute(&self) -> bool {
        self.prefix.is_some()
    }

    /// Get the path prefix
    pub fn prefix(&self) -> Option<PathPrefix> {
        self.prefix
    }

    /// Match the pattern against a path
    ///
    /// Note that the path should be normalized.
    /// For instance, '..' components in the pattern will be matched against literal '..' in the path.
    pub fn is_match(&self, path: &Path) -> bool {
        matches!(self.match_against(path), PatternMatchResult::Matched)
    }

    pub fn match_against(&self, path: &Path) -> PatternMatchResult {
        let Ok(path) = normalize_path(path) else {
            return PatternMatchResult::IncompatiblePrefix;
        };

        let is_absolute = path.prefix().is_some();

        match &self.prefix {
            Some(PathPrefix::RootDir) => {
                if !is_absolute {
                    return PatternMatchResult::PathNotAbsolute;
                }
            }

            Some(PathPrefix::WindowsDrive(windows_drive)) => match path.prefix() {
                Some(prefix) => match prefix {
                    PathPrefix::RootDir => return PatternMatchResult::IncompatiblePrefix,

                    PathPrefix::WindowsDrive(path_windows_drive) => {
                        if *windows_drive != path_windows_drive {
                            return PatternMatchResult::NotMatched;
                        }
                    }
                },

                None => return PatternMatchResult::PathNotAbsolute,
            },

            None => {
                if is_absolute {
                    return PatternMatchResult::PathIsAbsolute;
                }
            }
        }

        match_components(&self.components, path.components())
    }

    /// Get the common root directory for all possible matches of this pattern
    pub fn common_root_dir(&self) -> &Path {
        &self.common_root_dir
    }

    /// Check if the component contains a wildcard
    ///
    /// Can be useful for e.g. determining if a matching directory should be traversed or not,
    /// as the presence or absence of a wildcard indicates whether descendants may match
    ///
    /// Example:
    /// * `/a/b` matches `/a/b` but cannot match any descendant
    /// * `/a/**/b` matches `/a/b` and may match some descendants
    /// * `/a/b/**` matches `/a/b` and may match some descendants
    pub fn has_wildcard(&self) -> bool {
        self.has_wildcard
    }
}

fn build_common_root_dir(prefix: Option<PathPrefix>, components: &[Component]) -> PathBuf {
    // Get all deterministic components at the beginning of the pattern
    // These will be used to compute the common root directory
    let mut common_root_dir_components = components
        .iter()
        .map_while(|component| match component {
            // Only get literal components, as these will always match the exact same path components
            Component::Literal(lit) => Some(lit.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    // If the entire pattern is deterministic, match from the parent directory to allow yielding
    // that specific child item
    if common_root_dir_components.len() == components.len() {
        common_root_dir_components.pop();
    }

    // Build the common root directory
    let mut common_root_dir = match prefix {
        Some(prefix) => match prefix {
            PathPrefix::RootDir => MAIN_SEPARATOR_STR.to_owned(),
            PathPrefix::WindowsDrive(drive_letter) => {
                format!("{}:", drive_letter.uppercase_letter())
            }
        },

        None => String::new(),
    };

    for (i, common_root_dir_component) in common_root_dir_components.iter().enumerate() {
        if i > 0 {
            common_root_dir.push(MAIN_SEPARATOR);
        }

        common_root_dir.push_str(common_root_dir_component);
    }

    PathBuf::from(common_root_dir)
}

fn match_components(components: &[Component], mut path: &[OsString]) -> PatternMatchResult {
    for i in 0..components.len() {
        match &components[i] {
            Component::Wildcard => {
                if components[i + 1..].is_empty() {
                    return PatternMatchResult::Matched;
                }

                if path.is_empty() {
                    return if components[i + 1..].iter().any(|component| match component {
                        Component::Regex(_) | Component::Literal(_) | Component::ParentDir => true,
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
                        | PatternMatchResult::PathIsAbsolute
                        | PatternMatchResult::IncompatiblePrefix => unreachable!(),

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

                if part.as_encoded_bytes() != lit.as_bytes() {
                    return PatternMatchResult::NotMatched;
                }
            }

            Component::ParentDir => {
                let Some(part) = path.first() else {
                    return PatternMatchResult::NotMatched;
                };

                path = &path[1..];

                if part.as_encoded_bytes() != "..".as_bytes() {
                    return PatternMatchResult::NotMatched;
                }
            }

            Component::Regex(regex) => {
                let Some(part) = path.first() else {
                    return PatternMatchResult::Starved;
                };

                path = &path[1..];

                if !regex.is_match(part.as_encoded_bytes()) {
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

    /// Failed as the provided path uses a different path prefix than the pattern's expected one
    IncompatiblePrefix,

    /// Pattern matched against the provided path
    Matched,

    /// Pattern did not match against the provided path
    NotMatched,

    /// Pattern did not match against the provided path because of starvation
    /// This means the pattern *may* match against a descendant of the provided path
    Starved,
}
