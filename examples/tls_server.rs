use std::{net::TcpListener, sync::Arc, thread::spawn};

use blitz_ws::{
    accept_header,
    handshake::server::{Request, Response},
    stream::SimplifiedStream,
};
use native_tls_crate::TlsAcceptor;

fn main() {
    let identity =
        std::fs::read("path/to/some_identity.p12").expect("Failed to read some_identity.p12");
    let identity = native_tls_crate::Identity::from_pkcs12(&identity, "your-password")
        .expect("Failed to parse PKCS#12 identity");

    let tls_acceptor =
        Arc::new(TlsAcceptor::builder(identity).build().expect("Failed to build TLS acceptor"));

    let server = TcpListener::bind("0.0.0.0:8443").expect("Failed to bind to port 8443");

    for stream in server.incoming() {
        let stream = stream.expect("Failed to accept incoming stream");

        let acceptor = tls_acceptor.clone();

        spawn(move || {
            let tls_stream = acceptor.accept(stream).expect("TLS handshake failed");

            let cb = |req: &Request, mut res: Response| {
                println!("TLS WebSocket handshake");
                println!("Request URI: {}", req.uri().path());
                println!("The request's headers are:");
                for (header, _) in req.headers() {
                    println!("* {header}");
                }

                res.headers_mut().insert("X-TLS-Server", "blitz".parse().unwrap());

                Ok(res)
            };

            let mut ws = accept_header(SimplifiedStream::NativeTls(tls_stream), cb)
                .expect("WebSocket handshake failed");

            loop {
                let msg = ws.read().expect("Failed to read message");
                if msg.is_data() {
                    ws.send(msg).expect("Failed to write message");
                }
            }
        });
    }
}
