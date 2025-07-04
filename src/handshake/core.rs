//! WebSocket handshake control

use std::{fmt::{Debug, Display}, io::{Read, Write}};

use base64::Engine;
use sha1::{Digest, Sha1};

use crate::{error::{Error, Result}, handshake::machine::{HandshakeMachine, RoundResult, StageResult, TryParse}};

/// A WebSocket Handshake
#[derive(Debug)]
pub struct MidHandshake<Role: HandshakeRole> {
    /// The Handshake role
    pub role: Role,
    /// The handshake machine instance
    pub machine: HandshakeMachine<Role::InternalStream>
}

impl<Role: HandshakeRole> MidHandshake<Role> {
    /// Allows access to the machine
    pub fn get_ref(&self) -> &HandshakeMachine<Role::InternalStream> {
        &self.machine
    }

    /// Allows mutable access to the machine
    pub fn get_mut(&mut self) -> &mut HandshakeMachine<Role::InternalStream> {
        &mut self.machine
    }

    /// Restarts the handshake process
    pub fn handshake(mut self) -> Result<Role::FinalResult, HandshakeError<Role>> {
        let mut machine = self.machine;

        loop {
            machine = match machine.single_round()? {
                RoundResult::WouldBlock(m) => {
                    return Err(HandshakeError::Interrupted(MidHandshake {
                        machine: m,
                        ..self
                    }))
                },
                RoundResult::Incomplete(m) => m,
                RoundResult::StageFinished(s) => match self.role.stage_finished(s)? {
                    ProcessingResult::Continue(m) => m,
                    ProcessingResult::Done(res) => return Ok(res)
                }
            }
        }
    }
}

/// A handshake result
pub enum HandshakeError<Role: HandshakeRole> {
    /// Handshake was interrupted (would block)
    Interrupted(MidHandshake<Role>),
    /// Handshake failed
    Failure(Error)
}

impl<Role: HandshakeRole> Debug for HandshakeError<Role> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted(_) => write!(f, "HandshakeError::Interrupted(...)"),
            Self::Failure(e) => write!(f, "HandshakeError::Failure({:?})", e)
        }
    }
}

impl<Role: HandshakeRole> Display for HandshakeError<Role> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted(_) => write!(f, "Interrupted handshake (WouldBlock)"),
            Self::Failure(e) => write!(f, "{e}")
        }
    }
}

impl<Role: HandshakeRole> std::error::Error for HandshakeError<Role> {}

impl<Role: HandshakeRole> From<Error> for HandshakeError<Role> {
    fn from(value: Error) -> Self {
        HandshakeError::Failure(value)
    }
}

/// Handshake Role
pub trait HandshakeRole {
    #[doc(hidden)]
    type IncomingData: TryParse;
    #[doc(hidden)]
    type InternalStream: Read + Write;
    #[doc(hidden)]
    type FinalResult;

    #[doc(hidden)]
    fn stage_finished(
        &mut self,
        finish: StageResult<Self::IncomingData, Self::InternalStream>
    ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>>;
}

#[doc(hidden)]
#[derive(Debug)]
pub enum ProcessingResult<Stream, FinalResult> {
    Continue(HandshakeMachine<Stream>),
    Done(FinalResult)
}

/// Derives the `Sec-WebSocket-Accept` header value from a `Sec-WebSocket-Key` request header.
/// 
/// This function can be used to perform a handshake before passing a raw TCP stream to
/// [`WebSocket::with_config`][crate::protocol::WebSocket::with_config]
pub fn derive_accept_key(req_key: &[u8]) -> String {
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = Sha1::default();
    <Sha1 as Digest>::update(&mut hasher, req_key);
    <Sha1 as Digest>::update(&mut hasher, WS_GUID);

    base64::engine::general_purpose::STANDARD.encode(<Sha1 as Digest>::finalize(hasher))
}