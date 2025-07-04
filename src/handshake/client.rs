//! Client handshake machine

use std::{io::{Read, Write}, marker::PhantomData};

use base64::Engine;
use http::{HeaderMap, HeaderName, Method, Request as HttpRequest, Response as HttpResponse, StatusCode, Version};
use httparse::{Status, EMPTY_HEADER};

use crate::{error::{Error, ProtocolError, Result, SubProtocolError, UrlError}, handshake::{core::{derive_accept_key, HandshakeRole, MidHandshake, ProcessingResult}, headers::{FromHttparse, MAX_HEADERS}, machine::{HandshakeMachine, StageResult, TryParse}}, protocol::{config::WebSocketConfig, websocket::{OperationMode, WebSocket}}};

/// Client Request type
pub type Request = HttpRequest<()>;
/// Client Response Type
pub type Response = HttpResponse<Option<Vec<u8>>>;

/// Client handshake
#[derive(Debug)]
pub struct ClientHandshake<S> {
    verify_data: VerifyData,
    config: Option<WebSocketConfig>,
    _marker: PhantomData<S>
}

impl<S: Read + Write> ClientHandshake<S> {
    /// Initiate a client handshake
    pub fn start(stream: S, req: Request, config: Option<WebSocketConfig>) -> Result<MidHandshake<Self>> {
        if req.method() != Method::GET {
            return Err(Error::Protocol(ProtocolError::InvalidHttpMethod))
        }
        if req.version() < Version::HTTP_11 {
            return Err(Error::Protocol(ProtocolError::InvalidHttpVersion));
        }

        let subprotocols = extract_subprotocols(&req)?;

        let (request, key) = generate_request(req)?;

        let machine = HandshakeMachine::start_write(stream, request);

        let client = {
            let accept_key = derive_accept_key(key.as_ref());
            ClientHandshake {
                verify_data: VerifyData { accept_key, subprotocols },
                config,
                _marker: PhantomData
            }
        };

        Ok(MidHandshake { role: client, machine })
    }
}

impl<S: Read + Write> HandshakeRole for ClientHandshake<S> {
    type IncomingData = Response;
    type InternalStream = S;
    type FinalResult = (WebSocket<S>, Response);

    fn stage_finished(
            &mut self,
            finish: StageResult<Self::IncomingData, Self::InternalStream>
        ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>> {
        Ok(match finish {
            StageResult::DoneWriting(stream) => {
                ProcessingResult::Continue(HandshakeMachine::start_read(stream))
            },
            StageResult::DoneReading { result,   stream, tail } => {
                let res = match self.verify_data.verify_response(result) {
                    Ok(r) => r,
                    Err(Error::Http(mut e)) => {
                        *e.body_mut() = Some(tail);
                        return Err(Error::Http(e));
                    },
                    Err(e) => return Err(e)
                };

                let websocket = WebSocket::with_config(stream, OperationMode::Client, self.config.take().unwrap_or(WebSocketConfig::default()));
                ProcessingResult::Done((websocket, res))
            }
        })
    }
}

/// Verifies and generates a client WebSocket request from a raw request and extracts a WebSocket key from it
pub fn generate_request(mut request: Request) -> Result<(Vec<u8>, String)> {
    let mut req = Vec::new();
    write!(
        req,
        "GET {path} {version:?}\r\n",
        path = request.uri().path_and_query().ok_or(Error::Url(UrlError::NoPathOrQuery))?.as_str(),
        version = request.version()
    ).unwrap();

    const KEY_HEADERNAME: &str = "Sec-WebSocket-Key";
    const WEBSOCKET_HEADERS: [&str; 5] = ["Host", "Connection", "Upgrade", "Sec-WebSocket-Version", KEY_HEADERNAME];

    let key = request
        .headers()
        .get(KEY_HEADERNAME)
        .ok_or_else(|| {
            Error::Protocol(ProtocolError::InvalidHeader(
                HeaderName::from_bytes(KEY_HEADERNAME.as_bytes()).unwrap()
            ))
        })?
        .to_str()?
        .to_owned();

    let headers = request.headers_mut();
    for &header in &WEBSOCKET_HEADERS {
        let val = headers.remove(header).ok_or_else(|| {
            Error::Protocol(ProtocolError::InvalidHeader(
                HeaderName::from_bytes(header.as_bytes()).unwrap()
            ))
        })?;

        write!(
            req,
            "{header}: {value}\r\n",
            header = header,
            value = val.to_str().map_err(|e| {
                Error::Utf8(format!("{e} for header name '{header}' with value: {val:?}"))
            })?
        ).unwrap();
    }

    let insensitive: Vec<String> = WEBSOCKET_HEADERS.iter().map(|h| h.to_ascii_lowercase()).collect();
    for (k, v) in headers {
        let mut name = k.as_str();

        if insensitive.iter().any(|h| h == name) {
            return Err(Error::Protocol(ProtocolError::InvalidHeader(k.clone())));
        }

        if name == "sec-websocket-protocol" {
            name = "Sec-WebSocket-Protocol";
        }
        if name == "origin" {
            name = "Origin";
        }

        writeln!(
            req,
            "{}: {}\r",
            name,
            v.to_str().map_err(|e| Error::Utf8(format!("{e} for header name '{name}' with value: {v:?}")))?
        ).unwrap();

    }

    writeln!(req, "\r").unwrap();
    Ok((req, key))
}

fn extract_subprotocols(req: &Request) -> Result<Option<Vec<String>>> {
    if let Some(subprotocols) = req.headers().get("Sec-WebSocket-Protocol") {
        Ok(Some(subprotocols.to_str()?.split(',').map(|s| s.trim().to_string()).collect()))
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
struct VerifyData {
    accept_key: String,
    subprotocols: Option<Vec<String>>
}

impl VerifyData {
    pub fn verify_response(&self, res: Response) -> Result<Response> {
        if res.status() != StatusCode::SWITCHING_PROTOCOLS {
            return Err(Error::Http(res));
        }

        let headers = res.headers();

        if !headers
            .get("Connection")
            .and_then(|h| h.to_str().ok())
            .map(|v| v.split(|c| c == ',' || c == ' ').any(|s| s.eq_ignore_ascii_case("Upgrade")))
            .unwrap_or(false)
        {
            return Err(Error::Protocol(ProtocolError::MissingConnectionUpgradeHeader));
        }

        if !headers
            .get("Upgrade")
            .and_then(|h| h.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
        {
            return Err(Error::Protocol(ProtocolError::MissingUpgradeHeader));
        }

        if !headers
            .get("Sec-WebSocket-Accept")
            .map(|h| h == &self.accept_key)
            .unwrap_or(false)
        {
            return Err(Error::Protocol(ProtocolError::AcceptKeyMismatch));
        }

        if headers.get("Sec-WebSocket-Protocol").is_none() && self.subprotocols.is_some() {
            return Err(Error::Protocol(ProtocolError::SecWebSocketSubProtocolError(SubProtocolError::NoSubProtocol)));
        }
        if headers.get("Sec-Websocket-Protocol").is_some() && self.subprotocols.is_none() {
            return Err(Error::Protocol(ProtocolError::SecWebSocketSubProtocolError(SubProtocolError::ServerSentSubProtocolNoneRequested)));
        }
        if let Some(returned_subprotocol) = headers.get("Sec-WebSocket-Protocol") {
            if let Some(accepted_subprotocols) = &self.subprotocols {
                if !accepted_subprotocols.contains(&returned_subprotocol.to_str()?.to_string()) {
                    return Err(Error::Protocol(ProtocolError::SecWebSocketSubProtocolError(SubProtocolError::InvalidSubProtocol)));
                }
            }
        }

        Ok(res)
    }
}

impl TryParse for Response {
    fn try_parse(data: &[u8]) -> crate::error::Result<Option<(usize, Self)>> {
        let mut hbuffer = [EMPTY_HEADER; MAX_HEADERS];
        let mut req = httparse::Response::new(&mut hbuffer);

        Ok(match req.parse(data)? {
            Status::Partial => None,
            Status::Complete(n) => Some((n, Response::from_httparse(req)?))
        })
    }
}

impl<'b: 'h, 'h> FromHttparse<httparse::Response<'h, 'b>> for Response {
    fn from_httparse(raw: httparse::Response<'h, 'b>) -> crate::error::Result<Self> {
        if raw.version != Some(1) {
            return Err(Error::Protocol(ProtocolError::InvalidHttpVersion));
        }

        let headers = HeaderMap::from_httparse(raw.headers)?;

        let mut res = Response::new(None);
        *res.status_mut() = StatusCode::from_u16(raw.code.expect("Bug: no HTTP status code"))?;
        *res.headers_mut() = headers;
        *res.version_mut() = Version::HTTP_11;

        Ok(res)
    }
}

/// Generates a random accept key for the `Sec-WebSocket-Key` header
pub fn generate_key() -> String {
    let r: [u8; 16] = rand::random();
    base64::engine::general_purpose::STANDARD.encode(r)
}