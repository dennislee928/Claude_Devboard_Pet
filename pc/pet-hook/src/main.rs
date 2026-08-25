//! pet-hook: invoked by Claude Code hooks. Forwards the hook's stdin JSON to
//! the petd daemon at 127.0.0.1:8127 and exits. Always exits 0 and stays
//! silent so a missing daemon never disturbs the Claude Code session.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() {
    let mut body = String::new();
    let _ = std::io::stdin().read_to_string(&mut body);
    if body.trim().is_empty() {
        return;
    }
    let _ = send(&body);
}

fn send(body: &str) -> std::io::Result<()> {
    let addr = "127.0.0.1:8127".parse().unwrap();
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    s.set_write_timeout(Some(Duration::from_millis(300)))?;
    s.set_read_timeout(Some(Duration::from_millis(300)))?;
    write!(
        s,
        "POST /event HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )?;
    let mut buf = [0u8; 256];
    let _ = s.read(&mut buf);
    Ok(())
}
