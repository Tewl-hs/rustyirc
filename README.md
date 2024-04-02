# RustyIRC - Simple IRC Bot written in Rust

### Additions and Changes
    + Switched from basic IRCd server to Buzzen.com
        - Added serde for json serialization/deserialization on config file
        - Added md5 to create hash password needed to login
        - Added regex function for striping Buzzen [style] tags as well as mIRC codes from text
    + Created a struct and impl to act as the connection object
    + Parsed incoming data into events
    + Switched to tokio crate for async tcpstream
    + Added Color to the terminal and timestamp
        - Added colored crate for terminal coloring
        - Also added chrono for timestamp
    + Basic TCP Connection for Internet Relay Chat
