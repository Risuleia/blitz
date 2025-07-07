//! Frame module

pub mod codec;
pub mod core;

#[allow(clippy::module_inception)]
mod frame;
mod mask;
mod utf;

pub use self::{
    frame::{Frame, FrameHeader, CloseFrame},
    utf::Utf8Bytes
};