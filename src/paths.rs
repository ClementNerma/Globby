use std::{
    ffi::OsString,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf},
};

use crate::opaque_os_str::OpaqueOsStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPrefix {
    RootDir,
    WindowsDrive(WindowsDrive),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowsDrive(char);

impl WindowsDrive {
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

#[derive(Debug, Clone, Copy)]
pub struct InvalidWindowsDriveLetter;

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

pub fn normalize_path(path: &Path) -> Result<NormalizedPath, UnsupportedWindowsPrefix> {
    let path = OpaqueOsStr::new(path.as_os_str());

    let (prefix, path) = if let Some(path) = path.strip_prefix(b"\\\\") {
        let path = path.strip_prefix(b"?\\").ok_or(UnsupportedWindowsPrefix)?;

        // Expect and extract drive letter
        let (windows_drive, path) = extract_windows_drive(&path).ok_or(UnsupportedWindowsPrefix)?;

        (
            Some(PathPrefix::WindowsDrive(windows_drive)),
            // Check for directory separator or end of path
            if path.is_empty() {
                path.to_static()
            } else {
                path.try_strip_prefixes([b'/', b'\\']).to_static()
            },
        )
    } else if let Some((windows_drive, path)) = extract_windows_drive(&path) {
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

fn extract_windows_drive<'a>(path: &'a OpaqueOsStr) -> Option<(WindowsDrive, OpaqueOsStr<'a>)> {
    let (char, path) = path.strip_ascii_char()?;

    let windows_drive = WindowsDrive::try_from(char).ok()?;

    let (_, path) = path.strip_ascii_char().filter(|(c, _)| *c == ':')?;

    // TODO: remove to_static
    Some((windows_drive, path.to_static()))
}

#[derive(Debug, Clone, Copy)]
pub struct UnsupportedWindowsPrefix;
