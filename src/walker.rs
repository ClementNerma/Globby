use std::{
    ffi::OsStr,
    fs::{ReadDir, canonicalize},
    path::{Path, PathBuf},
};

use crate::{Pattern, normalize_path, paths::NormalizedPath, pattern::PatternMatchResult};

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
/// - No guarantee is given as for the order the results are yielded in
pub struct Walker {
    /// Set to [`None`] if the walker cannot apply, e.g. if the base directory does not exist
    state: Option<WalkerState>,
}

/// (Internal) Walker state
struct WalkerState {
    /// The pattern to apply to all entries
    pattern: Pattern,

    /// Directory we're walking from (canonicalized)
    walk_from: NormalizedPath,

    /// Prefix to add to all paths before pattern matching
    ///
    /// The reason this exists is as follows:
    /// * Let's say we have a base directory of '/a/b/c'
    /// * The pattern is '../**/*'
    /// * Now let's say our base directory is '/a/b'
    /// * When resolving e.g. `/a/b/c/d` from the parent, the relative path compared to the base directory
    ///   will be `d`, whereas we want `../c/d`
    ///
    /// So we prepare a prefix to join to all paths to make them comparable.
    /// In our example, the prefix would be equal to `..` and the path provided to the pattern matcher
    /// would be `../c/d`
    parent_prefix: PathBuf,

    /// Directory readers, recursively
    open_dirs: Vec<ReadDir>,

    /// Are we going into a directory?
    going_into_dir: Option<PathBuf>,
}

impl Walker {
    /// Create a walker that will yield filesystem entries that match the provided pattern
    pub fn new(pattern: Pattern, base_dir: &Path) -> Self {
        Self::new_inner(pattern, base_dir).unwrap_or(Self { state: None })
    }

    fn new_inner(pattern: Pattern, base_dir: &Path) -> Option<Self> {
        let base_dir = canonicalize(base_dir).ok()?;

        let walk_from = base_dir.join(pattern.common_root_dir());

        // Simplify the base directory, as to have an absolute path,
        // and avoid components like `.` or `..`
        let walk_from = normalize_path(&walk_from).ok()?;

        Some(Walker {
            state: Some(WalkerState {
                parent_prefix: diff_path(&walk_from, &normalize_path(&base_dir).unwrap()),
                going_into_dir: Some(walk_from.to_path_buf()),
                pattern,
                walk_from,
                open_dirs: vec![],
            }),
        })
    }

    pub fn is_invalid(&self) -> bool {
        self.state.is_none()
    }
}

impl Iterator for Walker {
    type Item = Result<PathBuf, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let state = self.state.as_mut()?;

        loop {
            // Check if we're going into a directory
            if let Some(going_into_dir) = state.going_into_dir.take() {
                match std::fs::read_dir(&going_into_dir) {
                    Err(err) => return Some(Err(err)),
                    Ok(reader) => {
                        state.open_dirs.push(reader);
                        continue;
                    }
                }
            }

            // Otherwise, get the currently handled directory's reader
            let queue = state.open_dirs.last_mut()?;

            let Some(entry) = queue.next() else {
                // If the reader is empty, remove it from the last
                state.open_dirs.pop();
                // then get to use the next reader
                continue;
            };

            // Get the successful entry or return the error
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => return Some(Err(err)),
            };

            // Compute the real entry path, as the walker only provides something relative to the base *walking* directory
            let entry_path = normalize_path(&entry.path()).unwrap();

            // Compute the path relative to the base directory (if the pattern is not absolute)
            let entry_path = if state.pattern.is_absolute() {
                entry_path.to_path_buf()
            } else {
                state
                    .parent_prefix
                    .join(diff_path(&entry_path, &state.walk_from))
            };

            // Check if the path matches the provided globbing pattern
            match state.pattern.match_against(&entry_path) {
                // Absolute path conflict should not happen as it's been taken care of ahead of matching
                PatternMatchResult::PathNotAbsolute
                | PatternMatchResult::PathIsAbsolute
                | PatternMatchResult::IncompatiblePrefix => {
                    unreachable!()
                }

                // Success!
                PatternMatchResult::Matched => {
                    // If the pattern contains no wildcard, no descendant of this path may be matched
                    // by the pattern, so if it's a directory, we can skip it
                    // Otherwise, we'll need to traverse it
                    if entry.path().is_dir() && state.pattern.has_wildcard() {
                        state.going_into_dir = Some(entry.path());
                    }

                    return Some(Ok(entry_path));
                }

                // May have matched if the path was more complete, so we just do nothing
                PatternMatchResult::Starved => {
                    if entry.path().is_dir() {
                        state.going_into_dir = Some(entry.path());
                    }
                }

                // Failed to match and not starved, so we simply ignore this entry
                PatternMatchResult::NotMatched => {}
            }
        }
    }
}

fn diff_path(path: &NormalizedPath, base: &NormalizedPath) -> PathBuf {
    assert!(path.prefix().is_some());
    assert!(base.prefix().is_some());
    assert_eq!(path.prefix(), base.prefix());

    let mut ita = path.components().iter();
    let mut itb = base.components().iter();

    let mut comps = PathBuf::new();

    loop {
        match (ita.next(), itb.next()) {
            (None, None) => break,

            (Some(a), None) => {
                comps.push(a);
                comps.extend(ita.by_ref());
                break;
            }

            (None, _) => comps.push(OsStr::new("..")),

            (Some(a), Some(b)) if comps.components().count() == 0 && a == b => (),

            (Some(a), Some(_)) => {
                comps.push(OsStr::new(".."));

                for _ in itb {
                    comps.push(OsStr::new(".."));
                }

                comps.push(a);

                comps.extend(ita.by_ref());

                break;
            }
        }
    }

    comps
}
