//! HTTP Request and Respose header handlers

use http::{HeaderMap, HeaderName, HeaderValue};
use httparse::{parse_headers, Header, EMPTY_HEADER};

use crate::{error::Result, handshake::machine::TryParse};

/// Limit for the number of header lines
pub const MAX_HEADERS: usize = 124;

/// Trait to convert raw objects into HTTP parse-able objects
pub(crate) trait FromHttparse<T>: Sized {
    /// Convert raw object into HTTP headers
    fn from_httparse(raw: T) -> Result<Self>;
}

impl<'b: 'h, 'h> FromHttparse<&'b [Header<'h>]> for HeaderMap {
    fn from_httparse(raw: &'b [Header<'h>]) -> Result<Self> {
        let mut headers = HeaderMap::new();

        for h in raw {
            headers.append(
                HeaderName::from_bytes(h.name.as_bytes())?,
                HeaderValue::from_bytes(h.value)?,
            );
        }

        Ok(headers)
    }
}

impl TryParse for HeaderMap {
    fn try_parse(data: &[u8]) -> crate::error::Result<Option<(usize, Self)>> {
        let mut hbuffer = [EMPTY_HEADER; MAX_HEADERS];

        Ok(match parse_headers(data, &mut hbuffer)? {
            httparse::Status::Partial => None,
            httparse::Status::Complete((size, hdr)) => Some((size, HeaderMap::from_httparse(hdr)?)),
        })
    }
}
