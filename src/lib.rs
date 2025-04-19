//! Globby is a library designed for searching all items in a given directory that match a *glob pattern*.
//!
//! # Examples
//!
//! ```rust
//! use globby::{PatternOpts, glob_current_dir};
//!
//! let pattern = glob_current_dir("**/*.*").unwrap();
//!
//! for path in pattern {
//!   println!("{}", path.unwrap().display());
//! }
//! ```
//!
//! This library should work on any platform.
//!
//! # Comparing to [`glob`](https://docs.rs/glob)
//!
//! The well-known glob library is more polished and has a lot more options, but also opinionated defaults that differ from this library, such as:
//!
//! * The base directory is not yielded in the results
//! * Symbolic links are always followed
//! * Directories are always yielded before their descendents
//! * Alternate groups (matching either one sub-pattern or another) is supported
//! * `**` matches anything, including files an hidden directories
//!
//!
//! # Syntax
//!
//! See [`Pattern`].

#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]

mod compiler;
mod fs_walker;
mod parser;
mod pattern;
mod walker;

use std::path::Path;

use parsy::ParsingError;

pub use self::{
    fs_walker::NoIncomingDir,
    pattern::{Pattern, PatternMatchResult, PatternOpts},
    walker::Walker,
};

/// Match a pattern against a directory
///
/// For details on how patterns are applied, see [`Walker::new`]
pub fn glob(pattern: &str, dir: &Path) -> Result<Walker, GlobError> {
    let pattern = Pattern::new(pattern).map_err(GlobError::InvalidPattern)?;

    Ok(Walker::new(pattern, dir))
}

/// Match a pattern against a directory
///
/// For details on how patterns are applied, see [`Walker::new`]
pub fn glob_with(pattern: &str, dir: &Path, opts: PatternOpts) -> Result<Walker, GlobError> {
    let pattern = Pattern::new_with_opts(pattern, opts).map_err(GlobError::InvalidPattern)?;

    Ok(Walker::new(pattern, dir))
}

/// Match a pattern against the current directory
///
/// Strictly equivalent to calling [`glob`] with the canonicalized path to the current directory
///
/// For details on how patterns are applied, see [`Walker`]
pub fn glob_current_dir(pattern: &str) -> Result<Walker, GlobError> {
    let current_dir = std::env::current_dir().map_err(GlobError::WalkerError)?;

    glob(pattern, &current_dir)
}

/// Error occuring during glob execution
#[derive(Debug)]
pub enum GlobError {
    /// The provided pattern is invalid
    InvalidPattern(ParsingError),

    /// An error occurred while traversing the filesystem
    WalkerError(std::io::Error),
}
