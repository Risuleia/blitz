#[derive(Debug)]
pub enum HttpParseError {
    InvalidMethod(String),
    InvalidRequest,
    MissingHeader(String)
}

impl std::fmt::Display for HttpParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpParseError::InvalidMethod(m) => write!(f, "Invalid HTTP method: {}", m),
            HttpParseError::InvalidRequest => write!(f, "Malformed HTTP Request"),
            HttpParseError::MissingHeader(h) => write!(f, "Missing required header: {}", h)
        }
    }
}

impl std::error::Error for HttpParseError {}