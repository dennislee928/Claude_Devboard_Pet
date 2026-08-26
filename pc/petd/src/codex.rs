//! Codex CLI provider.
//!
//! Codex has no hook system, so instead of being pushed events like Claude
//! Code does, we watch the rollout transcripts it writes under
//! `~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-*.jsonl` and tail them.
//!
//! Those files carry everything the panel needs, and more than the Claude side
//! can offer: `session_meta` (session id, cwd, provider), `thread_settings`
//! (the model), `event_msg/token_count` (token totals *and* the real
//! `rate_limits` — a genuine percentage of the plan's 5-hour and weekly
//! windows, with reset times), and the tool calls that say what the agent is
//! doing right now.
//!
//! Because the same file also says when a turn starts and finishes, Codex can
//! drive the pet's state machine exactly like Claude Code's hooks do.

use crate::sessions::SessionView;
use crate::state::{classify_bash, Event, ToolKind};
use crate::usage::{parse_rfc3339, Ledger, Source, Tokens, Window};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const PROVIDER: &str = "codex";

/// Rollouts untouched for this long are not shown as sessions any more.
const RECENT_SECS: u64 = 12 * 3600;
/// …but the first poll reaches back a full week, so the weekly usage window is
/// right immediately instead of filling up over the next seven days.
const BACKFILL_SECS: u64 = 8 * 24 * 3600;
/// A session counts as working until its turn completes or it goes quiet.
const BUSY_TIMEOUT: u64 = 90;

pub fn sessions_dir() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
            home.join(".codex")
        })
        .join("sessions")
}

/// Is Codex installed at all? Used to hide the provider when it is not.
pub fn is_present() -> bool {
    sessions_dir().is_dir()
}

#[derive(Default)]
struct Rollout {
    id: String,
    project: String,
    model: String,
    action: String,
    turns: u64,
    tool_calls: u64,
    tokens: Tokens,
    last_at: u64,
    busy: bool,
    offset: u64,
}

/// What Codex reports about the plan's own limits — a real percentage, unlike
/// anything we can compute ourselves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reported {
    pub windows: Vec<Window>,
    pub plan: Option<String>,
}

#[derive(Default)]
pub struct Watcher {
    files: HashMap<PathBuf, Rollout>,
    reported: Reported,
    /// Events derived from the tail, for the pet's state machine.
    pending: Vec<Event>,
    scanned_history: bool,
}

/// `{"cmd":"ls -la","workdir":…}` inside an exec tool call.
fn command_of(args: &str) -> Option<String> {
    let start = args.find("\"cmd\"")?;
    let rest = &args[start + 5..];
    let q = rest.find('"')?;
    let tail = &rest[q + 1..];
    let end = tail.find('"')?;
    Some(tail[..end].to_string())
}

fn short(s: &str, n: usize) -> String {
    let line = s.split('\n').next().unwrap_or(s).trim();
    if line.chars().count() > n {
        format!("{}…", line.chars().take(n).collect::<String>())
    } else {
        line.to_string()
    }
}

fn project_of(cwd: &str) -> String {
    cwd.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(cwd).to_string()
}

fn tokens_from(v: &Value) -> Tokens {
    let g = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Tokens {
        input: g("input_tokens").saturating_sub(g("cached_input_tokens")),
        output: g("output_tokens"),
        cache_read: g("cached_input_tokens"),
        cache_write: 0,
    }
}

/// Turn one rollout line into whatever it says about the session.
fn apply_line(r: &mut Rollout, line: &str, ledger: &mut Ledger, reported: &mut Reported, events: &mut Vec<Event>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    let at = v.get("timestamp").and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or(r.last_at);
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    let pl = v.get("payload").unwrap_or(&Value::Null);
    r.last_at = r.last_at.max(at);

    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();

    match kind {
        "session_meta" => {
            r.id = s(pl, "session_id");
            r.project = project_of(&s(pl, "cwd"));
        }
        "turn_context" => {
            let cwd = s(pl, "cwd");
            if !cwd.is_empty() {
                r.project = project_of(&cwd);
            }
        }
        "event_msg" => {
            let t = s(pl, "type");
            match t.as_str() {
                "task_started" => {
                    r.turns += 1;
                    r.busy = true;
                    r.action = "Thinking".into();
                    events.push(Event::Prompt);
                }
                "task_complete" | "turn_aborted" => {
                    r.busy = false;
                    r.action = "Done — waiting for you".into();
                    events.push(Event::Stopped);
                }
                "user_message" => {
                    r.action = "Reading your prompt".into();
                }
                "agent_message" => {
                    r.action = "Writing a reply".into();
                }
                "web_search_end" => {
                    r.action = "Searching the web".into();
                    events.push(Event::ToolStart(ToolKind::Search));
                }
                "patch_apply_end" => {
                    let ok = pl.get("success").and_then(Value::as_bool).unwrap_or(true);
                    events.push(Event::ToolEnd { error: !ok });
                }
                "exec_command_end" | "mcp_tool_call_end" => {
                    let ok = pl.get("success").and_then(Value::as_bool).unwrap_or(true)
                        && pl.get("exit_code").and_then(Value::as_i64).unwrap_or(0) == 0;
                    events.push(Event::ToolEnd { error: !ok });
                }
                "token_count" => {
                    if let Some(info) = pl.get("info") {
                        // `last_token_usage` is this turn only; the totals field
                        // is cumulative and would double count.
                        let t = tokens_from(info.get("last_token_usage").unwrap_or(&Value::Null));
                        if !t.is_zero() {
                            ledger.record(at, PROVIDER, &r.model, t);
                            r.tokens.add(&t);
                        }
                    }
                    if let Some(rl) = pl.get("rate_limits") {
                        *reported = parse_rate_limits(rl, reported.clone());
                    }
                }
                _ => {}
            }
        }
        "response_item" => {
            let t = s(pl, "type");
            match t.as_str() {
                "function_call" | "custom_tool_call" => {
                    r.tool_calls += 1;
                    let name = s(pl, "name");
                    let args = pl.get("arguments").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| s(pl, "input"));
                    let (action, kind) = describe_call(&name, &args);
                    r.action = action;
                    events.push(Event::ToolStart(kind));
                }
                "reasoning" => r.action = "Thinking".into(),
                _ => {}
            }
        }
        _ => {}
    }

    // the model can appear in several places depending on Codex's version
    for key in ["model", "model_slug"] {
        if let Some(m) = find_str(pl, key) {
            r.model = crate::sessions::pretty_model(&m);
            break;
        }
    }
}

/// Describe a Codex tool call the way the Claude side describes a hook.
pub fn describe_call(name: &str, args: &str) -> (String, ToolKind) {
    match name {
        "exec" | "shell" | "exec_command" | "local_shell" => match command_of(args) {
            Some(cmd) => (format!("$ {}", short(&cmd, 40)), classify_bash(&cmd)),
            None => ("Running a command".into(), ToolKind::Other),
        },
        "apply_patch" | "edit" | "write_file" => ("Editing files".into(), ToolKind::Edit),
        "update_plan" => ("Updating its plan".into(), ToolKind::Other),
        "web_search" | "search" => ("Searching the web".into(), ToolKind::Search),
        "view_image" | "read_file" => ("Reading a file".into(), ToolKind::Search),
        "" => ("Working".into(), ToolKind::Other),
        other if other.starts_with("mcp") => (format!("Calling {other}"), ToolKind::Other),
        other => (format!("Using {other}"), ToolKind::Other),
    }
}

/// Depth-limited search for a string field, since Codex nests the model name
/// under thread_settings / collaboration_mode depending on version.
fn find_str(v: &Value, key: &str) -> Option<String> {
    fn go(v: &Value, key: &str, depth: usize) -> Option<String> {
        if depth > 4 {
            return None;
        }
        match v {
            Value::Object(m) => {
                if let Some(Value::String(s)) = m.get(key) {
                    return Some(s.clone());
                }
                m.iter().filter(|(k, _)| *k != "base_instructions").find_map(|(_, x)| go(x, key, depth + 1))
            }
            _ => None,
        }
    }
    go(v, key, 0)
}

fn parse_rate_limits(rl: &Value, mut prev: Reported) -> Reported {
    let mut windows = Vec::new();
    for key in ["primary", "secondary"] {
        let Some(w) = rl.get(key).filter(|w| !w.is_null()) else { continue };
        let used = w.get("used_percent").and_then(Value::as_f64);
        let mins = w.get("window_minutes").and_then(Value::as_u64).unwrap_or(0);
        let label = match mins {
            0 => key.to_string(),
            m if m <= 60 => format!("{m}m"),
            m if m < 1440 => format!("{}h", m / 60),
            10080 => "Week".to_string(),
            m => format!("{}d", m / 1440),
        };
        windows.push(Window {
            label,
            tokens: Tokens::default(),
            used_percent: used.map(|u| u as f32),
            source: Source::Reported,
            resets_at: w.get("resets_at").and_then(Value::as_u64),
        });
    }
    if !windows.is_empty() {
        prev.windows = windows;
    }
    if let Some(p) = rl.get("plan_type").and_then(Value::as_str) {
        prev.plan = Some(p.to_string());
    }
    prev
}

/// Every rollout file touched within `max_age`, oldest first.
fn recent_rollouts(now: u64, max_age: u64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let root = sessions_dir();
    // sessions/<yyyy>/<mm>/<dd>/rollout-*.jsonl — walk at most that深 deep
    fn walk(dir: &Path, depth: usize, now: u64, max_age: u64, out: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                walk(&p, depth + 1, now, max_age, out);
            } else if p.extension().is_some_and(|x| x == "jsonl") {
                let age = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| now.saturating_sub(d.as_secs()))
                    .unwrap_or(u64::MAX);
                if age <= max_age {
                    out.push(p);
                }
            }
        }
    }
    walk(&root, 0, now, max_age, &mut out);
    out.sort();
    out
}

impl Watcher {
    /// Read whatever is new in every recent rollout. Returns the events that
    /// happened since the last poll, for the pet's state machine.
    pub fn poll(&mut self, now: u64, ledger: &mut Ledger) -> Vec<Event> {
        self.pending.clear();
        // Reach back a week the first time, then only at today's rollouts.
        let max_age = if self.scanned_history { RECENT_SECS } else { BACKFILL_SECS };
        self.scanned_history = true;
        for path in recent_rollouts(now, max_age) {
            let Ok(md) = std::fs::metadata(&path) else { continue };
            let len = md.len();
            // The ledger remembers how far into this file we already counted,
            // across restarts, so history is never double counted.
            let mut start = ledger.offset(&path);
            if len < start {
                start = 0; // truncated / replaced
            }
            if len == start {
                continue;
            }
            let Some((text, read_to)) = read_from(&path, start, len) else { continue };
            let first_read = start == 0;
            let entry = self.files.entry(path.clone()).or_default();
            entry.offset = read_to;
            ledger.set_offset(&path, read_to);
            let mut events = Vec::new();
            for line in text.lines() {
                apply_line(entry, line, ledger, &mut self.reported, &mut events);
            }
            // Replaying a whole pre-existing file must not make the pet act out
            // a session that already finished.
            if !first_read {
                self.pending.append(&mut events);
            }
            if entry.id.is_empty() {
                entry.id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            }
        }
        self.files.retain(|_, r| now.saturating_sub(r.last_at) <= RECENT_SECS);
        std::mem::take(&mut self.pending)
    }

    pub fn reported(&self) -> &Reported {
        &self.reported
    }

    pub fn sessions(&self, now: u64) -> Vec<SessionView> {
        let mut v: Vec<SessionView> = self
            .files
            .values()
            .filter(|r| !r.id.is_empty())
            .map(|r| {
                let idle = now.saturating_sub(r.last_at);
                SessionView {
                    id: r.id.chars().take(8).collect(),
                    project: r.project.clone(),
                    model: if r.model.is_empty() { "—".into() } else { r.model.clone() },
                    action: r.action.clone(),
                    prompts: r.turns,
                    tool_calls: r.tool_calls,
                    tokens: r.tokens,
                    busy: r.busy && idle < BUSY_TIMEOUT,
                    idle_secs: idle,
                }
            })
            .collect();
        v.sort_by(|a, b| b.busy.cmp(&a.busy).then(a.idle_secs.cmp(&b.idle_secs)));
        v
    }
}

fn read_from(path: &Path, from: u64, to: u64) -> Option<(String, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = vec![0u8; (to - from).min(8 << 20) as usize];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    let cut = buf.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
    buf.truncate(cut);
    Some((String::from_utf8_lossy(&buf).into_owned(), from + cut as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(lines: &[&str]) -> (Rollout, Ledger, Reported, Vec<Event>) {
        let mut r = Rollout::default();
        let mut l = Ledger::default();
        let mut rep = Reported::default();
        let mut ev = Vec::new();
        for line in lines {
            apply_line(&mut r, line, &mut l, &mut rep, &mut ev);
        }
        (r, l, rep, ev)
    }

    #[test]
    fn reads_a_real_rollout_shape() {
        let (r, l, _, ev) = feed(&[
            r#"{"timestamp":"2026-07-24T09:24:29.870Z","type":"session_meta","payload":{"session_id":"019f9370-c82e","cwd":"/Users/me/Documents/GitHub/chip_whisper","model_provider":"openai"}}"#,
            r#"{"timestamp":"2026-07-24T09:24:30.000Z","type":"event_msg","payload":{"type":"task_started","model_context_window":258400}}"#,
            r#"{"timestamp":"2026-07-24T09:24:31.000Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"const r = await tools.exec_command({\"cmd\":\"cargo test -p petd\",\"workdir\":\"/x\"})"}}"#,
            r#"{"timestamp":"2026-07-24T09:24:40.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":23593,"cached_input_tokens":593,"output_tokens":214}}}}"#,
            r#"{"timestamp":"2026-07-24T09:24:41.000Z","type":"event_msg","payload":{"type":"task_complete"}}"#,
        ]);
        assert_eq!(r.id, "019f9370-c82e");
        assert_eq!(r.project, "chip_whisper");
        assert_eq!(r.tool_calls, 1);
        assert_eq!(r.turns, 1);
        assert!(!r.busy); // task_complete arrived
        assert_eq!(r.action, "Done — waiting for you");
        // cached input is split out, never counted as fresh input
        assert_eq!(r.tokens.input, 23_000);
        assert_eq!(r.tokens.cache_read, 593);
        assert_eq!(r.tokens.output, 214);
        assert_eq!(l.samples.len(), 1);
        assert_eq!(l.samples[0].provider, "codex");
        // the shell command drives the pet just like a Claude Bash hook
        assert!(ev.contains(&Event::ToolStart(ToolKind::RunTest)));
        assert!(ev.contains(&Event::Stopped));
    }

    #[test]
    fn takes_the_model_from_wherever_codex_put_it() {
        let (r, _, _, _) = feed(&[
            r#"{"timestamp":"2026-07-24T09:24:29.870Z","type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"model":"gpt-5.6-sol","collaboration_mode":{"settings":{"model":"gpt-5.6-sol"}}}}}"#,
        ]);
        assert_eq!(r.model, "gpt-5.6-sol");
    }

    #[test]
    fn rate_limits_are_the_providers_own_numbers() {
        let (_, _, rep, _) = feed(&[
            r#"{"timestamp":"2026-07-24T09:24:40.000Z","type":"event_msg","payload":{"type":"token_count",
                "rate_limits":{"limit_id":"codex","primary":{"used_percent":17.0,"window_minutes":10080,"resets_at":1785286604},
                "secondary":{"used_percent":4.5,"window_minutes":300,"resets_at":1785200000},"plan_type":"plus"}}}"#,
        ]);
        assert_eq!(rep.plan.as_deref(), Some("plus"));
        assert_eq!(rep.windows.len(), 2);
        assert_eq!(rep.windows[0].label, "Week");
        assert_eq!(rep.windows[0].used_percent, Some(17.0));
        assert_eq!(rep.windows[0].source, Source::Reported);
        assert_eq!(rep.windows[1].label, "5h");
        assert_eq!(rep.windows[1].resets_at, Some(1785200000));
    }

    #[test]
    fn describes_calls_the_way_the_panel_shows_them() {
        assert_eq!(
            describe_call("exec", r#"{"cmd":"pytest -q","workdir":"/x"}"#),
            ("$ pytest -q".into(), ToolKind::RunTest)
        );
        assert_eq!(describe_call("apply_patch", "{}").1, ToolKind::Edit);
        assert_eq!(describe_call("update_plan", "{}").0, "Updating its plan");
    }
}
