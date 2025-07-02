#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<(u16, String)>)
}

impl Message {
    pub fn is_control(&self) -> bool {
        matches!(self, Message::Ping(_) | Message::Pong(_) | Message::Close(_))
    }

    pub fn into_data(self) -> Vec<u8> {
        match self {
            Self::Text(s) => s.into_bytes(),
            Self::Binary(b) => b,
            Self::Ping(b) => b,
            Self::Pong(b) => b,
            Self::Close(Some((code, reason))) => {
                let mut buf = Vec::with_capacity(2 + reason.len());
                buf.extend_from_slice(&code.to_be_bytes());
                buf.extend_from_slice(reason.as_bytes());
                buf
            },
            Self::Close(None) => vec![]
        }
    }

    pub fn from_close_payload(payload: Vec<u8>) -> Self {
        if payload.len() >= 2 {
            let code = u16::from_be_bytes([payload[0], payload[1]]);
            let reason = String::from_utf8_lossy(&payload[2..]).into_owned();
            Message::Close(Some((code, reason)))
        } else {
            Message::Close(None)
        }
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Text(s) => write!(f, "Text({})", s),
            Message::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
            Message::Ping(_) => write!(f, "Ping"),
            Message::Pong(_) => write!(f, "Pong"),
            Message::Close(Some((code, reason))) => write!(f, "Close({}, {})", code, reason),
            Message::Close(None) => write!(f, "Close")
        }
    }
}