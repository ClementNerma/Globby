use std::{
    fs::canonicalize,
    path::{Path, PathBuf},
};

use crate::{Pattern, fs_walker::FsWalker, pattern::PatternMatchResult};

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
    pattern: Pattern,

    /// Underlying filesystem walker
    fs_walker: FsWalker,

    /// Base directory (canonicalized)
    base_dir: PathBuf,
}

impl Walker {
    /// Create a walker that will yield filesystem entries that match the provided pattern
    pub fn new(pattern: Pattern, base_dir: &Path) -> Self {
        let Ok(base_dir) = canonicalize(base_dir) else {
            return Self { state: None };
        };

        let walk_from = base_dir.join(pattern.common_root_dir());

        // Canonicalize the base directory, as to have an absolute path,
        // and avoid components like `.` or `..`
        let Ok(walk_from) = canonicalize(&walk_from) else {
            return Self { state: None };
        };

        Self {
            state: Some(WalkerState {
                pattern,
                base_dir,
                fs_walker: FsWalker::new(walk_from),
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

            // Compute the real entry path, as the walker only provides something relative to the base *walking* directory
            let entry_path = canonicalize(entry.path()).unwrap();

            if entry_path == state.base_dir {
                continue;
            }

            let entry_path = if state.pattern.is_absolute() {
                entry_path
            } else {
                diff_path(&entry_path, &state.base_dir)
            };

            // Check if the path matches the provided globbing pattern
            match state.pattern.match_against(&entry_path) {
                // Absolute path conflict should not happen as it's been taken care of ahead of matching
                PatternMatchResult::PathNotAbsolute | PatternMatchResult::PathIsAbsolute => {
                    unreachable!()
                }

                // Success!
                PatternMatchResult::Matched => {
                    return Some(Ok(entry_path));
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

pub fn diff_path(path: &Path, base: &Path) -> PathBuf {
    assert!(path.is_absolute());
    assert!(base.is_absolute());

    let mut ita = path.components();
    let mut itb = base.components();

    use std::path::Component;

    let mut comps: Vec<Component> = vec![];

    loop {
        match (ita.next(), itb.next()) {
            (None, None) => break,

            (Some(a), None) => {
                comps.push(a);
                comps.extend(ita.by_ref());
                break;
            }

            (None, _) => comps.push(Component::ParentDir),

            (Some(a), Some(b)) if comps.is_empty() && a == b => (),

            (Some(a), Some(Component::CurDir)) => comps.push(a),

            (Some(_), Some(Component::ParentDir)) => unreachable!(),

            (Some(a), Some(_)) => {
                comps.push(Component::ParentDir);

                for _ in itb {
                    comps.push(Component::ParentDir);
                }

                comps.push(a);

                comps.extend(ita.by_ref());

                break;
            }
        }
    }

    comps.iter().map(|c| c.as_os_str()).collect()
}
