use std::{io, str::Utf8Error, string::FromUtf8Error};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Connection already closed")]
    AlreadyClosed,

    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),

    #[error("Protool Error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("UTF-8 Error: {0}")]
    Utf8(String),

    #[error("Write buffer is full")]
    WriteBufferFull,

    #[error("Capacity Error: {0}")]
    Capacity(#[from] CapacityError),

    #[error("Unexpected Message: {0}")]
    UnexpectedMessage(String),

    #[error("URL Error: {0}")]
    Url(#[from] UrlError),

    #[error("TLS Error: {0}")]
    Tls(#[from] TlsError)
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

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProtocolError {
    #[error("Invalid HTTP method (must be GET)")]
    InvalidHttpMethod,

    #[error("Unsupported HTTP version (must be at least HTTP/1.1)")]
    InvalidHttpVersion,

    #[error("Missing 'Connection: upgraded' header")]
    MissingConnectionUpgrade,

    #[error("Missing 'Upgrade: websocket' header")]
    MissingUpgradeHeader,

    #[error("Missing 'Sec-WebSocket-Version: 13' header")]
    MissingVersionHeader,

    #[error("Missing 'Sec-WebSocket-Key' header")]
    MissingKeyHeader,

    #[error("Mismatched 'Sec-WebSocket-Accept' header")]
    AcceptKeyMismatch,

    #[error("Encountered frame with non-zero reserved bits")]
    NonZeroReservedBits,

    #[error("Control frame must not be fragmented")]
    FragmentedControlFrame,

    #[error("Control frame payload too large")]
    ControlFrameTooBig,

    #[error("Received unmasked frame from client")]
    UnmaskedFrameFromClient,

    #[error("Received masked frame from server")]
    MaskedFrameFromServer,

    #[error("Received unknown control opcode: {0}")]
    UnknownControlOpCode(u8),

    #[error("Received unknown daa opcode: {0}")]
    UnknownDataOpCode(u8),

    #[error("Received continue frame without open fragmentation context")]
    UnexpectedContinue,

    #[error("Expected fragment of type {0:?} but received something else")]
    ExpectedFragment(FragmentType),

    #[error("Sent after close handshake started")]
    SendAfterClose,

    #[error("Received after close handshake completed")]
    ReceiveAfterClose,

    #[error("Invalid close frame payload")]
    InvalidCloseFrame,

    #[error("Connection closed without proper handshake")]
    ResetWithoutClosing,

    #[error("Server offered an unacceptable subprotocol")]
    InvalidSubprotocol,

    #[error("Client expected subprotocol '{expected}', but server selected '{actual}'")]
    SubprotocolMismatch {
        expected: String,
        actual: String
    },

    #[error("No common subprotocol could be negotiated")]
    NoMatchingSubprotocol
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FragmentType {
    Text,
    Binary
}

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CapacityError {
    #[error("Too many headers received")]
    TooManyHeaders,

    #[error("Payload too large: {size} > {max}")]
    MessageTooLarge {
        size: usize,
        max: usize
    }
}


#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum UrlError {
    #[error("Missing host name in URL")]
    MissingHost,

    #[error("Empty host name in URL")]
    EmptyHost,

    #[error("Unsupported URL scheme (expected 'ws://' or 'wss://')")]
    UnsupportedScheme,

    #[error("TLS feature not enabled but 'wss://' URL used")]
    TlsFeatureNotEnabled,

    #[error("No path / query segment in URL")]
    NoPathOrQuery,

    #[error("Unable to connect to host: {0}")]
    UnableToConnect(String)
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TlsError {
    #[cfg(feature = "native-tls")]
    #[error("Native TLS Error: {0}")]
    Native(#[from] native_tls::Error),

    #[cfg(feature = "rustls")]
    #[error("Rustls Error: {0}")]
    Rustls(#[from] rustls::Error),

    #[cfg(feature = "rustls")]
    #[error("Invalid DNS name for TLS")]
    InvalidDnsName
}