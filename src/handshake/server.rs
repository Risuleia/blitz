use std::{collections::HashMap, io::{self, Read, Write}};

use base64::{engine::general_purpose, Engine};
use sha1::{Digest, Sha1};

use crate::protocol::config::WebSocketConfig;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn parse_http_headers(stream: &mut impl Read) -> io::Result<HashMap<String, String>> {
    let mut buffer = [0u8; 4096];
    let mut total = 0;

    while total < buffer.len() {
        let n = stream.read(&mut buffer[total..])?;
        
        total += n;
        if buffer[..total].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    let req = std::str::from_utf8(&buffer[..total])
        .map_err(|e| io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut headers = HashMap::new();
    for line in req.lines().skip(1) {
        if let Some((k, v)) = line.split_once(":") {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    Ok(headers)
}

pub struct HandshakeResult {
    pub config: WebSocketConfig
}

pub fn respond_to_handshake<T: Read + Write>(
    stream: &mut T,
    config: &WebSocketConfig
) -> io::Result<HandshakeResult> {
    let headers = parse_http_headers(stream)?;

    let sec_websocket_key = headers.get("sec-websocket-key")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing Sec-WebSocket-Key"))?;

    let mut hasher = Sha1::new();
    hasher.update(sec_websocket_key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let res = hasher.finalize();
    let accept_key = general_purpose::STANDARD.encode(res);

    let mut negotiated_config = config.clone();
    let extensions_header = match negotiate_extensions(&headers, &config) {
        Some(ext_str) => {
            negotiated_config.compression.enabled = true;
            format!("Sec-WebSocket-Extensions: {}\r\n", ext_str)
        },
        None => String::new()
    };

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         {}\
         \r\n",
        accept_key,
        extensions_header
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(HandshakeResult { config: negotiated_config })
}

fn negotiate_extensions(
    req_headers: &HashMap<String, String>,
    config: &WebSocketConfig
) -> Option<String> {
    if !config.compression.enabled {
        return None;
    }

    if let Some(ext) = req_headers.get("sec-websocket-extensions") {
        if ext.contains("permessage-deflate") {
            let mut response = "permessage-deflate".to_string();

            if config.compression.server_no_context_takeover {
                response.push_str("; server_no_context_takeover");
            }
            if config.compression.client_no_context_takeover {
                response.push_str("; client_no_context_takeover");
            }
            if let Some(bits) = config.compression.server_max_window_bits {
                response.push_str(&format!("; server_max_window_bits={}", bits));
            }
            if let Some(bits) = config.compression.client_max_window_bits {
                response.push_str(&format!("; client_max_window_bits={}", bits));
            }

            return Some(response);
        }
    }

    None
}