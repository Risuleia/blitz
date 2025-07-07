use core::str;
use std::{borrow::Borrow, fmt::Display, hash::Hash, ops::Deref};

use bytes::{Bytes, BytesMut};

/// Utf8 payload.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Utf8Bytes(Bytes);

impl Utf8Bytes {
    /// Creates from a static str.
    #[inline]
    pub const fn from_static(str: &'static str) -> Self {
        Self(Bytes::from_static(str.as_bytes()))
    }

    /// Returns as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(&self.0)
        }
    }

    /// Creates from a [`Bytes`] object without checking the encoding.
    ///
    /// # Safety
    ///
    /// The bytes passed in must be valid UTF-8.
    pub unsafe fn from_bytes_unchecked(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

impl Deref for Utf8Bytes {
    type Target = str;

    /// ```
    /// /// Example fn that takes a str slice
    /// fn a(s: &str) {}
    ///
    /// let data = tungstenite::Utf8Bytes::from_static("foo123");
    ///
    /// // auto-deref as arg
    /// a(&data);
    ///
    /// // deref to str methods
    /// assert_eq!(data.len(), 6);
    /// ```
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<[u8]> for Utf8Bytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<str> for Utf8Bytes {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<Bytes> for Utf8Bytes {
    #[inline]
    fn as_ref(&self) -> &Bytes {
        &self.0
    }
}

impl Borrow<str> for Utf8Bytes {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Hash for Utf8Bytes {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for Utf8Bytes {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Utf8Bytes {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<T> PartialEq<T> for Utf8Bytes
where
    for<'a> &'a str: PartialEq<T>
{
    /// ```
    /// let payload = tungstenite::Utf8Bytes::from_static("foo123");
    /// assert_eq!(payload, "foo123");
    /// assert_eq!(payload, "foo123".to_string());
    /// assert_eq!(payload, &"foo123".to_string());
    /// assert_eq!(payload, std::borrow::Cow::from("foo123"));
    /// ```
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_str() == *other
    }
}

impl Display for Utf8Bytes {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<Bytes> for Utf8Bytes {
    type Error = std::str::Utf8Error;

    #[inline]
    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        std::str::from_utf8(&value)?;
        Ok(Self(value))
    }
}

impl TryFrom<BytesMut> for Utf8Bytes {
    type Error = std::str::Utf8Error;

    #[inline]
    fn try_from(value: BytesMut) -> Result<Self, Self::Error> {
        value.freeze().try_into()
    }
}

impl TryFrom<Vec<u8>> for Utf8Bytes {
    type Error = std::str::Utf8Error;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Bytes::from(value).try_into()
    }
}

impl From<String> for Utf8Bytes {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Utf8Bytes {
    #[inline]
    fn from(value: &str) -> Self {
        Self(Bytes::copy_from_slice(value.as_bytes()))
    }
}

impl From<&String> for Utf8Bytes {
    #[inline]
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}

impl From<Utf8Bytes> for Bytes {
    #[inline]
    fn from(Utf8Bytes(value): Utf8Bytes) -> Self {
        value
    }
}