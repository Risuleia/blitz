//! WebSocket Message handler

use bytes::Bytes;

use crate::{
    error::{CapacityError, Error, Result},
    protocol::{
        frame::{CloseFrame, Frame, Utf8Bytes},
        message::string_lib::StringCollector,
    },
};

mod string_lib {
    use crate::error::{Error, Result};
    use utf8::DecodeError;

    #[derive(Debug)]
    pub struct StringCollector {
        data: String,
        incomplete: Option<utf8::Incomplete>,
    }

    impl StringCollector {
        pub fn new() -> Self {
            StringCollector { data: String::new(), incomplete: None }
        }

        pub fn len(&self) -> usize {
            self.data
                .len()
                .saturating_add(self.incomplete.map(|i| i.buffer_len as usize).unwrap_or(0))
        }

        pub fn extend<T: AsRef<[u8]>>(&mut self, tail: T) -> Result<()> {
            let mut input: &[u8] = tail.as_ref();

            if let Some(mut incomplete) = self.incomplete.take() {
                if let Some((result, remaining)) = incomplete.try_complete(input) {
                    input = remaining;

                    match result {
                        Ok(s) => self.data.push_str(s),
                        Err(result_bytes) => {
                            return Err(Error::Utf8(String::from_utf8_lossy(result_bytes).into()))
                        }
                    }
                } else {
                    input = &[];
                    self.incomplete = Some(incomplete);
                }
            }

            if !input.is_empty() {
                match utf8::decode(input) {
                    Ok(s) => {
                        self.data.push_str(s);
                        Ok(())
                    }
                    Err(DecodeError::Incomplete { valid_prefix, incomplete_suffix }) => {
                        self.data.push_str(valid_prefix);
                        self.incomplete = Some(incomplete_suffix);

                        Ok(())
                    }
                    Err(DecodeError::Invalid { valid_prefix, invalid_sequence, .. }) => {
                        self.data.push_str(valid_prefix);

                        Err(Error::Utf8(String::from_utf8_lossy(invalid_sequence).into()))
                    }
                }
            } else {
                Ok(())
            }
        }

        pub fn into_string(self) -> Result<String> {
            if let Some(incomplete) = self.incomplete {
                Err(Error::Utf8(format!("Incomplete string: {:?}", incomplete)))
            } else {
                Ok(self.data)
            }
        }
    }
}

/// A struct representing the incomplete message.
#[derive(Debug)]
pub struct IncompleteMessage {
    collector: IncompleteMessageCollector,
}

#[derive(Debug)]
enum IncompleteMessageCollector {
    Text(StringCollector),
    Binary(Vec<u8>),
}

/// The type of incomplete message.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub enum IncompleteMessageType {
    /// Text type
    Text,
    /// Binary type
    Binary,
}

impl IncompleteMessage {
    /// Create new.
    pub fn new(msg_type: IncompleteMessageType) -> Self {
        IncompleteMessage {
            collector: match msg_type {
                IncompleteMessageType::Binary => IncompleteMessageCollector::Binary(Vec::new()),
                IncompleteMessageType::Text => {
                    IncompleteMessageCollector::Text(StringCollector::new())
                }
            },
        }
    }

    /// Get the current filled size of the buffer.
    pub fn len(&self) -> usize {
        match self.collector {
            IncompleteMessageCollector::Binary(ref b) => b.len(),
            IncompleteMessageCollector::Text(ref t) => t.len(),
        }
    }

    /// Checks if the incomplete message is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Add more data to an existing message.
    pub fn extend<T: AsRef<[u8]>>(&mut self, tail: T, limit: Option<usize>) -> Result<()> {
        let max = limit.unwrap_or(usize::MAX);
        let size = self.len();
        let portion = tail.as_ref().len();

        if size > max || portion > max - size {
            return Err(Error::Capacity(CapacityError::MessageTooLarge {
                size: size + portion,
                max,
            }));
        }

        match self.collector {
            IncompleteMessageCollector::Binary(ref mut b) => {
                b.extend(tail.as_ref());
                Ok(())
            }
            IncompleteMessageCollector::Text(ref mut t) => t.extend(tail),
        }
    }

    /// Convert an incomplete message into a complete one.
    pub fn complete(self) -> Result<Message> {
        match self.collector {
            IncompleteMessageCollector::Binary(b) => Ok(Message::Binary(b.into())),
            IncompleteMessageCollector::Text(t) => {
                let text = t.into_string()?;
                Ok(Message::Text(text.into()))
            }
        }
    }
}

/// A WebSocket message
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// A text message
    Text(Utf8Bytes),
    /// A binary message
    Binary(Bytes),
    /// A ping (control) message
    Ping(Bytes),
    /// A pong (control) message
    Pong(Bytes),
    /// A close (control) message
    Close(Option<CloseFrame>),
    /// Raw frame
    Frame(Frame),
}

impl Message {
    /// Create a new text WebSocket message from a stringable.
    pub fn new_text<S>(string: S) -> Message
    where
        S: Into<Utf8Bytes>,
    {
        Message::Text(string.into())
    }

    /// Create a new binary WebSocket message by converting to `Bytes`.
    pub fn new_binary<B>(binary: B) -> Message
    where
        B: Into<Bytes>,
    {
        Message::Binary(binary.into())
    }

    /// Indicates if the Message is of control protocol (`Ping`, `Pong`, `Close`)
    pub fn is_control(&self) -> bool {
        matches!(self, Message::Ping(_) | Message::Pong(_) | Message::Close(_))
    }

    /// Indicates if the Message is of data protocol (`Text`, `Binary`)
    pub fn is_data(&self) -> bool {
        matches!(self, Message::Text(_) | Message::Binary(_))
    }

    /// Indicates if the Message is of `Text` protocol
    pub fn is_text(&self) -> bool {
        matches!(self, Message::Text(_))
    }

    /// Indicates if the Message is of `Binary` protocol
    pub fn is_binary(&self) -> bool {
        matches!(self, Message::Binary(_))
    }

    /// Get the length of the WebSocket message.
    pub fn len(&self) -> usize {
        match *self {
            Message::Text(ref s) => s.len(),
            Message::Binary(ref b) | Message::Ping(ref b) | Message::Pong(ref b) => b.len(),
            Message::Close(ref frame) => frame.as_ref().map(|d| d.reason.len()).unwrap_or(0),
            Message::Frame(ref frame) => frame.len(),
        }
    }

    /// Returns true if the WebSocket message has no content.
    /// For example, if the other side of the connection sent an empty string.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Parses the message data
    pub fn into_data(self) -> Bytes {
        match self {
            Self::Text(s) => s.into(),
            Self::Binary(b) | Self::Ping(b) | Self::Pong(b) => b,
            Self::Close(None) => <_>::default(),
            Self::Close(Some(frame)) => frame.reason.into(),
            Self::Frame(frame) => frame.into_payload(),
        }
    }
}

impl From<String> for Message {
    #[inline]
    fn from(value: String) -> Self {
        Message::new_text(value)
    }
}

impl<'s> From<&'s str> for Message {
    #[inline]
    fn from(value: &'s str) -> Self {
        Message::new_text(value)
    }
}

impl<'b> From<&'b [u8]> for Message {
    #[inline]
    fn from(value: &'b [u8]) -> Self {
        Message::new_binary(Bytes::copy_from_slice(value))
    }
}

impl From<Bytes> for Message {
    fn from(value: Bytes) -> Self {
        Message::new_binary(value)
    }
}

impl From<Vec<u8>> for Message {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        Message::new_binary(value)
    }
}

impl From<Message> for Bytes {
    #[inline]
    fn from(value: Message) -> Self {
        value.into_data()
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Text(s) => write!(f, "Text({})", s),
            Message::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
            Message::Ping(_) => write!(f, "Ping"),
            Message::Pong(_) => write!(f, "Pong"),
            Message::Close(Some(frame)) => write!(f, "Close({}, {})", frame.code, frame.reason),
            Message::Close(None) => write!(f, "Close"),
            _ => Ok(()),
        }
    }
}
