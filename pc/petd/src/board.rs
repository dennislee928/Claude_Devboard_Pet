//! Serial link to the ESP32 board. Auto-detects the USB-UART bridge,
//! reconnects on failure, sends JSON lines, heartbeats every 2s.

use serialport::{SerialPort, SerialPortType};
use std::io::Write;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

#[derive(Clone, PartialEq)]
pub struct BoardMsg {
    pub state: String,
    pub chr: String,
    pub level: u8,
}

const KNOWN_BRIDGES: [(u16, u16); 4] = [
    (0x1A86, 0x55D4), // CH9102 (this project's DevKit V1)
    (0x10C4, 0xEA60), // CP2102
    (0x1A86, 0x7523), // CH340
    (0x0403, 0x6001), // FT232
];

pub fn detect_port() -> Option<String> {
    for p in serialport::available_ports().ok()? {
        if let SerialPortType::UsbPort(info) = &p.port_type {
            if KNOWN_BRIDGES.contains(&(info.vid, info.pid)) {
                return Some(p.port_name);
            }
        }
    }
    None
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

pub fn spawn(rx: Receiver<BoardMsg>, port_override: Option<String>, shared: std::sync::Arc<std::sync::Mutex<crate::Shared>>) {
    std::thread::spawn(move || {
        let mut port: Option<Box<dyn SerialPort>> = None;
        let set_conn = |on: bool| {
            if let Ok(mut s) = shared.lock() {
                s.board_connected = on;
            }
        };
        let mut last: Option<BoardMsg> = None;
        let mut last_attempt = std::time::Instant::now() - Duration::from_secs(10);
        loop {
            let msg = match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(m) => {
                    last = Some(m.clone());
                    Some(m)
                }
                Err(RecvTimeoutError::Timeout) => last.clone(), // heartbeat
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let Some(m) = msg else { continue };

            if port.is_none() && last_attempt.elapsed() > Duration::from_secs(5) {
                last_attempt = std::time::Instant::now();
                port = open(&port_override);
                set_conn(port.is_some());
            }
            if let Some(p) = port.as_mut() {
                let line = format!("{{\"s\":\"{}\",\"c\":\"{}\",\"lv\":{}}}\n", m.state, m.chr, m.level);
                if p.write_all(line.as_bytes()).is_err() {
                    eprintln!("petd: board write failed, will reconnect");
                    port = None;
                    set_conn(false);
                    continue;
                }
                // drain any acks so the OS buffer never fills
                let mut scratch = [0u8; 256];
                while p.bytes_to_read().unwrap_or(0) > 0 {
                    use std::io::Read;
                    if p.read(&mut scratch).is_err() {
                        break;
                    }
                }
            }
        }
    });
}
