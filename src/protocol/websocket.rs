//! WebSocket handler

use std::io::{self, Read, Write};

use crate::{protocol::{config::WebSocketConfig, frame::{Frame, OpCode}, message::Message}, MAX_CONTINUATION_FRAMES, MAX_CONTROL_FRAME_PAYLOAD};

/// WebSocket operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Client mode
    Client,
    /// Server mode
    Server
}

/// WebSocket input-output stream.
///
/// This is THE structure you want to create to be able to speak the WebSocket protocol.
/// It may be created by calling `connect`, `accept` or `client` functions.
///
/// Use [`WebSocket::read`], [`WebSocket::send`] to received and send messages.
#[derive(Debug)]
pub struct WebSocket<T> {
    stream: T,
    read_buffer: Vec<u8>,
    mode: OperationMode,
    config: WebSocketConfig
}

impl<T: Read + Write> WebSocket<T> {
    /// Initializes a new WebSocket stream
    pub fn new(stream: T, mode: OperationMode) -> Self {
        Self::with_config(stream, mode, WebSocketConfig::default())
    }

    /// Initializes a new WebSocket stream with configuration options
    pub fn with_config(stream: T, mode: OperationMode, config: WebSocketConfig) -> Self {
        WebSocket {
            stream,
            read_buffer: Vec::with_capacity(4096),
            mode,
            config
        }
    }

    /// Reads an incoming message in the stream
    pub fn read_message(&mut self) -> io::Result<Message> {
        let frame = Frame::read(&mut self.stream, self.config.compression.enabled)?;

        match self.mode {
            OperationMode::Server => {
                if !frame.masked {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Clients must mask frames"))
                }
            },
            OperationMode::Client => {
                if frame.masked {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Servers must not mask frames"))
                }
            }
        }

        if matches!(frame.opcode, OpCode::Ping | OpCode::Pong | OpCode::Close) {
            if !frame.fin {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Control frame must not be fragmented"))
            }

            if frame.payload.len() > MAX_CONTROL_FRAME_PAYLOAD {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Control frame payload too large"))
            }
        }

        match frame.opcode {
            OpCode::Text | OpCode::Binary if !frame.fin => {
                let mut payload = frame.payload;
                let is_text = frame.opcode == OpCode::Text;

                let mut continuation_count = 0;

                loop {
                    if continuation_count > MAX_CONTINUATION_FRAMES {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Too many continuation frames"))
                    }

                    let cont = Frame::read(&mut self.stream, self.config.compression.enabled)?;
                    if cont.opcode != OpCode::Continuation {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Expected continuation frame"))
                    }

                    payload.extend(cont.payload);
                    if self.config.max_message_size.is_some() {
                        if payload.len() > self.config.max_message_size.unwrap() {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large"))
                        }
                    }

                    continuation_count += 1;

                    if cont.fin {
                        break;
                    }
                }

                if is_text {
                    let data = String::from_utf8(payload).map_err(|e| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8: {}", e))
                    })?;

                    Ok(Message::Text(data))
                } else {
                    Ok(Message::Binary(payload))
                }
            }
            OpCode::Text => {
                let data = String::from_utf8(frame.payload).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8: {}", e))
                })?;

                Ok(Message::Text(data))
            },
            OpCode::Binary => Ok(Message::Binary(frame.payload)),
            OpCode::Ping => Ok(Message::Ping(frame.payload)),
            OpCode::Pong => Ok(Message::Pong(frame.payload)),
            OpCode::Continuation => Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected continuation")),
            OpCode::Close => Ok(Message::from_close_payload(frame.payload)),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported opcode"))
        }
    }

    /// Writes an outgoing message from the stream
    pub fn write_message(&mut self, msg: Message) -> io::Result<()> {
        let frame = match msg {
            Message::Text(s) => Frame::new(OpCode::Text, false, self.config.compression.enabled, false, s.into_bytes()),
            Message::Binary(b) => Frame::new(OpCode::Binary, false, self.config.compression.enabled, false, b),
            Message::Ping(b) => Frame::new(OpCode::Ping, false, self.config.compression.enabled, false, b),
            Message::Pong(b) => Frame::new(OpCode::Pong, false, self.config.compression.enabled, false, b),
            Message::Close(Some((code, reason))) => {
                let mut payload = Vec::with_capacity(2 + reason.len());
                payload.extend_from_slice(&code.to_be_bytes());
                payload.extend_from_slice(&reason.as_bytes());

                Frame::new(OpCode::Close, false, self.config.compression.enabled, false, payload)
            },
            Message::Close(None) => Frame::new(OpCode::Close, false, self.config.compression.enabled, false, vec![])
        };

        frame.write(&mut self.stream, self.config.compression.enabled)
    }

    /// Returns a mutable reference to the stream
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Returns the inner instance of the stream
    pub fn into_inner(self) -> T {
        self.stream
    }
}