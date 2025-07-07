//! Server handshake machine

use http::{HeaderMap, Method, Request as HttpRequest, Response as HttpResponse, StatusCode, Version};
use httparse::{Status, EMPTY_HEADER};
use std::{io::{Read, Write}, marker::PhantomData, result::Result as StdResult};

use crate::{error::{Error, ProtocolError, Result}, handshake::{core::{derive_accept_key, HandshakeRole, MidHandshake, ProcessingResult}, headers::{FromHttparse, MAX_HEADERS}, machine::{HandshakeMachine, StageResult, TryParse}}, protocol::{config::WebSocketConfig, websocket::{OperationMode, WebSocket}}};

/// Server Request type
pub type Request = HttpRequest<()>;
/// Server Response type
pub type Response = HttpResponse<()>;
/// Server Error Response type
pub type ErrorResponse = HttpResponse<Option<String>>;

fn create_parts<T>(req: &HttpRequest<T>) -> Result<http::response::Builder> {
    if req.method() != Method::GET {
        return Err(Error::Protocol(ProtocolError::InvalidHttpMethod));
    }

    if req.version() < Version::HTTP_11 {
        return Err(Error::Protocol(ProtocolError::InvalidHttpVersion));
    }

    let headers = req.headers();

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
        .get("Sec-WebSocket-Version")
        .map(|h| h == "13")
        .unwrap_or(false)
    {
        return Err(Error::Protocol(ProtocolError::MissingVersionHeader));
    }

    let key = headers
        .get("Sec-WebSocket-Key")
        .ok_or(Error::Protocol(ProtocolError::MissingKeyHeader))?;

    let builder = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .version(req.version())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Accept", derive_accept_key(key.as_bytes()));

    Ok(builder)
}

/// Creates a response for the request
pub fn create_response(req: &Request) -> Result<Response> {
    Ok(create_parts(req)?.body(())?)
}

/// Creates a response for the request with a custom body
pub fn create_response_with_body<T1, T2>(
    req: &HttpRequest<T1>,
    generate_body: impl FnOnce() -> T2
) -> Result<HttpResponse<T2>> {
    Ok(create_parts(req)?.body(generate_body())?)
}

/// Writes `response` to the stream `w`
pub fn write_response<T>(mut w: impl Write, res: &HttpResponse<T>) -> Result<()> {
    writeln!(w, "{:?} {}\r", res.version(), res.status())?;
    for (k, v) in res.headers() {
        writeln!(w, "{}: {}\r", k, v.to_str()?)?;
    }
    writeln!(w, "\r")?;

    Ok(())
}

impl TryParse for Request {
    fn try_parse(data: &[u8]) -> Result<Option<(usize, Self)>> {
        let mut header_buf = [EMPTY_HEADER; MAX_HEADERS];
        let mut req = httparse::Request::new(&mut header_buf);

        Ok(match req.parse(data)? {
            Status::Complete(n) => Some((n, Request::from_httparse(req)?)),
            Status::Partial => None
        })
    }
}

impl<'b: 'h, 'h> FromHttparse<httparse::Request<'h, 'b>> for Request {
    fn from_httparse(raw: httparse::Request<'h, 'b>) -> Result<Self> {
        if raw.method != Some("GET") {
            return Err(Error::Protocol(ProtocolError::InvalidHttpMethod));
        }

        if raw.version != Some(1) {
            return Err(Error::Protocol(ProtocolError::InvalidHttpVersion));
        }

        let mut req = Request::new(());
        *req.method_mut() = Method::GET;
        *req.uri_mut() = raw.path.expect("Bug: no path in header").parse()?;
        *req.version_mut() = Version::HTTP_11;
        *req.headers_mut() = HeaderMap::from_httparse(raw.headers)?;

        Ok(req)
    }
}

/// Callback trait
/// 
/// The callback is called when the server receives an incoming WebSocket
/// handshake request from the client. Specifying a callback allows you to analyze incoming headers
/// and add additional headers to the response that the server sends to the client and / or reject the
/// connection based on the incoming headers.
pub trait Callback: Sized {
    /// Called whenever the server reads the request from the client and is ready to respond to it.
    /// May return additional reply headers.
    /// Returning an error resulting in rejecting the incoming connection.
    fn on_request(self, req: &Request, res: Response) -> StdResult<Response, ErrorResponse>;
}

impl<F> Callback for F
where
    F: FnOnce(&Request, Response) -> StdResult<Response, ErrorResponse>
{
    fn on_request(self, req: &Request, res: Response) -> StdResult<Response, ErrorResponse> {
        self(req, res)
    }
}

/// Stub for an empty callback
#[derive(Clone, Copy, Debug)]
pub struct NoCallback;

impl Callback for NoCallback {
    fn on_request(self, _req: &Request, res: Response) -> StdResult<Response, ErrorResponse> {
        Ok(res)
    }
}

/// Server handshake role
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct ServerHandshake<S, C> {
    /// Callback which is called whenever the server read the request from the client and is ready
    /// to reply to it. The callback returns an optional headers which will be added to the reply
    /// which the server sends to the user.
    callback: Option<C>,
    /// WebSocket configuration.
    config: Option<WebSocketConfig>,
    /// Error code/flag. If set, an error will be returned after sending response to the client.
    error_response: Option<ErrorResponse>,
    /// Internal stream type.
    _marker: PhantomData<S>,
}

impl<S: Read + Write, C: Callback> ServerHandshake<S, C> {
    /// Start server handshake. `callback` specifies a custom callback which the user can pass to
    /// the handshake, this callback will be called when the a websocket client connects to the
    /// server, you can specify the callback if you want to add additional header to the client
    /// upon join based on the incoming headers.
    pub fn start(stream: S, callback: C, config: Option<WebSocketConfig>) -> MidHandshake<Self> {
        MidHandshake {
            machine: HandshakeMachine::start_read(stream),
            role: ServerHandshake {
                callback: Some(callback),
                config,
                error_response: None,
                _marker: PhantomData
            }
        }
    }
}

impl<S: Read + Write, C: Callback> HandshakeRole for ServerHandshake<S, C> {
    type IncomingData = Request;
    type InternalStream = S;
    type FinalResult = WebSocket<S>;

    fn stage_finished(
            &mut self,
            finish: StageResult<Self::IncomingData, Self::InternalStream>
        ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>> {
        match finish {
            StageResult::DoneReading { result, stream , tail } => {
                if !tail.is_empty() {
                    return Err(Error::Protocol(ProtocolError::JunkAfterRequest));
                }

                let response = create_response(&result)?;
                let callback_result = if let Some(callback) = self.callback.take() {
                    callback.on_request(&result, response)
                } else {
                    Ok(response)
                };

                match callback_result {
                    Ok(resp) => {
                        let mut output = vec![];
                        write_response(&mut output, &resp)?;
                        
                        Ok(ProcessingResult::Continue(HandshakeMachine::start_write(stream, output)))
                    },
                    Err(resp) => {
                        if resp.status().is_success() {
                            return Err(Error::Protocol(ProtocolError::CustomResponseSuccessful));
                        }

                        self.error_response = Some(resp);
                        let resp_ref = self.error_response.as_ref().unwrap();

                        let mut output = vec![];
                        write_response(&mut output, resp_ref)?;

                        if let Some(body) = resp_ref.body() {
                            output.extend_from_slice(body.as_bytes());
                        }

                        Ok(ProcessingResult::Continue(HandshakeMachine::start_write(stream, output)))
                    }
                }
            },
            StageResult::DoneWriting(stream) => {
                if let Some(err) = self.error_response.take() {
                    let (parts, body) = err.into_parts();
                    return Err(Error::Http(HttpResponse::from_parts(parts, body.map(|s| s.into_bytes()))));
                }
                
                Ok(ProcessingResult::Done(WebSocket::new(stream, OperationMode::Server, self.config)))
            }
        }
    }
}