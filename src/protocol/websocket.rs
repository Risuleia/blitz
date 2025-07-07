//! WebSocket handler

use std::{io::{self, Read, Write}, mem::replace};

use crate::{error::{CapacityError, Error, ProtocolError, Result}, protocol::{config::WebSocketConfig, frame::{codec::{CloseCode, Control, Data, OpCode}, core::FrameCodec, CloseFrame, Frame, Utf8Bytes}, message::{IncompleteMessage, IncompleteMessageType, Message}}, MAX_CONTROL_FRAME_PAYLOAD};

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
    context: WebSocketContext
}

impl<T: Read + Write> WebSocket<T> {
    /// Convert a raw socket into a WebSocket without performing a handshake.
    ///
    /// Call this function if you're using Tungstenite as a part of a web framework
    /// or together with an existing one. If you need an initial handshake, use
    /// `connect()` or `accept()` functions of the crate to construct a websocket.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn new(stream: T, mode: OperationMode, config: Option<WebSocketConfig>) -> Self {
        WebSocket {
            stream,
            context: WebSocketContext::new(mode, config)
        }
    }

    /// Convert a raw socket into a WebSocket without performing a handshake.
    ///
    /// Call this function if you're using Tungstenite as a part of a web framework
    /// or together with an existing one. If you need an initial handshake, use
    /// `connect()` or `accept()` functions of the crate to construct a websocket.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn from_partially_read(stream: T, part: Vec<u8>, mode: OperationMode, config: Option<WebSocketConfig>) -> Self {
        WebSocket {
            stream,
            context: WebSocketContext::from_partially_read(part, mode, config)
        }
    }

    /// Returns a shared reference to the stream
    pub fn get_ref(&self) -> &T {
        &self.stream
    }

    /// Returns a mutable reference to the stream
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Returns the inner instance of the stream
    pub fn into_inner(self) -> T {
        self.stream
    }

    /// Change the configuration.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn set_config(&mut self, func: impl FnOnce(&mut WebSocketConfig)) {
        self.context.set_config(func);
    }
    
    /// Read the configuration.
    pub fn get_config(&self) -> &WebSocketConfig {
        self.context.get_config()
    }

    /// Check if it is possible to read messages.
    ///
    /// Reading is impossible after receiving `Message::Close`. It is still possible after
    /// sending close frame since the peer still may send some data before confirming close.
    pub fn can_read(&self) -> bool {
        self.context.can_read()
    }

    /// Check if it is possible to write messages.
    ///
    /// Writing gets impossible immediately after sending or receiving `Message::Close`.
    pub fn can_write(&self) -> bool {
        self.context.can_write()
    }

    /// Check if it is possible to read messages.
    ///
    /// Reading is impossible after receiving `Message::Close`. It is still possible after
    /// sending close frame since the peer still may send some data before confirming close.
    pub fn read(&mut self) -> Result<Message> {
        self.context.read(&mut self.stream)
    }

    /// Writes and immediately flushes a message.
    /// Equivalent to calling [`write`](Self::write) then [`flush`](Self::flush).
    pub fn send(&mut self, msg: Message) -> Result<()> {
        self.write(msg)?;
        self.flush()
    }

    /// Write a message to the provided stream, if possible.
    ///
    /// A subsequent call should be made to [`flush`](Self::flush) to flush writes.
    ///
    /// In the event of stream write failure the message frame will be stored
    /// in the write buffer and will try again on the next call to [`write`](Self::write)
    /// or [`flush`](Self::flush).
    ///
    /// If the write buffer would exceed the configured [`WebSocketConfig::max_write_buffer_size`]
    /// [`Err(WriteBufferFull(msg_frame))`](Error::WriteBufferFull) is returned.
    ///
    /// This call will generally not flush. However, if there are queued automatic messages
    /// they will be written and eagerly flushed.
    ///
    /// For example, upon receiving ping messages tungstenite queues pong replies automatically.
    /// The next call to [`read`](Self::read), [`write`](Self::write) or [`flush`](Self::flush)
    /// will write & flush the pong reply. This means you should not respond to ping frames manually.
    ///
    /// You can however send pong frames manually in order to indicate a unidirectional heartbeat
    /// as described in [RFC 6455](https://tools.ietf.org/html/rfc6455#section-5.5.3). Note that
    /// if [`read`](Self::read) returns a ping, you should [`flush`](Self::flush) before passing
    /// a custom pong to [`write`](Self::write), otherwise the automatic queued response to the
    /// ping will not be sent as it will be replaced by your custom pong message.
    ///
    /// # Errors
    /// - If the WebSocket's write buffer is full, [`Error::WriteBufferFull`] will be returned
    ///   along with the equivalent passed message frame.
    /// - If the connection is closed and should be dropped, this will return [`Error::ConnectionClosed`].
    /// - If you try again after [`Error::ConnectionClosed`] was returned either from here or from
    ///   [`read`](Self::read), [`Error::AlreadyClosed`] will be returned. This indicates a program
    ///   error on your part.
    /// - [`Error::Io`] is returned if the underlying connection returns an error
    ///   (consider these fatal except for WouldBlock).
    /// - [`Error::Capacity`] if your message size is bigger than the configured max message size.
    pub fn write(&mut self, msg: Message) -> Result<()> {
        self.context.write(&mut self.stream, msg)
    }

    /// Flush writes.
    ///
    /// Ensures all messages previously passed to [`write`](Self::write) and automatic
    /// queued pong responses are written & flushed into the underlying stream.
    pub fn flush(&mut self) -> Result<()> {
        self.context.flush(&mut self.stream)
    }

    /// Close the connection.
    ///
    /// This function guarantees that the close frame will be queued.
    /// There is no need to call it again. Calling this function is
    /// the same as calling `write(Message::Close(..))`.
    ///
    /// After queuing the close frame you should continue calling [`read`](Self::read) or
    /// [`flush`](Self::flush) to drive the close handshake to completion.
    ///
    /// The websocket RFC defines that the underlying connection should be closed
    /// by the server. Tungstenite takes care of this asymmetry for you.
    ///
    /// When the close handshake is finished (we have both sent and received
    /// a close message), [`read`](Self::read) or [`flush`](Self::flush) will return
    /// [Error::ConnectionClosed] if this endpoint is the server.
    ///
    /// If this endpoint is a client, [Error::ConnectionClosed] will only be
    /// returned after the server has closed the underlying connection.
    ///
    /// It is thus safe to drop the underlying connection as soon as [Error::ConnectionClosed]
    /// is returned from [`read`](Self::read) or [`flush`](Self::flush).
    pub fn close(&mut self, code: Option<CloseFrame>) -> Result<()> {
        self.context.close(&mut self.stream, code)
    }
}

/// A context for managing WebSocket stream.
#[derive(Debug)]
pub struct WebSocketContext {
    /// Server or client?
    mode: OperationMode,
    /// encoder / decoder of frame.
    frame: FrameCodec,
    /// The state of processing, either "active" or "closing".
    state: WebSocketState,
    /// Receive: an incomplete message being processed.
    incomplete: Option<IncompleteMessage>,
    /// Send in addition to regular messages E.g. "pong" or "close".
    additional_send: Option<Frame>,
    /// True indicates there is an additional message (like a pong)
    /// that failed to flush previously and we should try again.
    unflushed_additional: bool,
    /// The configuration for the websocket session.
    config: WebSocketConfig
}

impl WebSocketContext {
    /// Create a WebSocket context that manages a post-handshake stream.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn new(mode: OperationMode, config: Option<WebSocketConfig>) -> Self {
        let configuration = config.unwrap_or_default();
        Self::_new(mode, FrameCodec::new(configuration.read_buffer_size), configuration)
    }

    /// Create a WebSocket context that manages an post-handshake stream.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn from_partially_read(part: Vec<u8>, mode: OperationMode, config: Option<WebSocketConfig>) -> Self {
        let configuration = config.unwrap_or_default();
        Self::_new(mode, FrameCodec::from_partially_read(part, configuration.read_buffer_size), configuration)
    }

    fn _new(mode: OperationMode, mut frame: FrameCodec, config: WebSocketConfig) -> Self {
        config.asset_valid();

        frame.max_out_buffer_len(config.max_write_buffer_size);
        frame.out_buffer_write_len(config.write_buffer_size);

        Self {
            mode,
            frame,
            state: WebSocketState::Active,
            incomplete: None,
            additional_send: None,
            unflushed_additional: false,
            config
        }
    }

    /// Change the configuration.
    ///
    /// # Panics
    /// Panics if config is invalid e.g. `max_write_buffer_size <= write_buffer_size`.
    pub fn set_config(&mut self, func: impl FnOnce(&mut WebSocketConfig)) {
        func(&mut self.config);

        self.config.asset_valid();
        self.frame.max_out_buffer_len(self.config.max_write_buffer_size);
        self.frame.out_buffer_write_len(self.config.write_buffer_size);
    }

    /// Read the configuration.
    pub fn get_config(&self) -> &WebSocketConfig {
        &self.config
    }

    /// Check if it is possible to read messages.
    ///
    /// Reading is impossible after receiving `Message::Close`. It is still possible after
    /// sending close frame since the peer still may send some data before confirming close.
    pub fn can_read(&self) -> bool {
        self.state.can_read()
    }

    /// Check if it is possible to write messages.
    ///
    /// Writing gets impossible immediately after sending or receiving `Message::Close`.
    pub fn can_write(&self) -> bool {
        self.state.is_active()
    }

    /// Read a message from the provided stream, if possible.
    ///
    /// This function sends pong and close responses automatically.
    /// However, it never blocks on write.
    pub fn read<T: Read + Write>(&mut self, stream: &mut T) -> Result<Message> {
        self.state.check_if_terminated()?;

        loop {
            if self.additional_send.is_some() || self.unflushed_additional {
                match self.flush(stream) {
                    Ok(_) => {},
                    Err(Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => self.unflushed_additional = true,
                    Err(e) => return Err(e)
                }
            } else if self.mode == OperationMode::Server && !self.state.can_read() {
                self.state = WebSocketState::Terminated;
                return Err(Error::ConnectionClosed);
            }

            if let Some(msg) = self._read(stream)? {
                return Ok(msg);
            }
        }
    }

    /// Write a message to the provided stream.
    ///
    /// A subsequent call should be made to [`flush`](Self::flush) to flush writes.
    ///
    /// In the event of stream write failure the message frame will be stored
    /// in the write buffer and will try again on the next call to [`write`](Self::write)
    /// or [`flush`](Self::flush).
    ///
    /// If the write buffer would exceed the configured [`WebSocketConfig::max_write_buffer_size`]
    /// [`Err(WriteBufferFull(msg_frame))`](Error::WriteBufferFull) is returned.
    pub fn write<T: Read + Write>(&mut self, stream: &mut T, msg: Message) -> Result<()> {
        self.state.check_if_terminated()?;

        if !self.state.is_active() {
            return Err(Error::Protocol(ProtocolError::SendAfterClose));
        }

        let frame = match msg {
            Message::Text(data) => Frame::new_data(data, OpCode::Data(Data::Text), true),
            Message::Binary(data) => Frame::new_data(data, OpCode::Data(Data::Binary), true),
            Message::Ping(data) => Frame::new_ping(data),
            Message::Pong(data) => {
                self.set_additional(Frame::new_pong(data));
                return self._write(stream, None).map(|_| ());
            },
            Message::Close(code) => return self.close(stream, code),
            Message::Frame(f) => f
        };

        let should_flush = self._write(stream, Some(frame))?;
        if should_flush {
            self.flush(stream)?;
        }

        Ok(())
    }

    /// Flush writes.
    ///
    /// Ensures all messages previously passed to [`write`](Self::write) and automatically
    /// queued pong responses are written & flushed into the `stream`.
    #[inline]
    pub fn flush<T: Read + Write>(&mut self, stream: &mut T) -> Result<()> {
        self._write(stream, None)?;
        self.frame.write_out(stream)?;

        stream.flush()?;

        self.unflushed_additional = false;

        Ok(())
    }

    /// Close the connection.
    ///
    /// This function guarantees that the close frame will be queued.
    /// There is no need to call it again. Calling this function is
    /// the same as calling `send(Message::Close(..))`.
    pub fn close<T: Read + Write>(&mut self, stream: &mut T, code: Option<CloseFrame>) -> Result<()> {
        if let WebSocketState::Active = self.state {
            self.state = WebSocketState::ClosedByServer;

            let frame = Frame::new_close(code);

            self._write(stream, Some(frame))?;
        }

        self.flush(stream)
    }

    fn _read<T: Read>(&mut self, stream: &mut T) -> Result<Option<Message>> {
        if let Some(frame) = self
            .frame
            .read(
                stream,
                self.config.max_frame_size,
                matches!(self.mode, OperationMode::Server),
                self.config.accept_unmasked_frames
            )
            .check_connection_reset(self.state)?
        {
            if !self.state.can_read() {
                return Err(Error::Protocol(ProtocolError::ReceiveAfterClose));
            }

            let header = frame.header();
            if header.rsv1 || header.rsv2 || header.rsv3 {
                return Err(Error::Protocol(ProtocolError::NonZeroReservedBits));
            }

            if self.mode == OperationMode::Client && frame.is_masked() {
                return Err(Error::Protocol(ProtocolError::MaskedFrameFromServer));
            }

            match frame.header().opcode {
                OpCode::Control(ctrl) => {
                    match ctrl {
                        _ if !frame.header().fin => Err(Error::Protocol(ProtocolError::FragmentedControlFrame)),
                        _ if frame.payload().len() > MAX_CONTROL_FRAME_PAYLOAD => Err(Error::Protocol(ProtocolError::ControlFrameTooBig)),
                        Control::Close => Ok(self.try_close(frame.into_close()?).map(Message::Close)),
                        Control::Reserved(code) => Err(Error::Protocol(ProtocolError::UnknownControlOpCode(code))),
                        Control::Ping => {
                            let data = frame.into_payload();
                            if self.state.is_active() {
                                self.set_additional(Frame::new_pong(data.clone()));
                            }

                            Ok(Some(Message::Ping(data)))
                        },
                        Control::Pong => Ok(Some(Message::Pong(frame.into_payload())))
                    }
                },
                OpCode::Data(data) => {
                    let fin = frame.header().fin;

                    match data {
                        Data::Continuation => {
                            if let Some(ref mut msg) = self.incomplete {
                                msg.extend(frame.into_payload(), self.config.max_message_size)?;
                            } else {
                                return Err(Error::Protocol(ProtocolError::UnexpectedContinue));
                            }

                            if fin {
                                Ok(Some(self.incomplete.take().unwrap().complete()?))
                            } else {
                                Ok(None)
                            }
                        },
                        data_frag if self.incomplete.is_some() => {
                            Err(Error::Protocol(ProtocolError::ExpectedFragment(data_frag)))
                        },
                        Data::Text if fin => {
                            check_max_size(frame.payload().len(), self.config.max_message_size)?;
                            Ok(Some(Message::Text(frame.into_text()?)))
                        },
                        Data::Binary if fin => {
                            check_max_size(frame.payload().len(), self.config.max_message_size)?;
                            Ok(Some(Message::Binary(frame.into_payload())))
                        },
                        Data::Text | Data::Binary => {
                            let msg_type = match data {
                                Data::Text => IncompleteMessageType::Text,
                                Data::Binary => IncompleteMessageType::Binary,
                                _ => panic!("Bug: message is neither text not binary")
                            };

                            let mut incomplete = IncompleteMessage::new(msg_type);
                            incomplete.extend(frame.into_payload(), self.config.max_message_size)?;
                            
                            self.incomplete = Some(incomplete);

                            Ok(None)
                        },
                        Data::Reserved(code) => Err(Error::Protocol(ProtocolError::UnknownDataOpCode(code)))
                    }
                }
            }
        } else {
            match replace(&mut self.state, WebSocketState::Terminated) {
                WebSocketState::ClosedByPeer | WebSocketState::CloseAcknowledged => Err(Error::ConnectionClosed),
                _ => Err(Error::Protocol(ProtocolError::ResetWithoutClosing))
            }
        }
    }

    fn _write<T: Read + Write>(&mut self, stream: &mut T, data: Option<Frame>) -> Result<bool> {
        if let Some(data) = data {
            self.buffer_frame(stream, data)?;
        }

        let should_flush = if let Some(msg) = self.additional_send.take() {
            match self.buffer_frame(stream, msg.clone()) {
                Err(Error::WriteBufferFull) => {
                    self.set_additional(msg);
                    false
                },
                Err(e) => return Err(e),
                Ok(_) => true
            }
        } else {
            self.unflushed_additional
        };

        if self.mode == OperationMode::Server && !self.state.can_read() {
            self.frame.write_out(stream)?;
            self.state = WebSocketState::Terminated;

            Err(Error::ConnectionClosed)
        } else {
            Ok(should_flush)
        }
    }

    /// Received a close frame. Tells if we need to return a close frame to the user.
    #[allow(clippy::option_option)]
    fn try_close(&mut self, close: Option<CloseFrame>) -> Option<Option<CloseFrame>> {
        match self.state {
            WebSocketState::Active => {
                self.state = WebSocketState::ClosedByPeer;

                let close = close.map(|frame| {
                    if !frame.code.allowed() {
                        CloseFrame {
                            code: CloseCode::Protocol,
                            reason: Utf8Bytes::from_static("Protocol violatoin")
                        }
                    } else {
                        frame
                    }
                });

                let reply = Frame::new_close(close.clone());
                self.set_additional(reply);

                Some(close)
            },
            WebSocketState::ClosedByPeer | WebSocketState::CloseAcknowledged => None,
            WebSocketState::ClosedByServer => {
                self.state = WebSocketState::CloseAcknowledged;
                Some(close)
            },
            WebSocketState::Terminated => unreachable!()
        }
    }

    /// Write a single frame into the write-buffer.
    fn buffer_frame<T>(&mut self, stream: &mut T, mut frame: Frame) -> Result<()>
    where 
        T: Read + Write
    {
        match self.mode {
            OperationMode::Server => {},
            OperationMode::Client => frame.set_random_mask(),
        }

        self.frame.write(stream, frame).check_connection_reset(self.state)
    }

    /// Replace `additional_send` if it is currently a `Pong` message.
    fn set_additional(&mut self, additional: Frame) {
        let empty_or_pong = self
            .additional_send
            .as_ref()
            .map_or(true, |f| f.header().opcode == OpCode::Control(Control::Pong));

        if empty_or_pong {
            self.additional_send.replace(additional);
        }
    }
}

fn check_max_size(size: usize, max: Option<usize>) -> Result<()> {
    if let Some(max) = max {
        if size > max {
            return Err(Error::Capacity(CapacityError::MessageTooLarge { size, max }));
        }
    }

    Ok(())
}

/// The current connection state.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum WebSocketState {
    /// The connection is active.
    Active,
    /// We initiated a close handshake.
    ClosedByServer,
    /// The peer initiated a close handshake.
    ClosedByPeer,
    /// The peer replied to our close handshake.
    CloseAcknowledged,
    /// The connection does not exist anymore.
    Terminated
}

impl WebSocketState {
    /// Tell if we're allowed to process normal messages.
    fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Tell if we should process incoming data. Note that if we send a close frame
    /// but the remote hasn't confirmed, they might have sent data before they receive our
    /// close frame, so we should still pass those to client code, hence ClosedByUs is valid.
    fn can_read(self) -> bool {
        matches!(self, Self::Active | Self::ClosedByServer)
    }

    /// Check if the state is active, return error if not.
    fn check_if_terminated(self) -> Result<()> {
        match self {
            WebSocketState::Terminated => Err(Error::AlreadyClosed),
            _ => Ok(())
        }
    }
}

/// Translate "Connection reset by peer" into `ConnectionClosed` if appropriate.
trait CheckConnectionReset {
    fn check_connection_reset(self, state: WebSocketState) -> Self;
}

impl<T> CheckConnectionReset for Result<T> {
    fn check_connection_reset(self, state: WebSocketState) -> Self {
        match self {
            Err(Error::Io(e)) => Err({
                if !state.can_read() && e.kind() == io::ErrorKind::ConnectionReset {
                    Error::ConnectionClosed
                } else {
                    Error::Io(e)
                }
            }),
            other => other
        }
    }
}