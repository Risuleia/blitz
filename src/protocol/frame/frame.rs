//! WebSocket Frame module

use std::{fmt::Display, io::{Cursor, ErrorKind, Read, Write}, mem, result::Result as StdResult, str::Utf8Error};

use bytes::{Bytes, BytesMut};

use super::{
    codec::{CloseCode, Control, Data, OpCode},
    mask::{apply_mask, generate},
};
use crate::{error::{Error, ProtocolError, Result}, protocol::frame::Utf8Bytes};

/// A struct representing the close command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseFrame {
    /// The reason as a code.
    pub code: CloseCode,
    /// The reason as text string.
    pub reason: Utf8Bytes

}

impl Display for CloseFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.reason, self.code)
    }
}

/// A struct representing a WebSocket frame header.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// Indicates is the frame is the last one of a possibly fragmented message
    pub fin: bool,
    /// Reserved for protocol extensions.
    pub rsv1: bool,
    /// Reserved for protocol extensions.
    pub rsv2: bool,
    /// Reserved for protocol extensions.
    pub rsv3: bool,
    /// WebSocket protocol opcode.
    pub opcode: OpCode,
    /// A frame mask (if any)
    pub mask: Option<[u8; 4]>,
}

impl Default for FrameHeader {
    fn default() -> Self {
        FrameHeader {
            fin: false,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: OpCode::Control(Control::Close),
            mask: None
        }
    }
}

impl FrameHeader {
    /// > The longest possible header is 14 bytes, which would represent a message sent from
    /// > the client to the server with a payload greater than 64KB.
    pub(crate) const MAX_HEADER_SIZE: usize = 14;

    /// Parse a header from an input stream.
    /// Returns `None` if insufficient data and does not consume anything in this case.
    /// Payload size is returned along with the header.
    pub fn parse(cursor: &mut Cursor<impl AsRef<[u8]>>) -> Result<Option<(Self, u64)>> {
        let init = cursor.position();

        match Self::parse_internal(cursor) {
            i @ Ok(None) => {
                cursor.set_position(init);
                i
            },
            other => other
        }
    }

    /// Get the size of the header formatted with given payload length.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self, length: u64) -> usize {
        2 +
        Length::for_len(length).additional() +
        (if self.mask.is_some() { 4 } else { 0 })
    }

    /// Format a header for given payload size.
    pub fn format(&self, length: u64, output: &mut impl Write) -> Result<()> {
        let code: u8 = self.opcode.into();

        let first_byte = {
            code | if self.fin { 0x80 } else { 0 }
                | if self.rsv1 { 0x40 } else { 0 }
                | if self.rsv2 { 0x20 } else { 0 }
                | if self.rsv3 { 0x10 } else { 0 }
        };

        let len = Length::for_len(length);

        let second_byte = len.len_byte() | if self.mask.is_some() { 0x80 } else { 0 };

        output.write_all(&[first_byte, second_byte])?;

        match len {
            Length::U8(_) => (),
            Length::U16 => {
                output.write_all(&(length as u16).to_be_bytes())?;
            },
            Length::U64 => {
                output.write_all(&length.to_be_bytes())?;
            }
        }

        if let Some(ref mask) = self.mask {
            output.write_all(mask)?;
        }

        Ok(())
    }

    /// Generate a random frame mask and store this in the header.
    ///
    /// Of course this does not change frame contents. It just generates a mask.
    pub(crate) fn set_random_mask(&mut self) {
        self.mask = Some(generate());
    }

    /// Internal parse engine.
    /// Returns `None` if insufficient data.
    /// Payload size is returned along with the header.
    fn parse_internal(cursor: &mut impl Read) -> Result<Option<(Self, u64)>> {
        let (a, b) = {
            let mut head = [0u8; 2];
            if cursor.read(&mut head)? != 2 {
                return Ok(None);
            }

            (head[0], head[1])
        };

        let fin = a & 0x80 != 0;
        let rsv1 = a & 0x40 != 0;
        let rsv2 = a & 0x20 != 0;
        let rsv3 = a & 0x10 != 0;

        let opcode = OpCode::from(a & 0x0F);

        let masked = b & 0x80 != 0;

        let len = {
            let len_byte = b & 0x7F;
            let particular_len = Length::for_byte(len_byte).additional();

            if particular_len > 0 {
                const SIZE: usize = mem::size_of::<u64>();
                assert!(particular_len < SIZE, "Length exceeded max size of unsigned 64-bit integer");

                let start = SIZE - particular_len;
                let mut buf = [0u8; SIZE];

                match cursor.read_exact(&mut buf[start..]) {
                    Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                    Err(e) => return Err(e.into()),
                    Ok(()) => u64::from_be_bytes(buf)
                }
            } else {
                u64::from(len_byte)
            }
        };

        let mask = if masked {
            let mut mask_bytes = [0u8; 4];
            if cursor.read(&mut mask_bytes)? != 4 {
                return Ok(None);
            } else {
                Some(mask_bytes)
            }
        } else {
            None
        };

        match opcode {
            OpCode::Control(Control::Reserved(_)) => {
                return Err(Error::Protocol(ProtocolError::UnknownControlOpCode(a & 0x0F)));
            },
            OpCode::Data(Data::Reserved(_)) => {
                return Err(Error::Protocol(ProtocolError::UnknownDataOpCode(a & 0x0F)));
            },
            _ => ()
        };

        let header = FrameHeader {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask
        };

        Ok(Some((header, len)))
    }
}

impl Frame {
    
}

/// The WebSocket Frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    header: FrameHeader,
    payload: Bytes
}

impl Frame {
    /// Get the length of the frame.
    /// This is the length of the header + the length of the payload.
    #[inline]
    pub fn len(&self) -> usize {
        let length = self.payload.len();
        self.header.len(length as u64) + length
    }

    /// Check if the frame is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the frame's header.
    #[inline]
    pub fn header(&self) -> &FrameHeader {
        &self.header
    }

    /// Get a mutable reference to the frame's header.
    #[inline]
    pub fn header_mut(&mut self) -> &mut FrameHeader {
        &mut self.header
    }

    /// Get a reference to the frame's payload.
    #[inline]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Test whether the frame is masked.
    #[inline]
    pub(crate) fn is_masked(&self) -> bool {
        self.header.mask.is_some()
    }

    /// Generate a random mask for the frame.
    ///
    /// This just generates a mask, payload is not changed. The actual masking is performed
    /// either on `format()` or on `apply_mask()` call.
    #[inline]
    pub(crate) fn set_random_mask(&mut self) {
        self.header.set_random_mask();
    }

    /// Consume the frame into its payload as string.
    #[inline]
    pub fn into_text(self) -> StdResult<Utf8Bytes, Utf8Error> {
        self.payload.try_into()
    }

    /// Consume the frame into its payload.
    #[inline]
    pub fn into_payload(self) -> Bytes {
        self.payload
    }

    /// Get frame payload as `&str`.
    #[inline]
    pub fn to_text(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(&self.payload)
    }

    /// Consume the frame into a closing frame.
    #[inline]
    pub(crate) fn into_close(self) -> Result<Option<CloseFrame>> {
        match self.payload.len() {
            0 => Ok(None),
            1 => Err(Error::Protocol(ProtocolError::InvalidCloseFrame)),
            _ => {
                let code = u16::from_be_bytes([self.payload[0], self.payload[1]]).into();
                let reason = Utf8Bytes::try_from(self.payload.slice(2..))?;

                Ok(Some(CloseFrame { code, reason }))
            }
        }
    }

    /// Create a new data frame.
    #[inline]
    pub fn new_data(data: impl Into<Bytes>, opcode: OpCode, fin: bool) -> Frame {
        debug_assert!(matches!(opcode, OpCode::Data(_)), "Invalid opcode for data frame");

        Frame {
            header: FrameHeader { fin, opcode, ..Default::default() },
            payload: data.into()
        }
    }

    /// Create a new Ping control frame.
    #[inline]
    pub fn new_ping(data: impl Into<Bytes>) -> Frame {
        Frame {
            header: FrameHeader { opcode: OpCode::Control(Control::Ping), ..<_>::default() },
            payload: data.into()
        }
    }

    /// Create a new Pong control frame.
    #[inline]
    pub fn new_pong(data: impl Into<Bytes>) -> Frame {
        Frame {
            header: FrameHeader { opcode: OpCode::Control(Control::Pong), ..<_>::default() },
            payload: data.into()
        }
    }

    /// Create a new Close control frame.
    #[inline]
    pub fn new_close(msg: Option<CloseFrame>) -> Frame {
        let payload = if let Some(CloseFrame { code, reason }) = msg {
            let mut p = BytesMut::with_capacity(reason.len() + 2);
            p.extend(u16::from(code).to_be_bytes());
            p.extend_from_slice(reason.as_bytes());
            p
        } else {
            <_>::default()
        };

        Frame {
            header: <_>::default(),
            payload: payload.into()
        }
    }

    /// Initializes a new frame
    pub fn new(header: FrameHeader, payload: Bytes) -> Self {
        Frame { header, payload }
    }

    /// Write a frame out to a buffer
    pub fn format_to_buf(mut self, output: &mut impl Write) -> Result<()> {
        self.header.format(self.payload.len() as u64, output)?;

        if let Some(mask) = self.header.mask.take() {
            let mut data = Vec::from(mem::take(&mut self.payload));
            apply_mask(&mut data, mask);

            output.write_all(&data)?;
        } else {
            output.write_all(&self.payload)?;
        }

        Ok(())
    }

    pub(crate) fn into_buf(mut self, buf: &mut Vec<u8>) -> Result<()> {
        self.header.format(self.payload.len() as u64, buf)?;

        let len = buf.len();
        buf.extend_from_slice(&self.payload);

        if let Some(mask) = self.header.mask.take() {
            apply_mask(&mut buf[len..], mask);
        }

        Ok(())
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        write!(
            f,
            "/
            [FRAME]
            final: {},
            reserved: {} {} {},
            opcode: {},
            length: {},
            payload-length: {},
            payload: 0x{}
            ",
            self.header.fin,
            self.header.rsv1,
            self.header.rsv2,
            self.header.rsv3,
            self.header.opcode,
            self.len(),
            self.payload.len(),
            self.payload.iter().fold(String::new(), |mut out, byte| {
                _ = write!(out, "{byte:02x}");
                out
            })
        )
    }
}

enum Length {
    U8(u8),
    U16,
    U64
}

impl Length {
    #[inline]
    fn for_len(len: u64) -> Self {
        if len < 126 {
            Length::U8(len as u8)
        } else if len < 65536 {
            Length::U16
        } else {
            Length::U64
        }
    }

    #[inline]
    fn additional(&self) -> usize {
        match *self {
            Self::U8(_) => 0,
            Self::U16 => 2,
            Self::U64 => 8
        }
    }

    #[inline]
    fn len_byte(&self) -> u8 {
        match *self {
            Self::U8(b) => b,
            Self::U16 => 126,
            Self::U64 => 127
        }
    }

    #[inline]
    fn for_byte(byte: u8) -> Self {
        match byte & 0x7F {
            126 => Length::U16,
            127 => Length::U64,
            b => Length::U8(b)
        }
    }
}