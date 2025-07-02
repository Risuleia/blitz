// #![deny(missing_docs)]

//! Blitz: Lightweight WebSocket + HTTP server components

pub mod error;
pub mod stream;
pub mod util;

pub mod handshake;
pub mod protocol;

pub const MAX_ALLOWED_LEN: usize = 16 * 1024 * 1024;
pub const MAX_CONTROL_FRAME_PAYLOAD: usize = 125;
pub const MAX_CONTINUATION_FRAMES: usize = 1024;