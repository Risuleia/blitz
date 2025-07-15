//! Helper traits to ease non-blocking handling.

use std::{
    io::{Error as IoError, ErrorKind as IoErrorKind},
    result::Result as StdResult,
};

use crate::error::Error;

/// Non-blocking IO handling.
pub trait NonBlockingError: Sized {
    /// Convert WouldBlock to None and don't touch other errors.
    fn into_non_blocking(self) -> Option<Self>;
}

impl NonBlockingError for IoError {
    fn into_non_blocking(self) -> Option<Self> {
        match self.kind() {
            IoErrorKind::WouldBlock => None,
            _ => Some(self),
        }
    }
}

impl NonBlockingError for Error {
    fn into_non_blocking(self) -> Option<Self> {
        match self {
            Error::Io(io_err) => io_err.into_non_blocking().map(Error::Io),
            other => Some(other),
        }
    }
}

/// Non-blocking IO wrapper.
///
/// This trait is implemented for `Result<T, E: NonBlockingError>`.
pub trait NonBlockingResult {
    /// Type of the converted result: `Result<Option<T>, E>`
    type Result;

    /// Perform the non-block conversion.
    fn no_block(self) -> Self::Result;
}

impl<T, E> NonBlockingResult for StdResult<T, E>
where
    E: NonBlockingError,
{
    type Result = StdResult<Option<T>, E>;

    fn no_block(self) -> Self::Result {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(err) => match err.into_non_blocking() {
                Some(real_err) => Err(real_err),
                None => Ok(None),
            },
        }
    }
}
