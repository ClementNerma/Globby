use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
};

#[derive(Debug)]
pub struct OpaqueOsStr<'a> {
    inner: Cow<'a, [u16]>,
}

impl<'a> OpaqueOsStr<'a> {
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

    pub fn to_static(&self) -> OpaqueOsStr<'static> {
        OpaqueOsStr {
            inner: Cow::Owned(self.inner.clone().into_owned()),
        }
    }

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

    pub fn borrow<'b: 'a>(&'b self) -> OpaqueOsStr<'b> {
        OpaqueOsStr {
            inner: Cow::Borrowed(self.inner.as_ref()),
        }
    }

    pub fn strip_prefix<'b: 'a>(&'b self, pattern: impl BytesPattern) -> Option<OpaqueOsStr<'b>> {
        pattern.strip_prefix(self.inner.as_ref()).map(|inner| Self {
            inner: Cow::Borrowed(inner),
        })
    }

    pub fn strip_ascii_char<'b: 'a>(&'b self) -> Option<(char, OpaqueOsStr<'b>)> {
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

    pub fn try_strip_prefixes<'b: 'a, const N: usize>(
        &'b self,
        patterns: [impl BytesPattern; N],
    ) -> OpaqueOsStr<'b> {
        for pattern in patterns {
            if let Some(stripped) = pattern.strip_prefix(self.inner.as_ref()) {
                return OpaqueOsStr {
                    inner: Cow::Borrowed(stripped),
                };
            }
        }

        self.borrow()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn split(&self, predicate: impl Fn(u8) -> bool) -> impl Iterator<Item = OpaqueOsStr> {
        self.inner
            .split(move |b| predicate(*b as u8))
            .map(|opaques| OpaqueOsStr {
                inner: Cow::Borrowed(opaques),
            })
    }
}

pub trait BytesPattern {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]>;
}

impl BytesPattern for u8 {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]> {
        value.strip_prefix(&[u16::from(*self)])
    }
}

impl BytesPattern for &[u8] {
    fn strip_prefix<'a>(&self, mut value: &'a [u16]) -> Option<&'a [u16]> {
        for byte in self.iter() {
            value = value.strip_prefix(&[u16::from(*byte)])?;
        }

        Some(value)
    }
}

impl<const N: usize> BytesPattern for &[u8; N] {
    fn strip_prefix<'a>(&self, mut value: &'a [u16]) -> Option<&'a [u16]> {
        for byte in self.iter() {
            value = value.strip_prefix(&[u16::from(*byte)])?;
        }

        Some(value)
    }
}

impl<F: Fn(u8) -> bool> BytesPattern for F {
    fn strip_prefix<'a>(&self, value: &'a [u16]) -> Option<&'a [u16]> {
        value
            .first()
            .filter(|o| self(**o as u8))
            .map(|_| &value[1..])
    }
}
