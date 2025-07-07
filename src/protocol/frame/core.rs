//! Utilities to work with raw WebSocket frames.

use std::io::{self, Cursor, Read, Write};

use bytes::{Buf, BytesMut};

use crate::{error::{CapacityError, Error, ProtocolError, Result}, protocol::frame::{frame::{Frame, FrameHeader}, mask::apply_mask}};

const READ_BUFFER_LENGTH: usize = 128 * 1024;

/// Read buffer size used for `FrameSocket`.
#[derive(Debug)]
pub struct FrameSocket<T> {
    /// The underlying network stream.
    stream: T,
    /// Codec for reading/writing frames.
    codec: FrameCodec
}

impl<T: Read + Write> FrameSocket<T> {
    /// Create a new frame socket.
    pub fn new(stream: T) -> Self {
        FrameSocket { stream, codec: FrameCodec::new(READ_BUFFER_LENGTH) }
    }

    /// Create a new frame socket from partially read data.
    pub fn from_partially_read(stream: T, part: Vec<u8>) -> Self {
        FrameSocket { stream, codec: FrameCodec::from_partially_read(part, READ_BUFFER_LENGTH) }
    }

    /// Extract a stream from the socket.
    pub fn into_inner(self) -> (T, BytesMut) {
        (self.stream, self.codec.in_buffer)
    }

    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &T {
        &self.stream
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Read a frame from stream.
    pub fn read(&mut self, max: Option<usize>) -> Result<Option<Frame>> {
        self.codec.read(&mut self.stream, max, false, true)
    }

    /// Writes and immediately flushes a frame.
    /// Equivalent to calling [`write`](Self::write) then [`flush`](Self::flush).
    pub fn send(&mut self, frame: Frame) -> Result<()> {
        self.write(frame)?;
        self.flush()
    }

    /// Write a frame to stream.
    ///
    /// A subsequent call should be made to [`flush`](Self::flush) to flush writes.
    ///
    /// This function guarantees that the frame is queued unless [`Error::WriteBufferFull`]
    /// is returned.
    /// In order to handle WouldBlock or Incomplete, call [`flush`](Self::flush) afterwards.
    pub fn write(&mut self, frame: Frame) -> Result<()> {
        self.codec.write(&mut self.stream, frame)
    }

    /// Flush writes.
    pub fn flush(&mut self) -> Result<()> {
        self.codec.write_out(&mut self.stream)?;
        Ok(self.stream.flush()?)
    }
}

/// A codec for WebSocket frames.
#[derive(Debug)]
pub(crate) struct FrameCodec {
    /// Buffer to read data from the stream.
    in_buffer: BytesMut,
    in_buffer_max_read: usize,
    /// Buffer to send packets to the network.
    out_buffer: Vec<u8>,
    /// Capacity limit for `out_buffer`.
    max_out_buffer_len: usize,
    /// Buffer target length to reach before writing to the stream
    /// on calls to `buffer_frame`.
    ///
    /// Setting this to non-zero will buffer small writes from hitting
    /// the stream.
    out_buffer_write_len: usize,
    /// Header and remaining size of the incoming packet being processed.
    header: Option<(FrameHeader, u64)>
}

impl FrameCodec {
    /// Create a new frame codec.
    pub(crate) fn new(len: usize) -> Self {
        Self {
            in_buffer: BytesMut::with_capacity(len),
            in_buffer_max_read: len.max(FrameHeader::MAX_HEADER_SIZE),
            out_buffer: <_>::default(),
            max_out_buffer_len: usize::MAX,
            out_buffer_write_len: 0,
            header: None
        }
    }

    /// Create a new frame codec from partially read data.
    pub(crate) fn from_partially_read(part: Vec<u8>, min_in_buffer_len: usize) -> Self {
        let mut buf = BytesMut::from_iter(part);
        buf.reserve(min_in_buffer_len.saturating_sub(buf.len()));

        Self {
            in_buffer: buf,
            in_buffer_max_read: min_in_buffer_len.max(FrameHeader::MAX_HEADER_SIZE),
            out_buffer: <_>::default(),
            max_out_buffer_len: usize::MAX,
            out_buffer_write_len: 0,
            header: None
        }
    }

    /// Sets a maximum size for the out buffer.
    pub(crate) fn max_out_buffer_len(&mut self, size: usize) {
        self.max_out_buffer_len = size
    }

    /// Sets [`Self::buffer_frame`] buffer target length to reach before
    /// writing to the stream.
    pub(crate) fn out_buffer_write_len(&mut self, size: usize) {
        self.out_buffer_write_len = size
    }

    /// Read a frame from the provided stream.
    pub(crate) fn read<S: Read>(
        &mut self,
        stream: &mut S,
        max: Option<usize>,
        unmask: bool,
        accept_unmasked: bool
    ) -> Result<Option<Frame>> {
        let max = max.unwrap_or(usize::MAX);

        let mut payload = loop {
            if self.header.is_none() {
                let mut cursor = Cursor::new(&mut self.in_buffer);
                self.header = FrameHeader::parse(&mut cursor)?;
                let n = cursor.position();
                Buf::advance(&mut self.in_buffer, n as _);

                if let Some((_, len)) = &self.header {
                    let len = *len as usize;

                    if len > max {
                        return Err(Error::Capacity(CapacityError::MessageTooLarge { size: len, max }));
                    }

                    self.in_buffer.reserve(len);
                } else {
                    self.in_buffer.reserve(FrameHeader::MAX_HEADER_SIZE);
                }
            }

            if let Some((_, len)) = &self.header {
                let len = *len as usize;
                if len <= self.in_buffer.len() {
                    break self.in_buffer.split_to(len);
                }
            }

            if self.read_in(stream)? == 0 {
                return Ok(None);
            }
        };

        let (mut header, length) = self.header.take().expect("Bug: no frame header");
        debug_assert_eq!(payload.len() as u64, length);

        if unmask {
            if let Some(mask) = header.mask.take() {
                apply_mask(&mut payload, mask);
            } else if !accept_unmasked {
                return Err(Error::Protocol(ProtocolError::UnmaskedFrameFromClient))
            }
        }

        let frame = Frame::new(header, payload.freeze());
        Ok(Some(frame))
    }

    /// Read into available `in_buffer` capacity.
    fn read_in<S: Read>(&mut self, stream: &mut S) -> io::Result<usize> {
        let len = self.in_buffer.len();
        debug_assert!(self.in_buffer.capacity() > len);

        self.in_buffer.resize(self.in_buffer.capacity().min(len + self.in_buffer_max_read), 0);

        let size = stream.read(&mut self.in_buffer[len..]);
        self.in_buffer.truncate(len + size.as_ref().copied().unwrap_or(0));

        size
    }

    /// Writes a frame into the `out_buffer`.
    /// If the out buffer size is over the `out_buffer_write_len` will also write
    /// the out buffer into the provided `stream`.
    ///
    /// To ensure buffered frames are written call [`Self::write_out_buffer`].
    ///
    /// May write to the stream, will **not** flush.
    pub(crate) fn write<S: Write>(&mut self, stream: &mut S, frame: Frame) -> Result<()>
    {
        if frame.len() + self.out_buffer.len() > self.max_out_buffer_len {
            return Err(Error::WriteBufferFull);
        }

        self.out_buffer.reserve(frame.len());
        frame.into_buf(&mut self.out_buffer).expect("Bug: can't write to vector");

        if self.out_buffer.len() > self.out_buffer_write_len {
            self.write_out(stream)
        } else {
            Ok(())
        }
    }

    /// Writes the out_buffer to the provided stream.
    ///
    /// Does **not** flush.
    pub(crate) fn write_out<S: Write>(&mut self, stream: &mut S) -> Result<()> {
        while !self.out_buffer.is_empty() {
            let len = stream.write(&self.out_buffer)?;

            if len == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "Connection reset while sending"
                ).into());
            }

            self.out_buffer.drain(0..len);
        }

        Ok(())
    }
}