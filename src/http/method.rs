use std::str::FromStr;
use crate::http::error::HttpParseError;

#[derive(Debug)]
pub enum HttpMethod {
    POST,
    GET,
    PATCH,
    PUT,
    DELETE,
    OPTIONS
}

impl FromStr for HttpMethod {
    type Err = HttpParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "POST" => Ok(HttpMethod::POST),
            "GET" => Ok(HttpMethod::GET),
            "PATCH" => Ok(HttpMethod::PATCH),
            "PUT" => Ok(HttpMethod::PUT),
            "DELETE" => Ok(HttpMethod::DELETE),
            "OPTIONS" => Ok(HttpMethod::OPTIONS),
            other => Err(HttpParseError::InvalidMethod(other.to_string()))
        }
    }
}