use std::io::{self, Read, Write};
use std::net::TcpStream;

// Added global variables for easier configuring
const SERVER_ADDRESS: &str = "irc.koach.com:6667";
const NICKNAME: &str = "RustyTewl";
const CHANNEL: &str = "#KoachsWorkShop";

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
    //print!("{}", text);
}

fn parse(stream: &mut TcpStream, buffer: &mut String) {
    while let Some(pos) = buffer.find('\n') {
        let line = buffer[..pos].trim_end_matches('\n');
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
        //println!("{}", line);
        let pong_msg = line.replace("PING", "PONG");
        send(stream, &pong_msg);
        return;
    }
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() >= 2 {
        match parts[1] {
            "JOIN" => on_join(stream, line),
            "PART" => on_part(stream, line),
            "PRIVMSG" => on_privmsg(stream, line),
            "QUIT" => on_quit(stream, line),
            "NOTICE" => on_notice(stream, line),
            "MODE" => on_mode(stream, line),
            "KICK" => on_kick(stream, line),
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
// :NICK!USER@ADDRESS JOIN :<CHANNEL>
fn on_join(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();

    let sender = parts[0].split('!').next().unwrap();
    let sender = &sender[1..];

    let channel = &parts[2][1..];

    println!("{} has joined {}", sender, channel);
}
// :NICK!USER@ADDRESS PART :<CHANNEL>
fn on_part(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();

    let sender = parts[0].split('!').next().unwrap();
    let sender = &sender[1..];

    let channel = &parts[2][1..];

    println!("{} has left {}", sender, channel);
}
// :NICK!USER@ADDRESS PRIVMSG <NICKNAME|CHANNEL> :<MESSAGE>
fn on_privmsg(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() >= 4 {
        let sender = parts[0].split('!').next().unwrap();
        let sender = &sender[1..];

        let target = parts[2];

        let message_parts = &parts[3..];
        let mut message = message_parts.join(" ");
        message = message.trim_start_matches(':').to_string();

        if target == CHANNEL {
            println!("{}: {}", sender, message);
        } else {
            println!("QUERY({}): {}", sender, message);
        }
    }
}
// :NICK!USER@ADDRESS QUIT :<MESSAGE>
fn on_quit(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    println!("{}", line);
    if parts.len() >= 3 {
        let sender = parts[0].split('!').next().unwrap();
        let sender = &sender[1..];

        let message_parts = &parts[2..];
        let mut message = message_parts.join(" ");
        message = message.trim_start_matches(':').to_string();

        println!("{} has quit: {}", sender, message);
    }
}
// :NICK!USER@ADDRESS NOTICE <NICKNAME|CHANNEL> :<MESSAGE>
fn on_notice(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() >= 3 && parts[3].starts_with(':') {
        let sender = parts[0].split('!').next().unwrap();
        let sender = &sender[1..];

        let target = parts[2];

        let message_parts = &parts[3..];
        let mut message = message_parts.join(" ");
        message = message.trim_start_matches(':').to_string();

        if target == CHANNEL {
            println!("CNOTICE({}): {}", sender, message);
        } else {
            println!("PNOTICE({}): {}", sender, message);
        }
    }
}
// :NICK!USER@ADDRESS MODE <CHANNEL|NICKNAME> <MODES> <PARAMS>
// * If there are no params a colon will be appear before <MODES> otherwise a colon will appear before the last <PARAM> in the message 
fn on_mode(_stream: &mut TcpStream, line: &str) {
    println!("Mode change: {}", line);
}
// :NICK!USER@ADDRESS KICK <CHANNEL> <NICK> :<MESSAGE>
fn on_kick(_stream: &mut TcpStream, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() >= 4 && parts[4].starts_with(':') {
        let sender = parts[0].split('!').next().unwrap();
        let sender = &sender[1..];

        let channel = parts[2];

        let target = parts[3];

        let message_parts = &parts[4..];
        let mut message = message_parts.join(" ");
        message = message.trim_start_matches(':').to_string();

        println!("{} has kicked {} from {} :{}", sender, target, channel, message);
    }
}

fn on_other(_stream: &mut TcpStream, line: &str) {
    println!("Other event: {}", line);
}