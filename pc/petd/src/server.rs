//! Localhost HTTP endpoint for Claude Code hook events (via pet-hook).
//!
//! One hook payload yields up to two things: a state-machine `Event` (what the
//! pet should *do*) and a `HookUpdate` (what the session status panel should
//! *show*). Both editions of the app use this same server.

use crate::sessions::{describe, HookUpdate};
use crate::state::{classify_tool, Event};
use serde_json::Value;
use std::sync::mpsc::Sender;

pub const ADDR: &str = "127.0.0.1:8127";

/// One parsed message from a hook (or from the desktop UI).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Incoming {
    pub ev: Option<Event>,
    pub hook: Option<HookUpdate>,
}

impl From<Event> for Incoming {
    fn from(e: Event) -> Self {
        Incoming { ev: Some(e), hook: None }
    }
}

pub fn spawn(tx: Sender<Incoming>) {
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
            if let Some(msg) = parse(&body) {
                let _ = tx.send(msg);
            }
            let _ = req.respond(tiny_http::Response::from_string("{\"ok\":1}"));
        }
    });
}

/// Map a Claude Code hook payload (or a manual {"s":"coding"} test message)
/// to an event plus the session status it implies.
pub fn parse(body: &str) -> Option<Incoming> {
    let v: Value = serde_json::from_str(body).ok()?;

    // manual override for testing: {"s":"coding"}
    if let Some(name) = v.get("s").and_then(Value::as_str) {
        let idx = crate::assets_gen::STATE_NAMES.iter().position(|&n| n == name)?;
        return Some(Event::ForceState(idx).into());
    }

    let hook = v.get("hook_event_name").and_then(Value::as_str)?;
    let tool = v.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let tool_input = v.get("tool_input");
    let str_at = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();

    let ev = match hook {
        "UserPromptSubmit" => Some(Event::Prompt),
        "SessionStart" => Some(Event::SessionStart),
        "Stop" => Some(Event::Stopped),
        "SubagentStop" => Some(Event::SubagentDone),
        "Notification" => Some(Event::PermissionWait),
        "PreToolUse" => {
            let cmd = v.pointer("/tool_input/command").and_then(Value::as_str);
            Some(Event::ToolStart(classify_tool(tool, cmd)))
        }
        "PostToolUse" => {
            let error = v.pointer("/tool_response/is_error").and_then(Value::as_bool).unwrap_or(false)
                || v.pointer("/tool_response/success").and_then(Value::as_bool) == Some(false);
            Some(Event::ToolEnd { error })
        }
        _ => None,
    };

    let update = HookUpdate {
        session_id: str_at("session_id"),
        cwd: str_at("cwd"),
        transcript: str_at("transcript_path"),
        hook: hook.to_string(),
        tool: tool.to_string(),
        action: describe(hook, tool, tool_input),
        busy: !matches!(hook, "Stop" | "Notification"),
        ended: hook == "SessionEnd",
    };

    if ev.is_none() && update.session_id.is_empty() {
        return None;
    }
    Some(Incoming { ev, hook: Some(update) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ToolKind;

    fn ev(body: &str) -> Option<Event> {
        parse(body).and_then(|i| i.ev)
    }

    #[test]
    fn parses_hook_events() {
        assert_eq!(ev(r#"{"hook_event_name":"UserPromptSubmit","prompt":"hi"}"#), Some(Event::Prompt));
        assert_eq!(
            ev(r#"{"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{}}"#),
            Some(Event::ToolStart(ToolKind::Edit))
        );
        assert_eq!(
            ev(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"cargo test"}}"#),
            Some(Event::ToolStart(ToolKind::RunTest))
        );
        assert_eq!(ev(r#"{"hook_event_name":"PostToolUse","tool_response":{"is_error":true}}"#), Some(Event::ToolEnd { error: true }));
        assert_eq!(ev(r#"{"hook_event_name":"Stop"}"#), Some(Event::Stopped));
        assert_eq!(ev(r#"{"hook_event_name":"SubagentStop"}"#), Some(Event::SubagentDone));
        assert_eq!(ev(r#"{"s":"celebrating"}"#), Some(Event::ForceState(crate::state::CELEBRATING)));
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn carries_session_status() {
        let msg = parse(
            r#"{"hook_event_name":"PreToolUse","session_id":"abc123","cwd":"/home/me/app",
                "transcript_path":"/t.jsonl","tool_name":"Edit","tool_input":{"file_path":"/home/me/app/src/main.rs"}}"#,
        )
        .unwrap();
        let h = msg.hook.unwrap();
        assert_eq!(h.session_id, "abc123");
        assert_eq!(h.action, "Editing main.rs");
        assert!(h.busy);
    }
}
