// #![deny(
//     missing_docs,
//     missing_copy_implementations,
//     missing_debug_implementations,
//     trivial_casts,
//     trivial_numeric_casts,
//     unstable_features,
//     unused_must_use,
//     unused_mut,
//     unused_imports,
//     unused_import_braces
// )]
//! Blitz: Lightweight WebSocket + HTTP server components
#![allow(clippy::result_large_err)]

pub mod error;
pub mod stream;
pub mod util;

#[cfg(feature = "handshake")]
pub mod handshake;
pub mod protocol;

mod buffer;

/// Constant for maximum message payload length
pub const MAX_ALLOWED_LEN: usize = 16 * 1024 * 1024;
/// Constant for maximum control frame payload size
pub const MAX_CONTROL_FRAME_PAYLOAD: usize = 125;
/// Constant for maximum continuation frames
pub const MAX_CONTINUATION_FRAMES: usize = 1024;

const READ_BUFFER_SIZE: usize = 4096;
type ReadBuffer = buffer::ReadBuffer<READ_BUFFER_SIZE>;