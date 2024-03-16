extern crate md5;

use colored::*;
use std::{env, error::Error, fs};
use std::io::{self, Write}; // Import io and Write trait
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Debug, Serialize, Deserialize)]
struct BuzzenConfig {
    nickname: String,
    email: String,
    password: String,
    server: String,
    channel: String,
}

impl BuzzenConfig {
    // Read configuration from file
    fn from_file(filename: &str) -> Result<Self, Box<dyn Error>> {
        let current_dir = env::current_dir()?;
        let config_path = current_dir.join(filename);

        match fs::read_to_string(&config_path) {
            Ok(contents) => {
                let config: BuzzenConfig = serde_json::from_str(&contents)?;
                Ok(config)
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    // File doesn't exist, create a default configuration and write it to the file
                    let default_config = BuzzenConfig {
                        nickname: String::new(),
                        email: String::new(),
                        password: String::new(),
                        server: String::new(),
                        channel: String::new(),
                    };
                    default_config.to_file(&config_path.into_os_string().into_string().unwrap())?;
                    Ok(default_config)
                } else {
                    Err(Box::new(err))
                }
            }
        }
    }

    // Write configuration to file
    fn to_file(&self, filename: &str) -> Result<(), Box<dyn Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(filename, json)?;
        Ok(())
    }
}
// irc.chat.twitch.tv
//  "CAP REQ :twitch.tv/membership twitch.tv/tags twitch.tv/commands";
// ("PASS oauth:{}", config.accessToken);
// ("NICK {}", config.nickname);
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load settings from config.json
    let config = BuzzenConfig::from_file("config.json")?;

    let mut client = IrcClient::connect(&config.server, &config.channel).await?;

    printall("alert", "Connected! Starting authentication process...");
    client.write("AUTHTYPE ircwx1").await?;

    let config = BuzzenConfig::from_file("config.json")?;

    let passwd = md5::compute(config.password);

    client.write(&format!("LOGINH {} {:?}", config.email, passwd)).await?;
    client.write(&format!("USER {} * 0 :RustBot", config.nickname)).await?;
    client.write("CLIENTMODE cd1").await?;

    let terminal = tokio::spawn(async move {
        loop {
            let mut input = String::new();
            std::io::stdout().flush().expect("Failed to flush stdout");
            std::io::stdin().read_line(&mut input).expect("Failed to read line");
            
            // Send the user input as a PRIVMSG to the IRC server
            println!("Input: {}", input);
        }
    });

    let server = tokio::spawn(async move {
        let _ = client.process_messages().await;
    });

    tokio::try_join!(terminal, server)?;

    Ok(())
}

fn printall(event: &str, text: &str) {
    let now = Local::now();
    let timestamp = now.format("[%H:%M:%S]").to_string();
    match event {
        "away" => {     
            let blueish = CustomColor::new(50, 109, 168);
            println!("{} {}", timestamp, text.custom_color(blueish));
        },
        "unaway" => {     
            let blueish = CustomColor::new(50, 109, 168);
            println!("{} {}", timestamp, text.custom_color(blueish));
        },
        "alert" => println!("{} {}", timestamp, text.yellow()),
        "alert_blue" => println!("{} {}", timestamp, text.bright_blue()),
        "sctcp" => {            
            let reddish = CustomColor::new(209, 82, 109);
            println!("{} {}", timestamp, text.custom_color(reddish));
        },
        "ctcpreply" => {            
            let orange = CustomColor::new(255, 165, 0);
            println!("{} {}", timestamp, text.custom_color(orange));
        },
        "ctcprequest" => {            
            let orange = CustomColor::new(255, 165, 0);
            println!("{} {}", timestamp, text.custom_color(orange));
        },
        "snotice" => println!("{} {}", timestamp, text.bright_red()),
        "join" => println!("{} {}", timestamp, text.green()),
        "part" => println!("{} {}", timestamp, text.green()),
        "welcome" => println!("{} {}", timestamp, text.bright_green()),
        "quit" => println!("{} {}", timestamp, text.green()),
        "kick" => println!("{} {}", timestamp, text.red()),
        "notice" => println!("{} {}", timestamp, text.bright_magenta()),
        "nick" => println!("{} {}", timestamp, text.bright_blue()),
        "numeric" => {
            let grey = CustomColor::new(128, 128, 128);
            println!("{} {}", timestamp, text.custom_color(grey));
        },
        "mode" => println!("{} {}", timestamp, text.cyan()),
        "privmsg" => println!("{} {}", timestamp, text.bright_white()),
        "query" => println!("{} {}", timestamp, text.white()),
        "action" => println!("{} {}", timestamp, text.italic().purple()),
        _ => println!("{} {}", timestamp, text)
    }
}

struct IrcClient {
    stream: TcpStream,
    message: String,
    nickname: String,
    address: String,
    channel: String,
}

impl IrcClient {
    pub async fn connect(server: &str, channel: &str) -> Result<Self, Box<dyn Error>> {
        let stream = TcpStream::connect(server).await?;
        let channel = channel.to_string();
        Ok(IrcClient { stream, message: String::new(), nickname: String::new(), address: String::new(),  channel })
    }

    pub async fn write(&mut self, data: &str) -> io::Result<usize> {
        if !data.starts_with("PONG") {
            if data.starts_with("LOGIN") {
                printall("default", "<< LOGINH ********** ***********")
            } else {
                printall("default", &format!("<< {}", data))
            }
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
                 //printall("default", &format!(">> {}", line));

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
                                // because buzzen is weird registered account join
                                if parts[2].starts_with(':') {
                                    // :<NICK!USER@ADDRESS> JOIN <PROFILE_DATA> :<CHANNEL>
                                    let channel = &parts[2][1..];    
                                    self.on_join(sender, address, channel).await?;
                                } else { // guest join
                                    // :<NICK!USER@ADDRESS> JOIN :<CHANNEL>
                                    let channel = &parts[3][1..];    
                                    self.on_join(sender, address, channel).await?;
                                }
                            },
                            "PART" => { // :<NICK!USER@ADDRESS> PART <CHANNEL>
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let channel = &parts[2];
    
                                self.on_part(sender, address, channel).await?;
                            },
                            "QUIT" => {
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let msg_parts = &parts[2..];
                                let mut msg = msg_parts.join(" ");
                                msg.remove(0);
                                let msg = trim_trailing_whitespace(&msg);   
    
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
                                msg.remove(0);
                                let msg = trim_trailing_whitespace(&msg);   
                                self.on_kick(sender, address, target, channel, &msg).await?;
                            },
                            "NOTICE" => {
                                let target = parts[2];

                                if parts[0].contains('!') {                                    
                                    let sender = parts[0].split('!').next().unwrap();
                                    let sender = &sender[1..];                                    
                                    let address = parts[0].split('!').nth(1).unwrap();                          
                                    if parts[3].starts_with(':') {                                        
                                        let msg_parts = &parts[3..];
                                        let mut msg = msg_parts.join(" ");
                                        msg.remove(0);
                                        self.on_channel_notice(sender, address, target, &msg).await?;
                                    } else { // because buzzen is weird
                                        // :<NICK!USER@ADDRESS> NOTICE <CHANNEL> <NICKNAME> :<MESSAGE>
                                        let msg_parts = &parts[4..];
                                        let mut msg = msg_parts.join(" ");
                                        msg.remove(0);
                                        let msg = trim_trailing_whitespace(&msg);   
                                        self.on_private_notice(sender, address, &msg).await?
                                    }
                                } else {     
                                    let msg_parts = &parts[3..];
                                    let mut msg = msg_parts.join(" ");
                                    msg.remove(0);                  
                                    let msg = trim_trailing_whitespace(&msg);                
                                    if target.starts_with('%') || target.starts_with('#') {
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
                                msg.remove(0);
                                let msg = trim_trailing_whitespace(&msg);   
                                if target.starts_with('%') || target.starts_with('#') {
                                    self.on_chanmode(sender, address, target, &msg).await?;
                                } else {
                                    self.on_usermode(&msg).await?;
                                }
                            },
                            "WHISPER" => {
                                // :<NICK!USER@ADDRESS> WHISPER <CHANNEL> <TARGET> :<MESSAGE>
                                let sender = parts[0].split('!').next().unwrap();
                                let sender = &sender[1..];
                                let address = parts[0].split('!').nth(1).unwrap();
                                let target = parts[2];
                                let msg_parts = &parts[4..];
                                let mut msg = msg_parts.join(" ");
                                msg.remove(0);  
                                let msg = trim_trailing_whitespace(&msg);                     

                                self.on_whisper(sender, address, target, &msg).await?;
                            },
                            "PRIVMSG" => {
                                if parts[0].starts_with(":%") || parts[0].starts_with(":#") {
                                    // Welcome message on buzzen is sent :%#Channelname PRIVMSG %#ChannelName :<WelcomeMessage>
                                    let target = parts[2];
                                    let msg_parts = &parts[3..];
                                    let mut msg = msg_parts.join(" ");
                                    msg.remove(0);
                                    let msg = trim_trailing_whitespace(&msg);   
                                    self.on_welcome(target, &msg).await?;
                                } else {   
                                    let sender = parts[0].split('!').next().unwrap();
                                    let sender = &sender[1..];
                                    let address = parts[0].split('!').nth(1).unwrap_or("");
                                    let target = parts[2];     

                                    //:<NICK!USER@ADDRESS> PRIVMSG <CHANNEL> :<MESSAGE> 
                                    if parts[3].starts_with(':') {
                                        let msg_parts = &parts[3..];
                                        let mut msg = msg_parts.join(" ");
                                        msg.remove(0);   
                                        let msg = trim_trailing_whitespace(&msg);                
                                        self.on_privmsg(sender, address, target, &msg).await?;
                                    } else { // because buzzen is weird
                                        // :<NICK!USER@ADDRESS> PRIVMSG <CHANNEL> <NAME> :<MESSAGE> 
                                        let msg_parts = &parts[4..];
                                        let mut msg = msg_parts.join(" ");
                                        msg.remove(0);    
                                        let msg = trim_trailing_whitespace(&msg);            
                                        self.on_query(sender, address, &msg).await?;
                                    }
                                }
                            },
                            _ => {
                                if let Ok(numeric) = parts[1].parse::<u16>() {
                                    let padded_numeric = format!("{:03}", numeric);

                                    self.on_numeric(&padded_numeric, &line).await?;                                    
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


    async fn on_welcome(&mut self, channel: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        let text = &format!(">> Welcome message for {} : {}", channel, message);
        printall("welcome", text);
        Ok(())
    }

    async fn on_whisper(&mut self, nick: &str, address: &str, channel: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        let text = &format!(">> Query from {} ({}) in {} : {}", nick, address, channel, message);
        printall("whisper", text);
        Ok(())
    }

    async fn on_join(&mut self, nick: &str, address: &str, channel: &str) -> io::Result<()> {
        let text = &format!(">> Join: {} ({}) has joined {}", nick, address, channel);
        printall("join", text);
        Ok(())
    }

    async fn on_part(&mut self, nick: &str, address: &str, channel: &str) -> io::Result<()> {
        let text =  &format!(">> Part: {} ({}) has left {}", nick, address, channel);
        printall("part", text);
        Ok(())
    }

    async fn on_quit(&mut self, nick: &str, address: &str, reason: &str) -> io::Result<()> {
        let text = &format!(">> Quit: {} ({}) has left the server. ({})", nick, address, reason);
        printall("quit", text);
        Ok(())
    }

    async fn on_nick(&mut self, nick: &str, address: &str, newnick: &str) -> io::Result<()> {
        if nick == self.nickname { // keep track of your own nick change
            self.nickname = newnick.to_string();
        }
        let text = &format!(">> Nick: {} ({}) has changed their nick to: {}", nick, address, newnick);
        printall("nick", text);
        Ok(())
    }

    // need to add support for actions and ctcp messages
    async fn on_privmsg(&mut self, nick: &str, address: &str, channel: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let message = message.replace('\u{0001}', "");
            let parts: Vec<&str> = message.split(' ').collect();
            if parts[0].to_uppercase() == "ACTION" {
                let action_message: String;
                if parts.len() > 1 {
                    action_message = parts[1..].join(" ");
                } else {
                    action_message = String::new(); // handle possible blank action
                }
                self.on_action(nick, address, channel, &action_message).await?
            } else {
                // CTCP Request
                self.on_ctcp_request(nick, address, &message).await?
            }            
        } else {
            // because buzzen is weird 
            if message.starts_with('\u{0002}') && message.ends_with('\u{0002}') && message.len() > 1 {
                let message = &message[1..message.len() - 1];
                let parts: Vec<&str> = message.split(' ').collect();
                let ctcp_type = parts[0].to_uppercase();
                if parts.len() > 1 {
                    let ctcp_reply = parts[1..].join(" ");
                    self.on_ctcp_reply(nick, address, &ctcp_type, &ctcp_reply).await?
                } else {
                    self.on_ctcp_request(nick, address, &ctcp_type).await?
                }
            } else {
                let text = &format!("{}: {}", nick, message);
                printall("privmsg", &text);
            }
        }
        Ok(())
    }

    async fn on_action(&mut self, nick: &str, _addrress: &str, _channel: &str, message: &str) -> io::Result<()> {
        let text = &format!("{} {}", nick, message);
        printall("action", text);
        Ok(())     
    }

    // need to add support for actions and ctcp messages
    async fn on_query(&mut self, nick: &str, address: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let message = &message[1..message.len() - 1];
            let parts: Vec<&str> = message.split(' ').collect();
            if parts[0].to_uppercase() == "ACTION" {
                let action_message: String;
                if parts.len() > 1 {
                    action_message = parts[1..].join(" ");
                } else {
                    action_message = String::new(); // handle possible blank action
                }
                self.on_query_action(nick, address,  &action_message).await?
            } else {
                // CTCP Request
                self.on_ctcp_request(nick, address, message).await?
            }
        } else {
            // because buzzen is weird
            if message.starts_with('\u{0002}') && message.ends_with('\u{0002}') && message.len() > 1 {
                let message = &message[1..message.len() - 1];
                let parts: Vec<&str> = message.split(' ').collect();
                let ctcp_type = parts[0].to_uppercase();
                if parts.len() > 1 {
                    let ctcp_reply = parts[1..].join(" ");
                    self.on_ctcp_reply(nick, address, &ctcp_type, &ctcp_reply).await?
                } else {
                    self.on_ctcp_request(nick, address, &ctcp_type).await?
                }
            } else {
                let text = &format!("{}: {}", nick, message);
                printall("privmsg", &text);
            }
        }
        Ok(())
    }

    // not going to bother adding channel specific ctcp requests since the bot only sits in 1 channel 
    async fn on_ctcp_request(&mut self, nick: &str, address: &str, request: &str) -> io::Result<()> {
        let text = &format!(">> CTCP {} Request from {} ({})", request, nick, address);
        printall("ctcprequest", text);
        Ok(())
    }

    async fn on_query_action(&mut self, nick: &str, _address: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        let text = &format!(">> Query from {} : {}", nick, message);
        printall("action", text);
        Ok(())
    }

    async fn on_chanmode(&mut self, nick: &str, _address: &str, channel: &str, modes: &str) -> io::Result<()> {
        let text = &format!(">> Mode: {} sets modes in {} to {}", nick, channel, modes);
        printall("mode", text);
        Ok(())
    }

    async fn on_usermode(&mut self, modes: &str) -> io::Result<()> {
        let text = &format!(">> Usermode: {}", modes);
        printall("usermode", text);
        Ok(())
    }

    async fn on_kick(&mut self, nick: &str, address: &str, knick: &str, channel:&str, reason: &str) -> io::Result<()> {
        let text = &format!(">> Kick: {} ({}) has kicked {} from {} : {}", nick, address, knick, channel, reason);
        printall("kick", text);
        Ok(())
    }

    async fn on_channel_notice(&mut self, nick: &str, address: &str, channel: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let msg = &message[1..message.len() - 1];
            let parts: Vec<&str> = msg.split(' ').collect();
            let ctcp_type = parts[0];
            if parts.len() == 1 {
                self.on_ctcp_reply(nick, address, ctcp_type, "").await?
            } else {
                let ctcp_reply = parts[1..].join(" ");                
                self.on_ctcp_reply(nick, address, ctcp_type, &ctcp_reply).await?
            }
        } else {
            let text = &format!(">> Notice to {} from {} ({}): {}", channel, nick, address, message);
            printall("notice", text);
        }
        Ok(())
    }

    async fn on_ctcp_reply(&mut self, nick: &str, address: &str, ctcp_type: &str, ctcp_reply: &str) -> io::Result<()> {
        let text = &format!(">> CTCP {} Reply from {} ({}) : {}", ctcp_type, nick, address, ctcp_reply);
        printall("ctcpreply", text);
        Ok(())
    }

    // need to add support for ctcp messages
    async fn on_private_notice(&mut self, nick: &str, address: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let msg = &message[1..message.len() - 1];
            let parts: Vec<&str> = msg.split(' ').collect();
            let ctcp_type = parts[0];
            if parts.len() == 1 {
                self.on_ctcp_reply(nick, address, ctcp_type, "").await?
            } else {
                let ctcp_reply = parts[1..].join(" ");                
                self.on_ctcp_reply(nick, address, ctcp_type, &ctcp_reply).await?
            }
        } else {
            let text = &format!(">> Notice from {} ({}): {}", nick, address, message);
            printall("notice", text);
        }
        Ok(())
    }

    async fn on_channel_snotice(&mut self, channel: &str, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let msg = &message[1..message.len() - 1];
            let parts: Vec<&str> = msg.split(' ').collect();
            let ctcp_type = parts[0];
            if parts.len() == 1 {
                self.on_server_ctcp(ctcp_type, "").await?
            } else {
                let ctcp_reply = parts[1..].join(" ");                
                self.on_server_ctcp(ctcp_type, &ctcp_reply).await?
            }
        } else {
            let text = &format!(">> Notice to {} : {}", channel, message);
            printall("snotice", &text);
        }
        Ok(())
    }

    async fn on_private_snotice(&mut self, message: &str) -> io::Result<()> {
        let message = &strip_style(&message);
        if message.starts_with('\u{0001}') && message.ends_with('\u{0001}') && message.len() > 1 {
            let msg = &message[1..message.len() - 1];
            let parts: Vec<&str> = msg.split(' ').collect();
            let ctcp_type = parts[0];
            if parts.len() == 1 {
                self.on_server_ctcp(ctcp_type, "").await?
            } else {
                let ctcp_reply = parts[1..].join(" ");                
                self.on_server_ctcp(ctcp_type, &ctcp_reply).await?
            }
        } else {
            let text = &format!(">> Notice: {}", message);
            printall("snotice", text);
        }
        Ok(())
    }

    async fn on_server_ctcp(&mut self, ctcp_type: &str, ctcp_reply: &str) -> io::Result<()> {
        let text = &format!(">> CTCP {} from Server: {}", ctcp_type, ctcp_reply);
        printall("sctcp", text);
        Ok(())
    }

    async fn on_numeric(&mut self, numeric: &str, line: &str) -> io::Result<()> {
        // for now print all numerics and their messages
        let parts: Vec<&str> = line.split(' ').collect();
        let message_parts = &parts[3..];
        let mut numeric_msg = message_parts.join(" ");
        if numeric_msg.starts_with(':') {
            numeric_msg.remove(0);
        }
        let text = &format!(">> Numeric({}): {}", numeric, numeric_msg);
        match &numeric[..] {
            "001" => {
                /* Welcome to...  */ 
                printall("numeric", text);
                let parts: Vec<&str> = numeric_msg.split(' ').collect();
                if parts.len() > 5 { 
                    self.nickname = parts[5].split('!').next().unwrap().to_string();
                    self.address = parts[5].split('!').nth(1).unwrap().to_string();
                }
                self.write(&format!("JOIN {}",self.channel)).await?;
            },
            /* 
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
            */
            "821" => { /* :UNAWAY MESSAGE */
                // :<NICK!USER@ADDRESS> 821 <CHANNEL> :<MESSAGE>
                let sender = parts[0].split('!').next().unwrap();
                let sender = &sender[1..];
                let message = &strip_style(&numeric_msg);
                let message = trim_trailing_whitespace(&message);
                // let address = parts[0].split('!').nth(1).unwrap();
                let text = format!(">> Back: {} has returned! ({})", sender, message);
                printall("unaway", &text)
            },
            "822" => { /* :AWAY MESSAGE */
                // :<NICK!USER@ADDRESS> 822 <CHANNEL> :<MESSAGE>
                let sender = parts[0].split('!').next().unwrap();
                let sender = &sender[1..];
                let message = &strip_style(&numeric_msg);
                let message = trim_trailing_whitespace(&message);
                let text = format!(">> Away: {} has gone away. ({})", sender, message);
                printall("away", &text)
            },
            _ => {
                // For more information on numerics: https://datatracker.ietf.org/doc/html/rfc2812
                printall("numeric", text);
            }
        }
        Ok(())
    } 

    async fn on_unsupported(&mut self, line: &str) -> io::Result<()> {
        let text = &format!("Unsupported event: {}", line); // print anything i have not added/forgot
        printall("default", text);
        Ok(())
    }
}

fn strip_style(value: &str) -> String {
    // strip Buzzen [style] tags
    let style_regex = Regex::new(r"\[(?:/)?style(?:[^\]]+)?\]").unwrap();
    let result = style_regex.replace_all(value, "");

    // strip mIRC codes (underline, bold, color, ect...)
    let special_regex = Regex::new(r"(\u{0003}(\d(\d)?(,(\d(\d)?)?)?)?|\u{001F}|\u{0002}|\u{000F}|\u{0016})").unwrap();
    special_regex.replace_all(&result, "").to_string()
}

fn trim_trailing_whitespace(input: &str) -> String {
    input.chars()
        .rev() // Start from the end of the string
        .skip_while(|&c| c.is_whitespace()) // Skip whitespaces
        .collect::<String>() // Collect the characters into a string
        .chars() // Reversed, so reverse it again to get original order
        .rev()
        .collect::<String>() // Collect the characters into a string
}