//! Localhost HTTP endpoint for Claude Code hook events (via pet-hook.exe).

use crate::state::{classify_tool, Event};
use serde_json::Value;
use std::sync::mpsc::Sender;

pub const ADDR: &str = "127.0.0.1:8127";

pub fn spawn(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(ADDR) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("petd: cannot bind {ADDR}: {e} (is another petd running?)");
                std::process::exit(2);
            }
        };
        println!("petd: listening on http://{ADDR}/event");
        for mut req in server.incoming_requests() {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            if let Some(ev) = parse_event(&body) {
                let _ = tx.send(ev);
            }
            let _ = req.respond(tiny_http::Response::from_string("{\"ok\":1}"));
        }
    });
}

/// Map a Claude Code hook payload (or a manual {"s":"coding"} test message)
/// to a state-machine event.
pub fn parse_event(body: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(body).ok()?;

    // manual override for testing: {"s":"coding"}
    if let Some(name) = v.get("s").and_then(Value::as_str) {
        let idx = crate::assets_gen::STATE_NAMES.iter().position(|&n| n == name)?;
        return Some(Event::ForceState(idx));
    }

    let hook = v.get("hook_event_name").and_then(Value::as_str)?;
    match hook {
        "UserPromptSubmit" => Some(Event::Prompt),
        "SessionStart" => Some(Event::SessionStart),
        "Stop" => Some(Event::Stopped),
        "Notification" => Some(Event::PermissionWait),
        "PreToolUse" => {
            let tool = v.get("tool_name").and_then(Value::as_str).unwrap_or("");
            let cmd = v.pointer("/tool_input/command").and_then(Value::as_str);
            Some(Event::ToolStart(classify_tool(tool, cmd)))
        }
        "PostToolUse" => {
            let error = v.pointer("/tool_response/is_error").and_then(Value::as_bool).unwrap_or(false)
                || v.pointer("/tool_response/success").and_then(Value::as_bool) == Some(false);
            Some(Event::ToolEnd { error })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ToolKind;

    #[test]
    fn parses_hook_events() {
        assert_eq!(parse_event(r#"{"hook_event_name":"UserPromptSubmit","prompt":"hi"}"#), Some(Event::Prompt));
        assert_eq!(
            parse_event(r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{}}"#),
            Some(Event::ToolStart(ToolKind::Edit))
        );
        assert_eq!(
            parse_event(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"cargo test"}}"#),
            Some(Event::ToolStart(ToolKind::RunTest))
        );
        assert_eq!(
            parse_event(r#"{"hook_event_name":"PostToolUse","tool_response":{"is_error":true}}"#),
            Some(Event::ToolEnd { error: true })
        );
        assert_eq!(parse_event(r#"{"hook_event_name":"Stop"}"#), Some(Event::Stopped));
        assert_eq!(parse_event(r#"{"s":"celebrating"}"#), Some(Event::ForceState(crate::state::CELEBRATING)));
        assert_eq!(parse_event("not json"), None);
    }
}
