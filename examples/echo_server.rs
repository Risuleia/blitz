use std::{net::TcpListener, thread::spawn};

use blitz::{accept_header, handshake::server::{Request, Response}};

fn main() {
    let server = TcpListener::bind("0.0.0.0:8080").unwrap();

    for stream in server.incoming() {
        let cb = |req: &Request, mut res: Response| {
            println!("Received a new WebSocket handshake!");
            println!("The request's path is: {}", req.uri().path());
            println!("The request's headers are:");
            for (header, _) in req.headers() {
                println!("* {header}");
            }

            let headers = res.headers_mut();
            headers.append("Some-Header-1", "Some-Value-2".parse().unwrap());
            headers.append("Some-Header-2", "Some-Value-2".parse().unwrap());

            Ok(res)
        };

        spawn(move || {
            let mut ws = accept_header(stream.unwrap(), cb).expect("Handshake failed");
    
            loop {
                let msg = ws.read_message().expect("Failed to read message");
                if !msg.is_data() {
                    ws.write_message(msg).expect("Failed to send message");
                }
            }
        });
    }
}