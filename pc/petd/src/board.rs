//! Serial link to the ESP32 board — auto-detects the USB-UART bridge,
//! reconnects on failure, and speaks the line protocol in both directions.
//!
//! PC -> board
//!   {"s":"coding","c":"grogu","lv":3}          standalone edition: PC is the brain
//!   {"e":<code>,"t":<toolkind>}                firmware edition: raw event, board decides
//!   {"e":20,"c":"grogu"}                       set character
//!   {"m":"Opus 5","a":"Editing main.rs","n":2,"tk":12345}   session status for the board screen
//!
//! board -> PC
//!   {"ok":1}                                   ack
//!   {"s":"coding","lv":3,"xp":432,"nx":1200}   firmware edition status broadcast
//!
//! Serial device names differ per OS (COM9 on Windows, /dev/cu.* on macOS,
//! /dev/ttyUSB* on Linux); detection is by USB VID/PID so it works on all three.

use crate::state::{Event, ToolKind};
use serialport::{SerialPort, SerialPortType};
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What petd wants the board to know.
#[derive(Clone, PartialEq, Debug)]
pub enum BoardMsg {
    /// Standalone edition: the PC already decided everything.
    Render { state: String, chr: String, level: u8 },
    /// Firmware edition: a raw event for the board's own state machine.
    Event(Event),
    /// Character choice (firmware edition).
    SetChar(String),
    /// Claude Code status strip shown under the pet on the board screen.
    Status { model: String, action: String, sessions: usize, tokens: u64 },
}

/// What the board reports back (firmware edition: the board is the brain).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoardStatus {
    pub state: usize,
    pub level: u8,
    pub xp: u64,
    pub next: Option<u64>,
}

const KNOWN_BRIDGES: [(u16, u16); 5] = [
    (0x1A86, 0x55D4), // CH9102 (this project's DevKit V1)
    (0x10C4, 0xEA60), // CP2102
    (0x1A86, 0x7523), // CH340
    (0x0403, 0x6001), // FT232
    (0x303A, 0x1001), // ESP32-S2/S3 native USB CDC
];

pub fn detect_port() -> Option<String> {
    for p in serialport::available_ports().ok()? {
        if let SerialPortType::UsbPort(info) = &p.port_type {
            if KNOWN_BRIDGES.contains(&(info.vid, info.pid)) {
                // macOS exposes both /dev/tty.* (blocking) and /dev/cu.*; prefer cu.
                if p.port_name.contains("/dev/tty.") {
                    continue;
                }
                return Some(p.port_name);
            }
        }
    }
    None
}

/// Event codes shared with firmware/src/proto.h.
pub fn event_code(ev: &Event) -> Option<(u8, u8)> {
    let kind = |k: &ToolKind| match k {
        ToolKind::Edit => 0u8,
        ToolKind::RunTest => 1,
        ToolKind::RunBuild => 2,
        ToolKind::Search => 3,
        ToolKind::Agent => 4,
        ToolKind::Other => 5,
    };
    Some(match ev {
        Event::Prompt => (1, 0),
        Event::SessionStart => (2, 0),
        Event::ToolStart(k) => (3, kind(k)),
        Event::ToolEnd { error: false } => (4, 0),
        Event::ToolEnd { error: true } => (5, 0),
        Event::Stopped => (6, 0),
        Event::PermissionWait => (7, 0),
        Event::Petted => (8, 0),
        Event::Feed => (9, 0),
        Event::ToggleSleep => (10, 0),
        Event::SubagentDone => (11, 0),
        Event::ForceState(s) => (12, *s as u8),
        _ => return None,
    })
}

fn escape(s: &str) -> String {
    s.chars().filter(|c| *c != '"' && *c != '\\' && *c != '\n').take(28).collect()
}

fn encode(m: &BoardMsg) -> Option<String> {
    Some(match m {
        BoardMsg::Render { state, chr, level } => format!("{{\"s\":\"{state}\",\"c\":\"{chr}\",\"lv\":{level}}}\n"),
        BoardMsg::Event(ev) => {
            let (code, arg) = event_code(ev)?;
            format!("{{\"e\":{code},\"t\":{arg}}}\n")
        }
        BoardMsg::SetChar(c) => format!("{{\"e\":20,\"c\":\"{c}\"}}\n"),
        BoardMsg::Status { model, action, sessions, tokens } => format!(
            "{{\"m\":\"{}\",\"a\":\"{}\",\"n\":{sessions},\"tk\":{tokens}}}\n",
            escape(model),
            escape(action)
        ),
    })
}

/// Parse a status line the board broadcasts in firmware edition.
pub fn parse_status(line: &str) -> Option<BoardStatus> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let name = v.get("s")?.as_str()?;
    let state = crate::assets_gen::STATE_NAMES.iter().position(|n| *n == name)?;
    Some(BoardStatus {
        state,
        level: v.get("lv").and_then(|x| x.as_u64()).unwrap_or(1).clamp(1, 5) as u8,
        xp: v.get("xp").and_then(|x| x.as_u64()).unwrap_or(0),
        next: v.get("nx").and_then(|x| x.as_u64()).filter(|n| *n > 0),
    })
}

fn open(port_override: &Option<String>) -> Option<Box<dyn SerialPort>> {
    let name = port_override.clone().or_else(detect_port)?;
    match serialport::new(&name, 115_200).timeout(Duration::from_millis(200)).open() {
        Ok(p) => {
            println!("petd: board connected on {name}");
            Some(p)
        }
        Err(e) => {
            eprintln!("petd: open {name} failed: {e}");
            None
        }
    }
}

/// Run the serial link. `status_tx` receives whatever the board reports (used
/// by the firmware edition to drive the PC-side window).
pub fn spawn(
    rx: Receiver<BoardMsg>,
    port_override: Option<String>,
    shared: Arc<Mutex<crate::Shared>>,
    status_tx: Option<Sender<BoardStatus>>,
) {
    std::thread::spawn(move || {
        let mut port: Option<Box<dyn SerialPort>> = None;
        let set_conn = |on: bool| {
            if let Ok(mut s) = shared.lock() {
                s.board_connected = on;
            }
        };
        let mut heartbeat: Option<BoardMsg> = None;
        let mut last_attempt = std::time::Instant::now() - Duration::from_secs(10);
        let mut rx_buf = String::new();
        loop {
            let msg = match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(m) => {
                    if matches!(m, BoardMsg::Render { .. }) {
                        heartbeat = Some(m.clone()); // only full renders are worth repeating
                    }
                    Some(m)
                }
                Err(RecvTimeoutError::Timeout) => heartbeat.clone(),
                Err(RecvTimeoutError::Disconnected) => return,
            };

            if port.is_none() && last_attempt.elapsed() > Duration::from_secs(5) {
                last_attempt = std::time::Instant::now();
                port = open(&port_override);
                set_conn(port.is_some());
            }
            let Some(p) = port.as_mut() else { continue };

            if let Some(line) = msg.as_ref().and_then(encode) {
                if p.write_all(line.as_bytes()).is_err() {
                    eprintln!("petd: board write failed, will reconnect");
                    port = None;
                    set_conn(false);
                    continue;
                }
            }

            // drain the board's replies so the OS buffer never fills, and pick
            // up status broadcasts on the way
            let mut scratch = [0u8; 256];
            while p.bytes_to_read().unwrap_or(0) > 0 {
                match p.read(&mut scratch) {
                    Ok(n) if n > 0 => rx_buf.push_str(&String::from_utf8_lossy(&scratch[..n])),
                    _ => break,
                }
            }
            while let Some(nl) = rx_buf.find('\n') {
                let line: String = rx_buf.drain(..=nl).collect();
                if let (Some(tx), Some(st)) = (status_tx.as_ref(), parse_status(&line)) {
                    let _ = tx.send(st);
                }
            }
            if rx_buf.len() > 4096 {
                rx_buf.clear();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_both_protocol_styles() {
        assert_eq!(
            encode(&BoardMsg::Render { state: "coding".into(), chr: "grogu".into(), level: 3 }).unwrap(),
            "{\"s\":\"coding\",\"c\":\"grogu\",\"lv\":3}\n"
        );
        assert_eq!(encode(&BoardMsg::Event(Event::ToolStart(ToolKind::RunTest))).unwrap(), "{\"e\":3,\"t\":1}\n");
        assert_eq!(encode(&BoardMsg::SetChar("beemo".into())).unwrap(), "{\"e\":20,\"c\":\"beemo\"}\n");
    }

    #[test]
    fn status_strings_cannot_break_the_json() {
        let line = encode(&BoardMsg::Status {
            model: "Opus \"5\"".into(),
            action: "Editing a\\b\nc".into(),
            sessions: 2,
            tokens: 99,
        })
        .unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(line.trim()).is_ok());
    }

    #[test]
    fn parses_board_status() {
        let st = parse_status("{\"s\":\"testing\",\"lv\":4,\"xp\":1500,\"nx\":3000}\n").unwrap();
        assert_eq!(st.state, crate::state::TESTING);
        assert_eq!(st.level, 4);
        assert_eq!(st.next, Some(3000));
        assert!(parse_status("garbage").is_none());
    }
}
