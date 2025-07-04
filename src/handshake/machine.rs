//! WebSocket handshake machine

use std::io::{Cursor, Read, Write};

use bytes::Buf;

use crate::{error::{Error, ProtocolError, Result}, util::NonBlockingResult, ReadBuffer};

/// A generic handshake state machine 
#[derive(Debug)]
pub struct HandshakeMachine<Stream> {
    stream: Stream,
    state: HandshakeState
}

impl<Stream> HandshakeMachine<Stream> {
    /// Start reading data from the peer
    pub fn start_read(stream: Stream) -> Self {
        Self {
            stream,
            state: HandshakeState::Reading(ReadBuffer::new(), AttackCheck::new())
        }
    }

    /// Start writing data to the peer
    pub fn start_write<D: Into<Vec<u8>>>(stream: Stream, data: D) -> Self {
        HandshakeMachine {
            stream,
            state: HandshakeState::Writing(Cursor::new(data.into()))
        }
    }

    /// Returns a shared reference to the internal stream
    pub fn get_ref(&self) -> &Stream {
        &self.stream
    }

    /// Returns a mutable reference to the internal stream
    pub fn get_mut(&mut self) -> &mut Stream {
        &mut self.stream
    }
}

impl<Stream: Read + Write> HandshakeMachine<Stream> {
    /// Performs a single-round handshake
    pub fn single_round<Object: TryParse>(mut self) -> Result<RoundResult<Object, Stream>> {
        match self.state {
            HandshakeState::Reading(mut buf, mut attack_check) => {
                let read = buf.read_from(&mut self.stream).no_block()?;
                match read {
                    Some(0) => Err(Error::Protocol(ProtocolError::IncompleteHandshake)),
                    Some(count) => {
                        attack_check.check_incoming_packet(count)?;
                        if let Some((size, obj)) = Object::try_parse(Buf::chunk(&buf))? {
                            buf.advance(size);

                            Ok(RoundResult::StageFinished(StageResult::DoneReading {
                                result: obj,
                                stream: self.stream,
                                tail: buf.into_vec()
                            }))
                        } else {
                            Ok(RoundResult::Incomplete(HandshakeMachine {
                                state: HandshakeState::Reading(buf, attack_check),
                                ..self
                            }))
                        }
                    },
                    None => Ok(RoundResult::WouldBlock(HandshakeMachine {
                        state: HandshakeState::Reading(buf, attack_check),
                        ..self
                    }))
                }
            },
            HandshakeState::Writing(mut buf) => {
                assert!(buf.has_remaining());

                if let Some(size) = self.stream.write(Buf::chunk(&buf)).no_block()? {
                    assert!(size > 0);

                    buf.advance(size);

                    Ok(if buf.has_remaining() {
                        RoundResult::Incomplete(HandshakeMachine {
                            state: HandshakeState::Writing(buf),
                            ..self
                        })
                    } else {
                        RoundResult::Incomplete(HandshakeMachine {
                            state: HandshakeState::Flushing,
                            ..self
                        })
                    })
                } else {
                    Ok(RoundResult::WouldBlock(HandshakeMachine {
                        state: HandshakeState::Writing(buf),
                        ..self
                    }))
                }
            },
            HandshakeState::Flushing => {
                match self.stream.flush().no_block()? {
                    Some(()) => Ok(RoundResult::StageFinished(StageResult::DoneWriting(self.stream))),
                    None => Ok(RoundResult::WouldBlock(HandshakeMachine {
                        state: HandshakeState::Flushing,
                        ..self
                    }))
                }
            }
        }
    }
}

/// The result of the Round
#[derive(Debug)]
pub enum RoundResult<Object, Stream> {
    /// Round not done, I/O would block
    WouldBlock(HandshakeMachine<Stream>),
    /// Round done, stage unchanged
    Incomplete(HandshakeMachine<Stream>),
    /// Stage complete 
    StageFinished(StageResult<Object, Stream>)
}

/// The result of the stage
#[derive(Debug)]
pub enum StageResult<Object, Stream> {
    /// Reading finished round
    #[allow(missing_docs)]
    DoneReading {
        result: Object,
        stream: Stream,
        tail: Vec<u8>
    },
    /// Writing finished round
    DoneWriting(Stream)
}

/// A parse-able object
pub trait TryParse: Sized {
    /// Returns Ok(None) if incomplete, Err on syntax errors
    fn try_parse(data: &[u8]) -> Result<Option<(usize, Self)>>;
}

/// The handshake state
#[derive(Debug)]
enum HandshakeState {
    /// Reading data from peer
    Reading(ReadBuffer, AttackCheck),
    /// Sending data to peer
    Writing(Cursor<Vec<u8>>),
    /// Flushing data to ensure that all intermediaries reach their destinations
    Flushing
}

/// Attack mitigation against DoS attacks
#[derive(Debug)]
pub(crate) struct AttackCheck {
    /// Number of HTTP header successful reads (TCP packets)
    packets: usize,
    /// Total number of bytes in HTTP header
    bytes: usize
}

impl AttackCheck {
    /// Initialize attack checking for incoming buffer
    fn new() -> Self {
        Self {
            packets: 0,
            bytes: 0
        }
    }

    /// Check the size of an incoming packet. To be called immediately after `read()`
    /// passing its returned bytes count as `size`
    fn check_incoming_packet(&mut self, size: usize) -> Result<()> {
        self.packets += 1;
        self.bytes += size;

        const MAX_BYTES: usize = 65536;
        const MAX_PACKETS: usize = 512;
        const MIN_PACKET_SIZE: usize = 128;
        const MIN_PACKET_CHECK_THRESHOLD: usize = 64;

        if self.bytes > MAX_BYTES
            || self.packets > MAX_PACKETS
            || (self.packets > MIN_PACKET_CHECK_THRESHOLD && self.packets * MIN_PACKET_SIZE > self.bytes) 
        {
            return Err(Error::AttackAttempt)
        }

        Ok(())
    }
}