use colored::*;
use std::{error::Error, io};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use chrono::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server = "irc.koach.com:6667";
    let nickname = "rustytewl";
    let mut client = IrcClient::connect(server).await?;

    client.write(&format!("NICK {}", nickname)).await?;
    client.write(&format!("USER {} 0 * :rustbot",nickname)).await?;

    client.process_messages().await?;

    Ok(())
}

fn printall(clr: &str, text: &str) {
    let now = Local::now();
    let timestamp = now.format("[%H:%M:%S]").to_string();
    match clr {
        "yellow" => println!("{} {}", timestamp, text.yellow()),
        "red" => println!("{} {}", timestamp, text.red()),
        "green" => println!("{} {}", timestamp, text.green()),
        "purple" => println!("{} {}", timestamp, text.purple()),
        "blue" => println!("{} {}", timestamp, text.blue()),
        "cyan" => println!("{} {}", timestamp, text.cyan()),
        "magenta" => println!("{} {}", timestamp, text.magenta()),
        "brightblue" => println!("{} {}", timestamp, text.bright_blue()),
        "brightgreen" => println!("{} {}", timestamp, text.bright_green()),
        "brightred" => println!("{} {}", timestamp, text.bright_red()),
        "brightcyan" => println!("{} {}", timestamp, text.bright_cyan()),
        "grey" => {
            let grey = CustomColor::new(128, 128, 128);
            println!("{} {}", timestamp, text.custom_color(grey));
        },
        _ => println!("{} {}", timestamp, text)
    }
}

struct IrcClient {
    stream: TcpStream,
    message: String,
    nickname: String,
    address: String,
}

impl IrcClient {
    pub async fn connect(server: &str) -> Result<Self, Box<dyn Error>> {
        let stream = TcpStream::connect(server).await?;
        Ok(IrcClient { stream, message: String::new(), nickname: String::new(), address: String::new() })
    }

    pub async fn write(&mut self, data: &str) -> io::Result<usize> {
        if !data.starts_with("PONG") {
            printall("white", &format!("<< {}", data))
        }
        self.stream.write(&format!("{}\n", data).as_bytes()).await
    }

    pub async fn read(&mut self) -> io::Result<()> {
        let mut buffer = [0; 4096];
        let bytes_read = self.stream.read(&mut buffer).await.unwrap();
        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
        }
        let str = std::str::from_utf8(&buffer[..bytes_read]).unwrap();
        self.message.push_str(str);
        Ok(())
    }

    pub async fn process_messages(&mut self) -> io::Result<()> {
        loop {
            self.read().await?;
            while let Some(pos) = self.message.find('\n') {
                let line = self.message[..pos].trim_end_matches('\n').to_string();
                
                if line.is_empty() {
                    self.message.clear();
                    break;
                }
                // print each line in its unparsed form
                // printall("white", &format!(">> {}", line));

                if line.starts_with("PING") {
                    let pong_msg = line.replace("PING", "PONG");
                    self.write(&pong_msg).await?;
                } else {
                    let parts: Vec<&str> = line.split(' ').collect();
                    if parts.len() >= 2 {
                        match parts[1] {
                            "JOIN" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let channel = &parts[2][1..];
    
                                self.on_join(sender, address, channel).await?;
                            },
                            "PART" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let channel = &parts[2][1..];
    
                                self.on_part(sender, address, channel).await?;
                            },
                            "QUIT" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let msg_parts = &parts[2..];
                                let mut msg = msg_parts.join(" ");
                                msg = msg.trim_start_matches(':').to_string();
    
                                self.on_quit(sender, address, &msg).await?;
                            },
                            "NICK" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let newnick = &parts[2][1..];
    
                                self.on_nick(sender, address, newnick).await?;
                            },
                            "KICK" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let channel = parts[2];
                                let target = parts[3];
                                let msg_parts = &parts[4..];
                                let mut msg = msg_parts.join(" ");
                                msg = msg.trim_start_matches(':').to_string();
                                self.on_kick(sender, address, target, channel, &msg).await?;
                            },
                            "NOTICE" => {
                                let target = parts[2];
                                let msg_parts = &parts[3..];
                                let mut msg = msg_parts.join(" ");
                                msg = msg.trim_start_matches(':').to_string();

                                if parts[0].contains('!') {                                    
                                    let sender = parts[0].split('!').next().unwrap();
                                    let sender = &sender[1..];                                    
                                    let address = parts[0].split('!').nth(1).unwrap();                          
                                    if target.starts_with("#") {
                                        self.on_channel_notice(sender, address, target, &msg).await?;
                                    } else {
                                        self.on_private_notice(sender, address, &msg).await?
                                    }
                                } else {                                    
                                    if target.starts_with("#") {
                                        self.on_channel_snotice(target, &msg).await?;
                                    } else {
                                        self.on_private_snotice(&msg).await?
                                    }
                                }
    
                            },
                            "MODE" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let target = parts[2];
                                let msg_parts = &parts[3..];
                                let mut msg = msg_parts.join(" ");
                                msg = msg.trim_start_matches(':').to_string();
                                if target.starts_with("#") {
                                    self.on_chanmode(sender, address, target, &msg).await?;
                                } else {
                                    self.on_usermode(&msg).await?;
                                }
                            },
                            "PRIVMSG" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let target = parts[2];
                                let msg_parts = &parts[3..];
                                let mut msg = msg_parts.join(" ");
                                msg = msg.trim_start_matches(':').to_string();                        

                                if target.starts_with("#") {
                                    self.on_privmsg(sender, address, target, &msg).await?;
                                } else {
                                    self.on_query(sender, address, &msg).await?;
                                }
                            },
                            _ => {
                                if let Ok(numeric) = parts[1].parse::<u16>() {
                                    let padded_numeric = format!("{:03}", numeric);
                                    let message_parts = &parts[3..];
                                    let mut numeric_msg = message_parts.join(" ");
                                    numeric_msg = numeric_msg.trim_start_matches(':').to_string();

                                    self.on_numeric(&padded_numeric, &numeric_msg).await?;                                    
                                } else {
                                    self.on_unsupported(&line).await?;
                                }
                            }
                        }
                    }
                }
                
                self.message.replace_range(..pos + 1, "");
            }
        }
    }

    async fn on_join(&mut self, nick: &str, address: &str, channel: &str) -> io::Result<()> {
        let text = &format!(">> Join: {} ({}) has joined {}", nick, address, channel);
        printall("green", text);
        Ok(())
    }

    async fn on_part(&mut self, nick: &str, address: &str, channel: &str) -> io::Result<()> {
        let text =  &format!(">> Part: {} ({}) has left {}", nick, address, channel);
        printall("green", text);
        Ok(())
    }

    async fn on_quit(&mut self, nick: &str, address: &str, reason: &str) -> io::Result<()> {
        let text = &format!(">> Quit: {} ({}) has left the server. ({})", nick, address, reason);
        printall("brightcyan", text);
        Ok(())
    }

    async fn on_nick(&mut self, nick: &str, address: &str, newnick: &str) -> io::Result<()> {
        if nick == self.nickname { // keep track of your own nick change
            self.nickname = newnick.to_string();
        }
        let text = &format!(">> Nick: {} ({}) has changed their nick to: {}", nick, address, newnick);
        printall("magenta", text);
        Ok(())
    }

    // need to add support for actions and ctcp messages
    async fn on_privmsg(&mut self, nick: &str, _addr: &str, _channel: &str, msg: &str) -> io::Result<()> {
        let text = &format!("{}: {}", nick, msg);
        printall("cyan", text);
        Ok(())
    }

    // need to add support for actions and ctcp messages
    async fn on_query(&mut self, nick: &str, _addr: &str, msg: &str) -> io::Result<()> {
        let text = &format!(">> Query from {} : {}", nick, msg);
        printall("blue", text);
        Ok(())
    }

    async fn on_chanmode(&mut self, nick: &str, _addr: &str, channel: &str, modes: &str) -> io::Result<()> {
        let text = &format!(">> Mode: {} sets modes in {} to {}", nick, channel, modes);
        printall("bightcyan", text);
        Ok(())
    }

    async fn on_usermode(&mut self, modes: &str) -> io::Result<()> {
        let text = &format!(">> Usermode: {}", modes);
        printall("grey", text);
        Ok(())
    }

    async fn on_kick(&mut self, nick: &str, address: &str, knick: &str, channel:&str, reason: &str) -> io::Result<()> {
        let text = &format!(">> Kick: {} ({}) has kicked {} from {} : {}", nick, address, knick, channel, reason);
        printall("red", text);
        Ok(())
    }

    // need to add support for ctcp messages
    async fn on_channel_notice(&mut self, nick: &str, address: &str, channel: &str, message: &str) -> io::Result<()> {
        let text = &format!(">> Notice to {} from {} ({}): {}", channel, nick, address, message);
        printall("purple", text);

        Ok(())
    }

    // need to add support for ctcp messages
    async fn on_private_notice(&mut self, nick: &str, address: &str, message: &str) -> io::Result<()> {
        let text = &format!(">> Notice from {} ({}): {}", nick, address, message);
        printall("brightgreen", text);
        Ok(())
    }

    // need to add support for ctcp messages although it is not likely the server will make a ctcp message
    async fn on_channel_snotice(&mut self, channel: &str, message: &str) -> io::Result<()> {
        let text = &format!(">> SNotice to {} : {}", channel, message);
        printall("red", text);
        Ok(())
    }

    // need to add support for ctcp messages although it is not likely the server will make a ctcp message
    async fn on_private_snotice(&mut self, message: &str) -> io::Result<()> {
        let text = &format!(">> SNotice: {}", message);
        printall("red", text);
        Ok(())
    }

    async fn on_numeric(&mut self, numeric: &str, numeric_msg: &str) -> io::Result<()> {
        // for now print all numerics and their messages
        let text = &format!(">> Numeric({}): {}", numeric, numeric_msg);
        printall("grey", text);
        match &numeric[..] {
            "001" => {
                /* Welcome to...  */ 
                let parts: Vec<&str> = numeric_msg.split(' ').collect();
                if parts.len() > 5 { 
                    self.nickname = parts[6].split('!').next().unwrap().to_string();
                    self.address = parts[6].split('!').nth(1).unwrap().to_string();
                }
                self.write("JOIN #tewlzbox").await?;
            },
            "002" => { /* Your host is... */ } ,
            "003" => { /* This server was created... */ } ,
            "004" => { /* Server type version... */ } ,
            "005" => { /* Server Supported Info */ } ,
            "251" => { /* There are _ users and _ invisible on _ servers. */ } ,
            "252" => { /* _ :operator(s) online */ } ,
            "253" => { /* _ :unknown ocnnections */ } ,
            "254" => { /* _ :channels formed */ } ,
            "255" => { /* I have _ clients and _ servers */ } ,
            "265" => { /* Current local users: _ Max: _ */ } ,
            "266" => { /* Current global users: _ Max: _ */ } ,
            "332" => { /* Channel Topic */ } ,
            "333" => { /* Channel Topic set by and timestamp */ } ,
            "353" => { /* Channel /NAMES LIST */ } ,
            "366" => { /* End of /NAMES LIST */ } ,
            "375" => { /* START OF MOTD */ } ,
            "372" => { /* MOTD */ } ,
            "376" => { /* END OF MOTD */ } ,
            _ => {
                // For more information on numerics: https://datatracker.ietf.org/doc/html/rfc2812
            }
        }
        Ok(())
    }

    async fn on_unsupported(&mut self, line: &str) -> io::Result<()> {
        let text = &format!("Unsupported event: {}", line); // print anything i have not added/forgot
        printall("red", text);
        Ok(())
    }
}
