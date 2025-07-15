use blitz_ws::{connect, protocol::message::Message};

fn main() {
    let (mut socket, response) = connect("ws://localhost:8080/socket").expect("Couldn't connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (header, _) in response.headers() {
        println!("* {header}");
    }

    socket.write(Message::Text("Hello!".into())).unwrap();
    loop {
        let msg = socket.read().expect("Error reading message.");
        println!("Received: {msg}");
    }
}
