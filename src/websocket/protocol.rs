use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};
use sha1::{Digest, Sha1};

use crate::{http::{request::HttpRequest, response::HttpResponse}, websocket::error::HandshakeError};

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub fn try_websocket_upgrade(req: &HttpRequest) -> Result<HttpResponse, HandshakeError> {
    let upgrade = req.headers.get("Upgrade");
    let connection = req.headers.get("Connection");

    let valid_upgrade = upgrade
        .map(|v| v.to_ascii_lowercase() == "websocket")
        .unwrap_or(false);

    let valid_connection = connection
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);

    if !valid_connection || !valid_upgrade {
        return Err(HandshakeError::MissingUpgradeHeaders);
    }

    match req.headers.get("Sec-WebSocket-Version") {
        Some(v) if v.trim() == "13" => {},
        _ => return Err(HandshakeError::InvalidVersion)
    }

    let sec_key = match req.headers.get("Sec-WebSocket-Key") {
        Some(key) => key.trim(),
        None => return Err(HandshakeError::MissingWebSocketKey)
    };

    let mut hasher = Sha1::new();
    hasher.update(sec_key.as_bytes());
    hasher.update(WS_GUID.as_bytes());

    let res = hasher.finalize();
    let accept_value = general_purpose::STANDARD.encode(res);

    let mut headers = HashMap::new();
    headers.insert("Upgrade".into(), "websocket".into());
    headers.insert("Connection".into(), "Upgrade".into());
    headers.insert("Sec-WebSocket-Accept".into(), accept_value);

    Ok(HttpResponse {
        status_code: 101,
        reason_phrase: "Switching protocols".into(),
        headers,
        body: None
    })
}