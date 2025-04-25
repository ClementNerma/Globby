use std::{
    fs::canonicalize,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf},
};

use crate::{Pattern, fs_walker::FsWalker, parser::PatternType, pattern::PatternMatchResult};

/// Walker implementation, yielding filesystem entries that match the provided pattern
///
/// Walks recursively from the provided base directory if the pattern is not absolute, or from the path's
/// root point otherwise.
///
/// For instance, a pattern written as `/**/*` will make the walker from `/` no matter what the provided
/// base directory is.
///
/// Yielded results may be [`Err`] variants in case something goes wrong while fetching informations about
/// the related path.
///
/// For more informations on how pattern matching works, see [`Pattern`].
///
/// # Path relativility
///
/// * If the pattern is absolute (starts with a `/`), all yielded results will be absolute. Otherwise, yielded results will be relative to the provided base directory.
/// * If the pattern starts with an ancestor (`../`), yielded results will not be simplified. e.g. starting from directory `/a/b` and matching `../*` will yield `../b` results instead of `.`
///
/// # Ordering and traversal rules
///
/// - Directories are always yielded before their content
/// - Symbolic links are always followed
/// - The base directory is not yielded in the results
/// - The iterator yields errors first, then entries in alphabetical order (see [`String::cmp`])
pub struct Walker {
    /// Set to [`None`] if the walker cannot apply, e.g. if the base directory does not exist
    state: Option<WalkerState>,
}

/// (Internal) Walker state
struct WalkerState {
    pattern: Pattern,

    /// Underlying filesystem walker
    fs_walker: FsWalker,

    /// Directory to strip, if the base directory the pattern is not absolute
    strip_dir: Option<PathBuf>,
}

impl Walker {
    /// Create a walker that will yield filesystem entries that match the provided pattern
    pub fn new(pattern: Pattern, base_dir: &Path) -> Self {
        let walk_from = base_dir.join(pattern.common_root_dir());

        // Canonicalize the base directory, as to have an absolute path,
        // and avoid components like `.` or `..`
        let Ok(walk_from) = canonicalize(&walk_from) else {
            return Self { state: None };
        };

        // Determine the portion of the path to strip from yielded results
        let strip_dir = if pattern.common_root_dir().is_absolute() {
            // If the pattern is absolute, strip nothing, as we're starting from the root directory
            None
        } else {
            // Otherwise, strip by default the base directory, as we're walking from here
            let mut base_dir = base_dir;

            // If the pattern is relative to a parent (e.g. the pattern starts with `../`), change the
            // directory to strip to a parent
            if let PatternType::RelativeToParent { depth } = pattern.pattern_type() {
                for _ in 0..depth.into() {
                    let Some(parent) = base_dir.parent() else {
                        return Self { state: None };
                    };

                    base_dir = parent;
                }
            }

            Some(base_dir.to_owned())
        };

        Self {
            state: Some(WalkerState {
                pattern,
                fs_walker: FsWalker::new(walk_from),
                strip_dir,
            }),
        }
    }
}

impl Iterator for Walker {
    type Item = Result<PathBuf, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let state = self.state.as_mut()?;

        loop {
            // Get the next entry from the walker
            let entry = match state.fs_walker.next()? {
                Ok(entry) => entry,
                Err(err) => return Some(Err(err)),
            };

            // Compute the real entry path, as the walker only provides something relative to the base directory
            let entry_path = entry
                .path()
                // Also apply directory stripping (if the base directory is not absolute)
                .strip_prefix(match state.strip_dir.as_deref() {
                    Some(path) => path,
                    None => Path::new(""),
                })
                .unwrap()
                .to_owned();

            // Check if the path matches the provided globbing pattern
            match state.pattern.match_against(&entry_path) {
                // Absolute path conflict should not happen as it's been taken care of ahead of matching
                PatternMatchResult::PathNotAbsolute | PatternMatchResult::PathIsAbsolute => {
                    unreachable!()
                }

                // Success!
                PatternMatchResult::Matched => {
                    return Some(Ok(match state.pattern.pattern_type() {
                        // Make the yielded path relative to the base directory by adding '../' prefixes
                        PatternType::RelativeToParent { depth } => {
                            // TODO: cache this string to avoid runtime formatting overhead
                            let prefix = format!("..{MAIN_SEPARATOR_STR}").repeat(depth.into());

                            Path::new(&prefix).join(entry_path)
                        }

                        _ => entry_path,
                    }));
                }

                // Failed to match
                PatternMatchResult::NotMatched => {
                    // Skip sub-directory traversal as no child would have matched anyway
                    if entry.path().is_dir() {
                        state.fs_walker.skip_incoming_dir().unwrap();
                    }
                }

                // May have matched if the path was more complete, so we just do nothing
                PatternMatchResult::Starved => {}
            }
        }
    }
}
