use blitz::{connect, protocol::message::Message};

fn main() {
    let (mut socket, resposne) = connect("ws://localhost:8080/socket").expect("Couldn't connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", resposne.status());
    println!("Response contains the following headers:");
    for (header, _) in resposne.headers() {
        println!("* {header}");
    }

    socket.write_message(Message::Text("Hello!".into())).unwrap();
    loop {
        let msg = socket.read_message().expect("Error reading message.");
        println!("Received: {msg}");
    }
}