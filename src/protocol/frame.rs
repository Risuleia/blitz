//! WebSocket Frame module

use std::io::{self, Read, Write};

use crate::{protocol::compression, MAX_ALLOWED_LEN};

/// WebSocket message opcode as in RFC 6455.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    /// A continuation frame
    Continuation = 0x0,
    /// A text frame
    Text = 0x1,
    /// A binary frame
    Binary = 0x2,
    /// A close frame
    Close = 0x8,
    /// A ping frame
    Ping = 0x9,
    /// A pong frame
    Pong = 0xA,
    /// Edge cases
    Bad = 0xFF
}

impl From<u8> for OpCode {
    fn from(byte: u8) -> Self {
        match byte {
            0x0 => OpCode::Continuation,
            0x1 => OpCode::Text,
            0x2 => OpCode::Binary,
            0x8 => OpCode::Close,
            0x9 => OpCode::Ping,
            0xA => OpCode::Pong,
            _ => OpCode::Bad
        }
    }
}

/// The WebSocket Frame
#[derive(Debug)]
pub struct Frame {
    /// Indicates is the frame is the last one of a possibly fragmented message
    pub fin: bool,
    /// WebSocket protocol opcode
    pub opcode: OpCode,
    /// Whether the frame is masked or not
    pub masked: bool,
    /// A frame mask (if any)
    pub masking_key: Option<[u8; 4]>,
    /// Rserved for protocol extensions
    pub rsv1: bool,
    /// The frame data
    pub payload: Vec<u8>
}

impl Frame {
    fn apply_mask(payload: &mut [u8], key: [u8; 4]) {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }

    /// Initializes a new frame
    pub fn new(
        opcode: OpCode,
        fin: bool,
        rsv1: bool,
        masked: bool,
        payload: Vec<u8>
    ) -> Self {
        let masking_key = if masked {
            Some(rand::random::<[u8; 4]>())
        } else {
            None
        };

        Frame { fin, rsv1, opcode, payload, masked, masking_key }
    }

    /// Reads a frame
    pub fn read<R: Read>(reader: &mut R, compression: bool) -> io::Result<Self> {
        let mut header = [0u8; 2];
        reader.read_exact(&mut header)?;

        let fin = (header[0] & 0x80) != 0;
        let rsv1 = (header[0] & 0x40) != 0;
        let opcode = OpCode::from(header[0] & 0x0F);

        if let OpCode::Bad = opcode {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid OpCode"));
        }

        let masked = (header[1] & 0x80) != 0;
        let mut payload_len = (header[1] & 0x7F) as u64;

        if payload_len > MAX_ALLOWED_LEN.try_into().unwrap() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Frame too large"));
        }

        if payload_len == 126 {
            let mut extended = [0u8; 2];
            reader.read_exact(&mut extended)?;
            payload_len = u16::from_be_bytes(extended) as u64;
        } else if payload_len == 127 {
            let mut extended = [0u8; 8];
            reader.read_exact(&mut extended)?;
            payload_len = u64::from_be_bytes(extended);
        }

        let masking_key = if masked {
            let mut key = [0u8; 4];
            reader.read_exact(&mut key)?;
            Some(key)
        } else {
            None
        };

        let mut payload = vec![0u8; payload_len as usize];
        reader.read_exact(&mut payload)?;

        if let Some(key) = masking_key {
            Self::apply_mask(&mut payload, key);
        }

        if rsv1 && compression {
            payload = compression::decompress(&payload)?;
        }

        Ok(Self {
            fin,
            rsv1,
            opcode,
            payload,
            masked,
            masking_key,
        })
    }

    /// Writes a frame
    pub fn write<W: Write>(&self, writer: &mut W, compression: bool) -> io::Result<()> {
        let mut first_byte = 0u8;
        let mut rsv1 = self.rsv1;
        let mut payload = self.payload.clone();

        if compression && matches!(self.opcode, OpCode::Text | OpCode::Binary) {
            payload = compression::compress(&payload)?;
            rsv1 = true
        }

        if self.fin { first_byte |= 0x80 };
        if rsv1 { first_byte |= 0x40 };
        first_byte |= self.opcode as u8;

        writer.write_all(&[first_byte])?;

        let mask_bit = if self.masked { 0x80 } else { 0x00 };
        let payload_len = payload.len();

        if payload_len < 126 {
            writer.write_all(&[(mask_bit | (payload_len as u8))])?
        } else if payload_len <= u16::MAX as usize {
            writer.write_all(&[mask_bit | 126])?;
            writer.write_all(&(payload_len as u16).to_be_bytes())?;
        } else {
            writer.write_all(&[mask_bit | 127])?;
            writer.write_all(&(payload_len as u64).to_be_bytes())?;
        }

        if let Some(key) = self.masking_key {
            writer.write_all(&key)?;

            let mut masked_payload = payload.clone();
            Self::apply_mask(&mut masked_payload, key);
            writer.write_all(&masked_payload)?;
        } else {
            writer.write_all(&payload)?;
        }

        Ok(())
    }
}