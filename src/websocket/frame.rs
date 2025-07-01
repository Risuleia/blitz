use std::io::{self, Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    Other(u8)
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
            other => OpCode::Other(other)
        }
    }
}

impl From<OpCode> for u8 {
    fn from(op: OpCode) -> Self {
        match op {
            OpCode::Continuation => 0x0,
            OpCode::Text => 0x1,
            OpCode::Binary => 0x2,
            OpCode::Close => 0x8,
            OpCode::Ping => 0x9,
            OpCode::Pong => 0xA,
            OpCode::Other(b) => b
        }
    }
}

const MAX_ALLOWED_LEN: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: OpCode,
    pub payload: Vec<u8>,
    pub is_masked: bool,
    pub masking_key: Option<[u8; 4]>
}

impl WebSocketFrame {
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut header = [0u8; 2];
        reader.read_exact(&mut header)?;

        let fin = header[0] & 0b1000_0000 != 0;
        let opcode = OpCode::from(header[0] & 0b0000_1111);

        let is_masked = header[1] & 0b1000_0000 != 0;
        let mut payload_len = (header[1] & 0b0111_1111) as u64;

        if payload_len == 126 {
            let mut extended = [0u8; 2];
            reader.read_exact(&mut extended)?;
            payload_len = u16::from_be_bytes(extended) as u64;
        } else if payload_len == 127 {
            let mut extended = [0u8; 8];
            reader.read_exact(&mut extended)?;
            payload_len = u64::from_be_bytes(extended);
        }

        let masking_key = if is_masked {
            let mut key = [0u8; 4];
            reader.read_exact(&mut key)?;
            Some(key)
        } else {
            None
        };

        let mut payload = vec![0u8; payload_len as usize];
        reader.read_exact(&mut payload)?;

        if let Some(key) = masking_key {
            for i in 0..payload.len() {
                payload[i] ^= key[i % 4];
            }
        }

        Ok(Self {
            fin,
            opcode,
            payload,
            is_masked,
            masking_key
        })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut header = Vec::with_capacity(14);

        let byte1 = (if self.fin { 0b1000_0000 } else { 0 }) | (u8::from(self.opcode) & 0x0F);
        header.push(byte1);

        let mut byte2 = 0u8;
        let payload_len = self.payload.len();

        if payload_len > MAX_ALLOWED_LEN.try_into().unwrap() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Payload too large"));
        }

        if payload_len <= 125 {
            byte2 |= payload_len as u8;
            header.push(byte2);
        } else if payload_len <= 65535 {
            byte2 |= 126;
            header.push(byte2);
            header.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            byte2 |= 127;
            header.push(byte2);
            header.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }

        writer.write_all(&header)?;
        writer.write_all(&self.payload)?;
        writer.flush()
    }
}