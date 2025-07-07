//! Codes defined in RFC 6455

use std::fmt::Display;

/// WebSocket message opcode as in RFC 6455.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OpCode {
    /// Data (text or binary).
    Data(Data),
    /// Control (close, ping, pong).
    Control(Control)
}

/// Data opcodes as in RFC 6455
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Data {
    /// A continuation frame
    Continuation = 0x0,
    /// A text frame
    Text = 0x1,
    /// A binary frame
    Binary = 0x2,
    /// 0xb-f are reserved for further control frames
    Reserved(u8)
}

/// Control opcodes as in RFC 6455
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Control {
    /// A close frame
    Close = 0x8,
    /// A ping frame
    Ping = 0x9,
    /// A pong frame
    Pong = 0xA,
    /// 0xb-f are reserved for further control frames
    Reserved(u8)
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Continuation => write!(f, "CONTINUE"),
            Self::Text => write!(f, "TEXT"),
            Self::Binary => write!(f, "BINARY"),
            Self::Reserved(other) => write!(f, "RESERVED_DATA_{other}'") 
        }
    }
}

impl Display for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Close => write!(f, "CLOSE"),
            Self::Ping => write!(f, "PING"),
            Self::Pong => write!(f, "PONG"),
            Self::Reserved(other) => write!(f, "RESERVED_CONTROL_{other}")
        }
    }
}

impl Display for OpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Data(d) => d.fmt(f),
            Self::Control(c) => c.fmt(f)
        }
    }
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        match value {
            self::OpCode::Data(self::Data::Continuation) => 0x0,
            self::OpCode::Data(self::Data::Text) => 0x1,
            self::OpCode::Data(self::Data::Binary) => 0x2,
            self::OpCode::Data(self::Data::Reserved(i)) => i,

            self::OpCode::Control(self::Control::Close) => 0x8,
            self::OpCode::Control(self::Control::Ping) => 0x9,
            self::OpCode::Control(self::Control::Pong) => 0xA,
            self::OpCode::Control(self::Control::Reserved(i)) => i,
        }
    }
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        match value {
            0x0 => Self::Data(Data::Continuation),
            0x1 => Self::Data(Data::Text),
            0x2 => Self::Data(Data::Binary),
            i @ 0x3..=0x7 => Self::Data(Data::Reserved(i)),
            0x8 => Self::Control(Control::Close),
            0x9 => Self::Control(Control::Ping),
            0xA => Self::Control(Control::Pong),
            i @ 0xB..=0xF => Self::Control(Control::Reserved(i)),
            _ => panic!("Bug: OpCode out of range")
        }
    }
}

/// Status code used to indicate why an endpoint is closing the WebSocket connection.
#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CloseCode {
    /// Indicates a normal closure, meaning that the purpose for
    /// which the connection was established has been fulfilled.
    Normal = 0x3e8,
    
    /// Indicates that an endpoint is "going away", such as a server
    /// going down or a browser having navigated away from a page.
    Away = 0x3e9,

    /// Indicates that an endpoint is terminating the connection due
    /// to a protocol error.
    Protocol = 0x3EA,

    /// Indicates that an endpoint is terminating the connection
    /// because it has received a type of data it cannot accept (e.g., an
    /// endpoint that understands only text data MAY send this if it
    /// receives a binary message).
    Unsupported = 0x3EB,

    /// Indicates that no status code was included in a closing frame. This
    /// close code makes it possible to use a single method, `on_close` to
    /// handle even cases where no close code was provided.
    Status = 0x3ED,

    /// Indicates an abnormal closure. If the abnormal closure was due to an
    /// error, this close code will not be used. Instead, the `on_error` method
    /// of the handler will be called with the error. However, if the connection
    /// is simply dropped, without an error, this close code will be sent to the
    /// handler.
    Abnormal = 0x3EE,

    /// Indicates that an endpoint is terminating the connection
    /// because it has received data within a message that was not
    /// consistent with the type of the message (e.g., non-UTF-8 \[RFC3629\]
    /// data within a text message).
    Invalid = 0x3EF,

    /// Indicates that an endpoint is terminating the connection
    /// because it has received a message that violates its policy.  This
    /// is a generic status code that can be returned when there is no
    /// other more suitable status code (e.g., Unsupported or Size) or if there
    /// is a need to hide specific details about the policy.
    Policy = 0x3F0,

    /// Indicates that an endpoint is terminating the connection
    /// because it has received a message that is too big for it to
    /// process.
    Size = 0x3F1,

    /// Indicates that an endpoint (client) is terminating the
    /// connection because it has expected the server to negotiate one or
    /// more extension, but the server didn't return them in the response
    /// message of the WebSocket handshake.  The list of extensions that
    /// are needed should be given as the reason for closing.
    /// Note that this status code is not used by the server, because it
    /// can fail the WebSocket handshake instead.
    Extension = 0x3F2,

    /// Indicates that a server is terminating the connection because
    /// it encountered an unexpected condition that prevented it from
    /// fulfilling the request.
    Error = 0x3F3,

    /// Indicates that the server is restarting. A client may choose to reconnect,
    /// and if it does, it should use a randomized delay of 5-30 seconds between attempts.
    Restart = 0x3F4,

    /// Indicates that the server is overloaded and the client should either connect
    /// to a different IP (when multiple targets exist), or reconnect to the same IP
    /// when a user has performed an action.
    Again = 0x3F5,

    #[doc(hidden)]
    Tls = 0x3F7,

    #[doc(hidden)]
    Reserved(u16),

    #[doc(hidden)]
    Iana(u16),

    #[doc(hidden)]
    Library(u16),

    #[doc(hidden)]
    Bad(u16)
}

impl CloseCode {
    /// Check if this CloseCode is allowed.
    pub fn allowed(self) -> bool {
        !matches!(self, Self::Bad(_) | Self::Reserved(_) | Self::Status | Self::Abnormal | Self::Tls)
    }
}

impl Display for CloseCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code: u16 = self.into();
        write!(f, "{code}")
    }
}

impl From<CloseCode> for u16 {
    fn from(value: CloseCode) -> u16 {
        match value {
            self::CloseCode::Normal => 0x3e8,
            self::CloseCode::Away => 0x3e9,
            self::CloseCode::Protocol => 0x3EA,
            self::CloseCode::Unsupported => 0x3EB,
            self::CloseCode::Status => 0x3ED,
            self::CloseCode::Abnormal => 0x3EE,
            self::CloseCode::Invalid => 0x3EF,
            self::CloseCode::Policy => 0x3F0,
            self::CloseCode::Size => 0x3F1,
            self::CloseCode::Extension => 0x3F2,
            self::CloseCode::Error => 0x3F3,
            self::CloseCode::Restart => 0x3F4,
            self::CloseCode::Again => 0x3F5,
            self::CloseCode::Tls => 0x3F7,
            self::CloseCode::Bad(other) => other,
            self::CloseCode::Reserved(other) => other,
            self::CloseCode::Iana(other) => other,
            self::CloseCode::Library(other) => other,
        }
    }
}

impl<'t> From<&'t CloseCode> for u16 {
    fn from(value: &'t CloseCode) -> Self {
        value.into()
    }
}

impl From<u16> for CloseCode {
    fn from(value: u16) -> Self {
        match value {
            0x3e8 => Self::Normal,
            0x3e9 => Self::Away,
            0x3EA => Self::Protocol,
            0x3EB => Self::Unsupported,
            0x3ED => Self::Status,
            0x3EE => Self::Abnormal,
            0x3EF => Self::Invalid,
            0x3F0 => Self::Policy,
            0x3F1 => Self::Size,
            0x3F2 => Self::Extension,
            0x3F3 => Self::Error,
            0x3F4 => Self::Restart,
            0x3F5 => Self::Again,
            0x3F7 => Self::Tls,
            0x1..=0x3E7 => Self::Bad(value),
            0x3F8..=0xBB7 => Self::Reserved(value),
            0xBB8..=0xF9F => Self::Iana(value),
            0xFA0..=0x1387 => Self::Library(value),
            _ => Self::Bad(value),
        }
    }
}


