use std::io::{self, Read, Write};

use crate::protocol::{config::WebSocketConfig, frame::{Frame, OpCode}, message::Message};

pub enum Mode {
    Client,
    Server
}

pub struct WebSocket<T> {
    stream: T,
    read_buffer: Vec<u8>,
    mode: Mode,
    config: WebSocketConfig
}

impl<T: Read + Write> WebSocket<T> {
    pub fn new(stream: T, mode: Mode) -> Self {
        Self::with_config(stream, mode, WebSocketConfig::default())
    }

    pub fn with_config(stream: T, mode: Mode, config: WebSocketConfig) -> Self {
        WebSocket {
            stream,
            read_buffer: Vec::with_capacity(4096),
            mode,
            config
        }
    }

    pub fn read_message(&mut self) -> io::Result<Message> {
        let frame = Frame::read(&mut self.stream, self.config.compression.enabled)?;

        match self.mode {
            Mode::Server => {
                if !frame.masked {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Clients must mask frames"))
                }
            },
            Mode::Client => {
                if frame.masked {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Servers must not mask frames"))
                }
            }
        }

        if matches!(frame.opcode, OpCode::Ping | OpCode::Pong | OpCode::Close) {
            if !frame.fin {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Control frame must not be fragmented"))
            }

            if frame.payload.len() > self.config.max_control_frame_payload {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Control frame payload too large"))
            }
        }

        match frame.opcode {
            OpCode::Text | OpCode::Binary if !frame.fin => {
                let mut payload = frame.payload;
                let is_text = frame.opcode == OpCode::Text;

                let mut continuation_count = 0;

                loop {
                    if continuation_count > self.config.max_continuation_frames {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Too many continuation frames"))
                    }

                    let cont = Frame::read(&mut self.stream, self.config.compression.enabled)?;
                    if cont.opcode != OpCode::Continuation {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Expected continuation frame"))
                    }

                    payload.extend(cont.payload);
                    if payload.len() > self.config.max_message_size {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large"))
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

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    pub fn into_inner(self) -> T {
        self.stream
    }
}