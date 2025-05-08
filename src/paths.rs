use std::{
    ffi::OsString,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf},
};

use crate::opaque_os_str::OpaqueOsStr;

/// A normalized path
///
/// * The prefix is guaranteed to be supported by this crate
/// * All `.` and empty components have been removed
#[derive(Debug, Clone)]
pub struct NormalizedPath {
    prefix: Option<PathPrefix>,
    components: Vec<OsString>,
}

impl NormalizedPath {
    pub fn prefix(&self) -> Option<PathPrefix> {
        self.prefix
    }

    pub fn components(&self) -> &[OsString] {
        &self.components
    }

    pub fn to_path_buf(&self) -> PathBuf {
        let Self { prefix, components } = self;

        let mut path = match prefix {
            Some(prefix) => match prefix {
                PathPrefix::RootDir => PathBuf::from(MAIN_SEPARATOR_STR),
                PathPrefix::WindowsDrive(windows_drive) => PathBuf::from(format!(
                    "{}:{MAIN_SEPARATOR_STR}",
                    windows_drive.uppercase_letter(),
                )),
            },
            None => PathBuf::new(),
        };

        for component in components {
            path.push(component);
        }

        path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPrefix {
    RootDir,
    WindowsDrive(WindowsDrive),
}

/// A valid Windows drive
///
/// Represents any of the 26 uppercase letter from 'A' to 'Z'
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowsDrive(char);

impl WindowsDrive {
    // Get the drive's uppercase letter ('A' to 'Z')
    pub fn uppercase_letter(&self) -> char {
        self.0
    }
}

impl TryFrom<char> for WindowsDrive {
    type Error = InvalidWindowsDriveLetter;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        if value.is_ascii_alphabetic() {
            Ok(Self(value.to_ascii_uppercase()))
        } else {
            Err(InvalidWindowsDriveLetter)
        }
    }
}

/// A character that wasn't between 'A' and 'Z' was provided
#[derive(Debug, Clone, Copy)]
pub struct InvalidWindowsDriveLetter;

/// Normalize a path
///
/// * Extracts the prefix (root directory and `C:\`, `\\?\\C:\` syntaxes)
/// * Detects unsupported prefixes (e.g. `\\?\server\share`, `\\?\UNC\`, `\\.\device`)
/// * Removes empty and `.` components
pub fn normalize_path(path: &Path) -> Result<NormalizedPath, UnsupportedWindowsPrefix> {
    let path = OpaqueOsStr::new(path.as_os_str());

    let (prefix, path) = if let Some(path) = path.strip_prefix(b"\\\\") {
        let path = path.strip_prefix(b"?\\").ok_or(UnsupportedWindowsPrefix)?;

        // Expect and extract drive letter
        let (windows_drive, path) = strip_windows_drive(path).ok_or(UnsupportedWindowsPrefix)?;

        (
            Some(PathPrefix::WindowsDrive(windows_drive)),
            // Check for directory separator or end of path
            if path.is_empty() {
                path.to_static()
            } else {
                path.try_strip_prefixes([b'/', b'\\']).to_static()
            },
        )
    } else if let Some((windows_drive, path)) = strip_windows_drive(path.borrow()) {
        (
            Some(PathPrefix::WindowsDrive(windows_drive)),
            // Check for directory separator or end of path
            if path.is_empty() {
                path.to_static()
            } else {
                path.try_strip_prefixes([b'/', b'\\']).to_static()
            },
        )
    } else if let Some(path) = path.strip_prefix(b'/').or_else(|| path.strip_prefix(b'\\')) {
        (Some(PathPrefix::RootDir), path)
    } else {
        (None, path)
    };

    let mut components: Vec<OsString> = vec![];

    for component in path.split(|c| c == b'/' || c == b'\\') {
        match component.to_os_string().to_str() {
            Some("" | ".") => continue,

            _ => {
                components.push(component.to_os_string());
            }
        }
    }

    Ok(NormalizedPath { prefix, components })
}

/// Match and strip the Windows drive from the provided path
fn strip_windows_drive<'a>(path: OpaqueOsStr<'a>) -> Option<(WindowsDrive, OpaqueOsStr<'a>)> {
    let (char, path) = path.strip_ascii_char()?;

    let windows_drive = WindowsDrive::try_from(char).ok()?;

    let (_, path) = path.strip_ascii_char().filter(|(c, _)| *c == ':')?;

    // TODO: remove to_static
    Some((windows_drive, path.to_static()))
}

/// The provided path contains an invalid or unsupported Windows prefix (e.g. `\\?\server\share`, `\\?\UNC\`, `\\.\device`)
#[derive(Debug, Clone, Copy)]
pub struct UnsupportedWindowsPrefix;
