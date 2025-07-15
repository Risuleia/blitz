# Blitz

Lightweight stream-based WebSocket implementation for [Rust](https://www.rust-lang.org/), inspired from [Tungstenite](https://github.com/snapview/tungstenite-rs).

```rust
use std::net::TcpListener;
use std::thread::spawn;
use blitz::accept;

/// A WebSocket echo server
fn main () {
    let server = TcpListener::bind("0.0.0.0:8080").unwrap();
    for stream in server.incoming() {
        spawn (move || {
            let mut websocket = accept(stream.unwrap()).unwrap();
            loop {
                let msg = websocket.read().unwrap();

                // We do not want to send back ping/pong messages.
                if msg.is_data() {
                    websocket.send(msg).unwrap();
                }
            }
        });
    }
}
```

Take a look at the examples section to see how to write a simple client/server.

Blitz 
**NOTE:** `blitz` exposes primitives, not batteries. For a higher-level abstraction with full async support and production-ready integrations, consider using frameworks like tokio-tungstenite instead. [`tokio-tungstenite`](https://github.com/snapview/tokio-tungstenite).

[![MIT licensed](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE-MIT)
[![Apache-2.0 licensed](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)

[Documentation](https://docs.rs/blitz)

Introduction
------------
Blitz implements key parts of [RFC6455](https://tools.ietf.org/html/rfc6455), including:
- WebSocket framing, handshake, masking, and control frames
- RFC-compliant close codes and opcodes
- HTTP 1.1 request parsing (handshake only)
- TLS via native-tls or rustls
- Optional permessage-deflate compression (via flate2)

Features
--------

Blitz supports multiple optional TLS and utility features via Cargo:
* `native-tls`
* `native-tls-vendored`
* `rustls-tls-native-roots`
* `rustls-tls-webpki-roots`

Choose the one that is appropriate for your needs.

By default **no TLS feature is activated**, so make sure you use one of the TLS features,
otherwise you won't be able to communicate with the TLS endpoints.

Design Goals
------------
- Zero async: Fully synchronous, blocking API
- Explicit handshake logic and frame state machine
- No hidden abstractions, futures, or macros
- Easy to audit, debug, and extend

Example Projects
---------------
[examples/echo.rs](./examples.echo.rs): Simple WebSocket echo server <br/>
[examples/client.rs](./examples/client.rs): Basic WebSocket client using TcpStream <br/>
[examples/tls.rs](./examples/tls.rs): Connecting via TLS with native-tls or rustls

Compression Support
-------------------
Blitz supports **permessage-deflate** via the `flate2` crate and the `rust_backend` feature.
Compression is not enabled by default. You must explicitly negotiate it during the handshake if you want it.

Testing
-------
- Internal unit tests cover most framing and handshake logic.
- Autobahn Test Suite compatibility is being targeted (WIP).
- CI planned for protocol compliance and memory safety.

Contributing
------------
Blitz is experimental but aims for clarity and completeness. Contributions are welcome — especially:
- Autobahn Test Suite passes
- Advanced compression support
- More TLS options (e.g. ALPN)
- Streaming upgrade support

> Please file issues or PRs on GitHub.

License
------
Licensed under either of:
- MIT License
- Apache License, Version 2.0
at your option.

