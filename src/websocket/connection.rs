use std::{io::{self, BufReader, BufWriter}, net::TcpStream};

use crate::websocket::frame::{OpCode, WebSocketFrame};

pub struct WebSocketConnection {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>
}

impl WebSocketConnection {
    pub fn new(stream: TcpStream) -> Self {
        let reader = BufReader::new(stream.try_clone().expect("Failed to clone stream"));
        let writer = BufWriter::new(stream);

        Self { reader, writer }
    }

    pub fn handle_next_frame(&mut self) -> io::Result<()> {
        let frame = WebSocketFrame::read_from(&mut self.reader)?;
        println!("Received frame: {:?}", frame);

        match frame.opcode {
            OpCode::Text => self.send_text(std::str::from_utf8(&frame.payload).unwrap_or("[invalid utf8]"))?,
            OpCode::Ping => self.send_pong(&frame.payload)?,
            OpCode::Close => self.send_close(&frame.payload)?,
            _ => {}
        }

        Ok(())
    }

    pub fn send_text(&mut self, text: &str) -> io::Result<()> {
        let frame = WebSocketFrame {
            fin: true,
            opcode: OpCode::Text,
            payload: text.as_bytes().to_vec(),
            is_masked: false,
            masking_key: None,
        };
        self.send_frame(frame)
    }

    pub fn send_pong(&mut self, payload: &[u8]) -> io::Result<()> {
        let frame = WebSocketFrame {
            fin: true,
            opcode: OpCode::Pong,
            payload: payload.to_vec(),
            is_masked: false,
            masking_key: None,
        };
        self.send_frame(frame)
    }

    pub fn send_close(&mut self, payload: &[u8]) -> io::Result<()> {
        let frame = WebSocketFrame {
            fin: true,
            opcode: OpCode::Close,
            payload: payload.to_vec(),
            is_masked: false,
            masking_key: None,
        };
        self.send_frame(frame)
    }

    pub fn send_frame(&mut self, frame: WebSocketFrame) -> io::Result<()> {
        frame.write_to(&mut self.writer)
    }

    pub fn run(&mut self) -> io::Result<()> {
        loop {
            match self.handle_next_frame() {
                Ok(_) => continue,
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    println!("Client disconnected.");
                    break;
                },
                Err(e) => {
                    eprintln!("Error handling frame: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
}