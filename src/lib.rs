#![deny(
    missing_docs,
    missing_copy_implementations,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_must_use,
    unused_mut,
    unused_imports,
    unused_import_braces
)]
//! Blitz: Lightweight WebSocket + HTTP server components
#![allow(clippy::result_large_err)]

#[cfg(feature = "handshake")]
pub use http;

#[cfg(feature = "handshake")]
pub mod client;
#[cfg(feature = "handshake")]
pub mod handshake;
#[cfg(feature = "handshake")]
mod server;

#[cfg(all(any(feature = "native-tls", feature = "rustls"), feature = "handshake"))]
mod tls;

pub mod buffer;
pub mod error;
pub mod protocol;
pub mod stream;
pub mod util;

/// Constant for maximum message payload length
pub const MAX_ALLOWED_LEN: usize = 16 * 1024 * 1024;
/// Constant for maximum control frame payload size
pub const MAX_CONTROL_FRAME_PAYLOAD: usize = 125;
/// Constant for maximum continuation frames
pub const MAX_CONTINUATION_FRAMES: usize = 1024;

const READ_BUFFER_SIZE: usize = 4096;
type ReadBuffer = buffer::ReadBuffer<READ_BUFFER_SIZE>;

pub use bytes::Bytes;

#[cfg(feature = "handshake")]
pub use crate::{
    client::{client, connect, ClientRequestBuilder},
    server::{accept, accept_with_config, accept_header, accept_header_with_config},
    handshake::{client::ClientHandshake, server::ServerHandshake, HandshakeError}
};

#[cfg(all(any(feature = "native-tls", feature = "__rustls-tls"), feature = "handshake"))]
pub use tls::{client_tls, client_tls_with_config, Connector};