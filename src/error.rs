//! Error handling

use std::{io, str::Utf8Error, string::FromUtf8Error};

use http::{HeaderName, Response};
use thiserror::Error;


/// Generic result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Possible WebSocket errors.
#[derive(Debug, Error)]
pub enum Error {
    /// WebSocket connection closed normally. This informs you of the close.
    /// It's not an error as such and nothing wrong happened.
    ///
    /// This is returned as soon as the close handshake is finished (we have both sent and
    /// received a close frame) on the server end and as soon as the server has closed the
    /// underlying connection if this endpoint is a client.
    ///
    /// Thus when you receive this, it is safe to drop the underlying connection.
    ///
    /// Receiving this error means that the WebSocket object is not usable anymore and the
    /// only meaningful action with it is dropping it.
    #[error("Connection closed")]
    ConnectionClosed,

    /// Trying to work with already closed connection.
    ///
    /// Trying to read or write after receiving `ConnectionClosed` causes this.
    ///
    /// As opposed to `ConnectionClosed`, this indicates your code tries to operate on the
    /// connection when it really shouldn't anymore, so this really indicates a programmer
    /// error on your part.
    #[error("Connection already closed")]
    AlreadyClosed,

    /// Input-output error. Apart from WouldBlock, these are generally errors with the
    /// underlying connection and you should probably consider them fatal.
    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),

    /// Protocol violation.
    #[error("Protool Error: {0}")]
    Protocol(#[from] ProtocolError),

    /// UTF-8 coding error.
    #[error("UTF-8 Error: {0}")]
    Utf8(String),

    /// Message write buffer is full.
    #[error("Write buffer is full")]
    WriteBufferFull,

    /// - When reading: buffer capacity exhausted.
    /// - When writing: your message is bigger than the configured max message size
    ///   (64MB by default).
    #[error("Capacity Error: {0}")]
    Capacity(#[from] CapacityError),

    /// HTTP error.
    #[error("HTTP Error: {}", .0.status())]
    #[cfg(feature = "handshake")]
    Http(Response<Option<Vec<u8>>>),

    /// HTTP format error.
    #[error("HTTP format error: {0}")]
    #[cfg(feature = "handshake")]
    HttpFormat(#[from] http::Error),

    /// Invalid URL.
    #[error("URL Error: {0}")]
    Url(#[from] UrlError),

    /// TLS error.
    ///
    /// Note that this error variant is enabled unconditionally even if no TLS feature is enabled,
    /// to provide a feature-agnostic API surface.
    #[error("TLS Error: {0}")]
    Tls(#[from] TlsError),

    /// Attack attempt detected.
    #[error("Detected attempted attack")]
    AttackAttempt
}

impl From<Utf8Error> for Error {
    fn from(value: Utf8Error) -> Self {
        Error::Utf8(value.to_string())
    }
}
impl From<FromUtf8Error> for Error {
    fn from(value: FromUtf8Error) -> Self {
        Error::Utf8(value.to_string())
    }
}

#[cfg(feature = "handshake")]
impl From<http::header::InvalidHeaderName> for Error {
    fn from(value: http::header::InvalidHeaderName) -> Self {
        Error::HttpFormat(value.into())
    }
}

#[cfg(feature = "handshake")]
impl From<http::header::InvalidHeaderValue> for Error {
    fn from(value: http::header::InvalidHeaderValue) -> Self {
        Error::HttpFormat(value.into())
    }
}

#[cfg(feature = "handshake")]
impl From<http::header::ToStrError> for Error {
    fn from(value: http::header::ToStrError) -> Self {
        Error::Utf8(value.to_string())
    }
}

#[cfg(feature = "handshake")]
impl From<http::uri::InvalidUri> for Error {
    fn from(value: http::uri::InvalidUri) -> Self {
        Error::HttpFormat(value.into())
    }
}

#[cfg(feature = "handshake")]
impl From<http::status::InvalidStatusCode> for Error {
    fn from(value: http::status::InvalidStatusCode) -> Self {
        Error::HttpFormat(value.into())
    }
}

#[cfg(feature = "handshake")]
impl From<httparse::Error> for Error {
    fn from(value: httparse::Error) -> Self {
        match value {
            httparse::Error::TooManyHeaders => Error::Capacity(CapacityError::TooManyHeaders),
            e => Error::Protocol(ProtocolError::HttparseError(e))
        }
    }
}

/// Indicates the specific type/cause of a protocol error.
#[allow(missing_copy_implementations)]
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProtocolError {
    /// Use of the wrong HTTP method (the WebSocket protocol requires the GET method be used).
    #[error("Invalid HTTP method (must be GET)")]
    InvalidHttpMethod,

    /// Wrong HTTP version used (the WebSocket protocol requires version 1.1 or higher).
    #[error("Unsupported HTTP version (must be at least HTTP/1.1)")]
    InvalidHttpVersion,

    /// Invalid header is passed. Or the header is missing in the request. Or not present at all. Check the request that you pass.
    #[error("Missing, duplicated or incorrect header {0}")]
    #[cfg(feature = "handshake")]
    InvalidHeader(HeaderName),

    /// Missing `Connection: upgrade` HTTP header.
    #[error("Missing 'Connection: upgrade' header")]
    MissingConnectionUpgradeHeader,
    
    /// Missing `Upgrade: websocket` HTTP header.
    #[error("Missing 'Upgrade: websocket' header")]
    MissingUpgradeHeader,
    
    /// Missing `Sec-WebSocket-Version: 13` HTTP header.
    #[error("Missing 'Sec-WebSocket-Version: 13' header")]
    MissingVersionHeader,
    
    /// Missing `Sec-WebSocket-Key` HTTP header.
    #[error("Missing 'Sec-WebSocket-Key' header")]
    MissingKeyHeader,
    
    /// The `Sec-WebSocket-Accept` header is either not present or does not specify the correct key value.
    #[error("Mismatched 'Sec-WebSocket-Accept' header")]
    AcceptKeyMismatch,
    
    /// The `Sec-WebSocket-Protocol` header was invalid
    #[error("SubProtocol error: {0}")]
    SecWebSocketSubProtocolError(SubProtocolError),

    /// No more data while still performing handshake.
    #[error("Handshake incomplete")]
    IncompleteHandshake,

    /// Wrapper around a [`httparse::Error`] value.
    #[error("httparse error: {0}")]
    #[cfg(feature = "handshake")]
    HttparseError(#[from] httparse::Error),

    /// Reserved bits in frame header are non-zero.
    #[error("Encountered frame with non-zero reserved bits")]
    NonZeroReservedBits,

    /// Control frames must not be fragmented.
    #[error("Control frame must not be fragmented")]
    FragmentedControlFrame,

    /// Control frames must have a payload of 125 bytes or less.
    #[error("Control frame payload too large")]
    ControlFrameTooBig,

    /// The server must close the connection when an unmasked frame is received.
    #[error("Received unmasked frame from client")]
    UnmaskedFrameFromClient,

    /// The client must close the connection when a masked frame is received.
    #[error("Received masked frame from server")]
    MaskedFrameFromServer,

    /// Encountered an invalid controlopcode.
    #[error("Received unknown control opcode: {0}")]
    UnknownControlOpCode(u8),

    /// Encountered an invalid data opcode.
    #[error("Received unknown data opcode: {0}")]
    UnknownDataOpCode(u8),

    /// Received a continue frame despite there being nothing to continue.
    #[error("Received continue frame without open fragmentation context")]
    UnexpectedContinue,

    /// Received data while waiting for more fragments.
    #[error("Expected fragment of type {0:?} but received something else")]
    ExpectedFragment(FragmentType),

    /// Not allowed to send after having sent a closing frame.
    #[error("Sent after close handshake started")]
    SendAfterClose,

    /// Remote sent data after sending a closing frame.
    #[error("Received after close handshake completed")]
    ReceiveAfterClose,

    /// The payload for the closing frame is invalid.
    #[error("Invalid close frame payload")]
    InvalidCloseFrame,

    /// Connection closed without performing the closing handshake.
    #[error("Connection closed without proper handshake")]
    ResetWithoutClosing,

    /// Garbage data encountered after client request.
    #[error("Junk after client request")]
    JunkAfterRequest,

    /// Custom responses must be unsuccessful.
     #[error("Custom response must not be successful")]
    CustomResponseSuccessful,
}

/// Indicates the specific type/cause of a subprotocol header error.
#[derive(Error, Clone, PartialEq, Eq, Debug, Copy)]
pub enum SubProtocolError {
    /// The server sent a subprotocol to a client handshake request but none was requested
    #[error("Server sent a subprotocol but none was requested")]
    ServerSentSubProtocolNoneRequested,

    /// The server sent an invalid subprotocol to a client handhshake request
    #[error("Server sent an invalid subprotocol")]
    InvalidSubProtocol,

    /// The server sent no subprotocol to a client handshake request that requested one or more
    /// subprotocols
    #[error("Server sent no subprotocol")]
    NoSubProtocol,
}

/// Fragment type
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FragmentType {
    /// Text fragment
    Text,
    /// Binary fragment
    Binary
}

/// Indicates the specific type/cause of a capacity error.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum CapacityError {
    /// Too many headers provided (see [`httparse::Error::TooManyHeaders`]).
    #[error("Too many headers received")]
    TooManyHeaders,

    /// Received header is too long.
    /// Message is bigger than the maximum allowed size.
    #[error("Payload too large: {size} > {max}")]
    MessageTooLarge {
        /// The size of the message.
        size: usize,
        /// The maximum allowed message size.
        max: usize
    }
}


/// Indicates the specific type/cause of URL error.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum UrlError {
    /// The URL does not include a host name.
    #[error("Missing host name in URL")]
    MissingHost,

    /// The URL host name, though included, is empty.
    #[error("Empty host name in URL")]
    EmptyHost,

    /// Unsupported URL scheme used (only `ws://` or `wss://` may be used).
    #[error("Unsupported URL scheme (expected 'ws://' or 'wss://')")]
    UnsupportedScheme,

    /// TLS is used despite not being compiled with the TLS feature enabled.
    #[error("TLS feature not enabled but 'wss://' URL used")]
    TlsFeatureNotEnabled,

    /// The URL does not include a path/query.
    #[error("No path / query segment in URL")]
    NoPathOrQuery,

    /// Failed to connect with this URL.
    #[error("Unable to connect to host: {0}")]
    UnableToConnect(String)
}

/// TLS errors.
///
/// Note that even if you enable only the rustls-based TLS support, the error at runtime could still
/// be `Native`, as another crate in the dependency graph may enable native TLS support.
#[allow(missing_copy_implementations)]
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TlsError {
    /// Native TLS error.
    #[cfg(feature = "native-tls")]
    #[error("Native TLS Error: {0}")]
    Native(#[from] native_tls_crate::Error),

    /// Rustls error.
    #[cfg(feature = "rustls")]
    #[error("Rustls Error: {0}")]
    Rustls(#[from] rustls::Error),

    /// DNS name resolution error.
    #[cfg(feature = "rustls")]
    #[error("Invalid DNS name for TLS")]
    InvalidDnsName
}