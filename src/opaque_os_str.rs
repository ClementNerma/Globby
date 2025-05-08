use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

/// An immutable type representing an opaque OS string
///
/// This enables manipulating platform-specific strings on e.g. Windows and Unix
///
/// Unlike [`OsString`], this type supports byte-level operations and encoding
///
/// The type's content remains opaque as it is platform-specific
#[derive(Debug)]
pub struct OpaqueOsStr<'a> {
    inner: Cow<'a, [u16]>,
}

impl<'a> OpaqueOsStr<'a> {
    /// Create a new opaque OS string
    ///
    /// Various FFI APIs are used to perform the conversion internally
    pub fn new(os_str: &OsStr) -> Self {
        let bytes = {
            #[cfg(target_family = "windows")]
            {
                use std::os::windows::ffi::OsStrExt;

                os_str.encode_wide().collect::<Vec<_>>()
            }

            #[cfg(target_family = "unix")]
            {
                use std::os::unix::ffi::OsStrExt;

                os_str
                    .as_bytes()
                    .iter()
                    .map(|byte| u16::from(*byte))
                    .collect::<Vec<_>>()
            }

            #[cfg(all(not(target_family = "unix"), not(target_family = "windows")))]
            {
                compile_error!("Unsupported platform! Only Unix and Windows are supported.");
            }
        };

        Self {
            inner: Cow::Owned(bytes),
        }
    }

    /// Convert back to an [`OsString`]
    ///
    /// This is guaranteed to give back the same string as [`Self::new`]
    pub fn to_os_string(&self) -> OsString {
        #[cfg(target_family = "windows")]
        {
            use std::os::windows::ffi::OsStringExt;

            OsString::from_wide(self.inner.as_ref())
        }

        #[cfg(target_family = "unix")]
        {
            use std::os::unix::ffi::OsStringExt;

            OsString::from_vec(
                self.inner
                    .iter()
                    .map(|o| u8::try_from(*o).unwrap())
                    .collect::<Vec<_>>(),
            )
        }

        #[cfg(all(not(target_family = "unix"), not(target_family = "windows")))]
        {
            compile_error!("Unsupported platform! Only Unix and Windows are supported.");
        }
    }

    /// Borrow this opaque string
    ///
    /// This is akin to cloning, but doesn't require allocating and allows changing the lifetime when required
    pub fn borrow(&self) -> OpaqueOsStr {
        OpaqueOsStr {
            inner: Cow::Borrowed(self.inner.as_ref()),
        }
    }

    /// Get a 'static variant of this string
    ///
    /// Will require cloning or allocating for the inner value
    pub fn to_static(&self) -> OpaqueOsStr<'static> {
        OpaqueOsStr {
            inner: Cow::Owned(self.inner.clone().into_owned()),
        }
    }

    /// Strip a prefix using the provided pattern
    pub fn strip_prefix(&'a self, pattern: impl StripPattern) -> Option<Self> {
        pattern.strip_prefix(self.inner.as_ref()).map(|inner| Self {
            inner: Cow::Borrowed(inner),
        })
    }

    /// Strip the first byte if it's a valid ASCII character
    pub fn strip_ascii_char(&'a self) -> Option<(char, Self)> {
        self.inner
            .first()
            .and_then(|c| char::from_u32(u32::from(*c as u8)))
            .map(|c| {
                (
                    c,
                    OpaqueOsStr {
                        inner: Cow::Borrowed(&self.inner[1..]),
                    },
                )
            })
    }

    /// Try stripping any of the provided prefixes
    pub fn try_strip_prefixes<const N: usize>(
        &'a self,
        patterns: [impl StripPattern; N],
    ) -> OpaqueOsStr<'a> {
        for pattern in patterns {
            if let Some(stripped) = pattern.strip_prefix(self.inner.as_ref()) {
                return OpaqueOsStr {
                    inner: Cow::Borrowed(stripped),
                };
            }
        }

        self.borrow()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Split the string using the given predicate
    pub fn split(&self, predicate: impl Fn(u8) -> bool) -> impl Iterator<Item = OpaqueOsStr> {
        self.inner
            .split(move |b| predicate(*b as u8))
            .map(|opaques| OpaqueOsStr {
                inner: Cow::Borrowed(opaques),
            })
    }
}

/// A pattern used to strip bytes from an [`OpaqueOsStr`]
pub trait StripPattern {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]>;
}

impl StripPattern for u8 {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]> {
        value.strip_prefix(&[u16::from(*self)])
    }
}

impl StripPattern for &[u8] {
    fn strip_prefix<'a>(&self, mut value: &'a [u16]) -> Option<&'a [u16]> {
        for byte in self.iter() {
            value = value.strip_prefix(&[u16::from(*byte)])?;
        }

        Some(value)
    }
}

impl<const N: usize> StripPattern for &[u8; N] {
    fn strip_prefix<'a>(&self, mut value: &'a [u16]) -> Option<&'a [u16]> {
        for byte in self.iter() {
            value = value.strip_prefix(&[u16::from(*byte)])?;
        }

        Some(value)
    }
}

impl<F: Fn(u8) -> bool> StripPattern for F {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]> {
        value
            .first()
            .filter(|o| self(**o as u8))
            .map(|_| &value[1..])
    }
}
