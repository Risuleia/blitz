use std::collections::HashMap;

use crate::http::{error::HttpParseError, method::HttpMethod};

#[derive(Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>
}

impl HttpRequest {
    pub fn from_raw(raw: &str) -> Result<Self, HttpParseError> {
        let mut lines = raw.lines();

        let req_line = lines.next().ok_or(HttpParseError::InvalidRequest)?;
        let mut parts = req_line.split_whitespace();

        let method_str = parts.next().ok_or(HttpParseError::InvalidRequest)?;
        let path = parts.next().ok_or(HttpParseError::InvalidRequest)?;
        let version = parts.next().ok_or(HttpParseError::InvalidRequest)?;

        let method = method_str.parse::<HttpMethod>()?;

        let mut headers = HashMap::new();
        for line in lines.by_ref() {
            let line = line.trim();
            if line.is_empty() {
                break;
            }

            let mut kv = line.splitn(2, ':');
            let key = kv.next().ok_or(HttpParseError::InvalidRequest)?;
            let value = kv.next().ok_or(HttpParseError::InvalidRequest)?;

            headers.insert(key.trim().to_string(), value.trim().to_string());
        }

        let body = lines.collect::<Vec<&str>>().join("\n");
        let body = if body.trim().is_empty() { None } else { Some(body) };

        Ok(HttpRequest {
            method,
            path: path.to_string(),
            version: version.to_string(),
            headers,
            body
        })
    }
}