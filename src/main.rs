use std::io::{self, Read, Write};
use std::net::TcpStream;

// Added global variables for easier configuring
const SERVER_ADDRESS: &str = "irc.koach.com:6667";
const NICKNAME: &str = "RustyTewl";
const CHANNEL: &str = "#koachsworkshop";

fn main() -> io::Result<()> {
    if let Ok(mut stream) = TcpStream::connect(SERVER_ADDRESS) {
        send(&mut stream, &format!("NICK {}", NICKNAME));
        send(&mut stream, &format!("USER {} 0 * :RustBot", NICKNAME));
        println!("Connected!");
        let mut buffer = String::new();
        loop {
            let mut read_buffer = [0; 4096];
            let got = stream.read(&mut read_buffer)?;
            if got == 0 {
                break;
            }
            let str = std::str::from_utf8(&read_buffer[..got]).unwrap();
            buffer.push_str(str);
            parse(&mut stream, &mut buffer);
        }
    }
    Ok(())
}

fn send(stream: &mut TcpStream, text: &str) {
    let text = format!("{}\n", text);
    stream.write(text.as_bytes()).unwrap();
    print!("{}", text);
}

fn parse(stream: &mut TcpStream, buffer: &mut String) {
    while let Some(pos) = buffer.find('\n') {
        let line = buffer[..pos].trim_end_matches('\r').trim_end_matches('\n');
        if line.is_empty() {
            buffer.clear();
            break;
        }
        handle_line(stream, line);
        buffer.replace_range(..pos + 1, "");
    }
}

fn handle_line(stream: &mut TcpStream, line: &str) {
    if line.starts_with("PING") {
        println!("{}", line);
        let pong_msg = line.replace("PING", "PONG");
        send(stream, &pong_msg);
        return;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        match parts[1] {
            "JOIN" => on_join(stream, line),
            "PART" => on_part(stream, line),
            "PRIVMSG" => on_privmsg(stream, line),
            "QUIT" => on_quit(stream, line),
            "NOTICE" => on_notice(stream, line),
            "MODE" => on_mode(stream, line),
            _ => {
                if let Ok(numeric) = parts[1].parse::<u16>() {
                    on_numeric(stream, numeric, line);
                } else {
                    on_other(stream, line);
                }
            }
        }
    }
}

fn on_numeric(stream: &mut TcpStream, numeric: u16, line: &str) {
    let padded_numeric = format!("{:03}", numeric);
    let parts: Vec<&str> = line.splitn(3, ':').collect();
    if parts.len() >= 3 {
        let message = parts[2].trim();
        println!("Numeric({}): {}", padded_numeric, message);
    }
    match &padded_numeric[..] {
        "001" => send(stream, &format!("JOIN {}",CHANNEL)),
        _ => {
            // do nothing yet
        }
    }
}

fn on_join(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    println!("{} joined {}", parts[0], parts[2]);
}

fn on_part(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    println!("{} left {}", parts[0], parts[2]);
}

fn on_privmsg(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 4 {
        let sender = {
            let sender_split: Vec<&str> = parts[0].split('!').collect();
            sender_split.get(0).map_or("", |s| s.trim_start_matches(':'))
        };

        let channel = parts.get(2).copied().unwrap_or("");

        let message_parts = &parts[3..];
        let mut message = message_parts.join(" ");
        message = message.trim_start_matches(':').to_string();

        println!("{} in {}: {}", sender, channel, message);
    }
}

fn on_quit(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.splitn(3, ':').collect();
    println!("{} quit: {}", parts[0], parts[2]);
}

fn on_notice(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.splitn(4, ':').collect();
    if parts.len() >= 4 && parts[3].starts_with(':') {
        let target = parts[2];
        let message = parts[3].trim_start_matches(':');
        println!("Notice to {}: {}", target, message);
    }
}

fn on_mode(_stream: &mut TcpStream, line: &str) {
    println!("Mode change: {}", line);
}

fn on_other(_stream: &mut TcpStream, line: &str) {
    println!("Other event: {}", line);
}