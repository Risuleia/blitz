use std::{io::{self, Read, Write}, net::{TcpListener, TcpStream}, thread};

use blitz::http::request::HttpRequest;
use blitz::protocol::{websocket::WebSocketConnection, protocol::try_websocket_upgrade};

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080")
        .expect("Could not bind to address");

    println!("Listening on ws://0.0.0.0:8080");

    for stream in listener.incoming() {
        let stream = stream?;
        thread::spawn(move || {
            if let Err(e) = handle_connection(stream) {
                eprintln!("Error handling connection: {:?}", e);
            }
        });
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let raw_req = String::from_utf8_lossy(&buffer[..bytes_read]);

    let req = match HttpRequest::from_raw(&raw_req) {
        Ok(req) => req,
        Err(_) => return Ok(())
    };

    let is_ws_upgrade = req
        .headers
        .get("Upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    if is_ws_upgrade {
        let res = try_websocket_upgrade(&req).unwrap();
        stream.write_all(res.to_string().as_bytes())?;
        stream.flush()?;

        let mut ws_conn = WebSocketConnection::new(stream);
        ws_conn.run()?;
    } else {
        let body = "Hello from HTTP!";
        let res = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        stream.write_all(res.as_bytes())?;
        stream.flush()?;
    }

    Ok(())
}