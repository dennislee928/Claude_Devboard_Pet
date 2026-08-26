//! What Claude Code is actually doing, per session.
//!
//! Two sources feed this:
//!   * hook payloads (session id, cwd, tool name + input, permission waits) —
//!     these arrive instantly and describe the *act the agent is taking*;
//!   * the session transcript JSONL that every hook payload points at — read
//!     incrementally for the `model` and the token `usage` of each assistant
//!     turn, which is where the *usage per session* numbers come from.
//!
//! Nothing here talks to the network; the transcript is a local file Claude
//! Code already writes.

use crate::usage::{parse_rfc3339, Ledger};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Ledger key for everything this module tracks.
pub const PROVIDER: &str = "claude";

/// Sessions with no hook traffic for this long stop being shown.
const STALE: Duration = Duration::from_secs(30 * 60);
/// …and are dropped entirely after this long.
const FORGET: Duration = Duration::from_secs(12 * 60 * 60);
/// A session counts as "busy" until Stop, or until it goes quiet this long.
const BUSY_TIMEOUT: Duration = Duration::from_secs(90);

/// Fields lifted out of one Claude Code hook payload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HookUpdate {
    pub session_id: String,
    pub cwd: String,
    pub transcript: String,
    pub hook: String,
    pub tool: String,
    /// Human-readable act, e.g. `Editing state.rs` or `$ cargo test`.
    pub action: String,
    pub busy: bool,
    pub ended: bool,
}

pub use crate::usage::Tokens;

struct Session {
    id: String,
    project: String,
    model: String,
    action: String,
    tool: String,
    prompts: u64,
    tool_calls: u64,
    tokens: Tokens,
    last: Instant,
    busy: bool,
    transcript: PathBuf,
    offset: u64,
}

/// Immutable view handed to the UI / the board.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionView {
    pub id: String,
    pub project: String,
    pub model: String,
    pub action: String,
    pub prompts: u64,
    pub tool_calls: u64,
    pub tokens: Tokens,
    pub busy: bool,
    pub idle_secs: u64,
}

pub struct Registry {
    map: HashMap<String, Session>,
    prompts: u64,
}

/// `/Users/me/code/my-app` -> `my-app`
fn project_of(cwd: &str) -> String {
    cwd.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(cwd).to_string()
}

/// `claude-opus-5-20260101` -> `Opus 5`; unknown ids pass through trimmed.
pub fn pretty_model(id: &str) -> String {
    let l = id.to_lowercase();
    let family = ["opus", "sonnet", "haiku", "fable"].iter().find(|f| l.contains(**f)).copied();
    match family {
        Some(f) => {
            // the version sits right after the family name: claude-<family>-<ver>-<date>
            let rest = l.split(f).nth(1).unwrap_or("").trim_matches('-');
            // version parts are 1-2 digits; an 8-digit chunk is the release date
            let ver: String = rest
                .split('-')
                .take_while(|p| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit()))
                .collect::<Vec<_>>()
                .join(".");
            let mut name = f.to_string();
            if let Some(c) = name.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            if ver.is_empty() {
                name
            } else {
                format!("{name} {ver}")
            }
        }
        None => id.trim_start_matches("claude-").to_string(),
    }
}

/// Describe the act an agent is taking from a hook payload.
pub fn describe(hook: &str, tool: &str, input: Option<&Value>) -> String {
    let s = |key: &str| input.and_then(|v| v.get(key)).and_then(Value::as_str).unwrap_or("");
    let base = |p: &str| p.rsplit(['/', '\\']).find(|x| !x.is_empty()).unwrap_or(p).to_string();
    let clip = |t: &str, n: usize| {
        let t = t.split('\n').next().unwrap_or(t).trim();
        if t.chars().count() > n {
            format!("{}…", t.chars().take(n).collect::<String>())
        } else {
            t.to_string()
        }
    };
    match hook {
        "UserPromptSubmit" => return "Reading your prompt".into(),
        "SessionStart" => return "Session started".into(),
        "Stop" | "SubagentStop" => return "Done — waiting for you".into(),
        "Notification" => return "Waiting for permission".into(),
        _ => {}
    }
    match tool {
        "Edit" | "MultiEdit" => format!("Editing {}", base(s("file_path"))),
        "Write" => format!("Writing {}", base(s("file_path"))),
        "NotebookEdit" => format!("Editing {}", base(s("notebook_path"))),
        "Read" => format!("Reading {}", base(s("file_path"))),
        "Bash" | "PowerShell" => format!("$ {}", clip(s("command"), 40)),
        "Grep" => format!("Grepping /{}/", clip(s("pattern"), 24)),
        "Glob" => format!("Globbing {}", clip(s("pattern"), 24)),
        "WebSearch" => format!("Searching the web: {}", clip(s("query"), 28)),
        "WebFetch" => format!("Fetching {}", clip(s("url"), 34)),
        "Task" | "Agent" => format!("Delegating to a subagent: {}", clip(s("description"), 28)),
        "TodoWrite" => "Updating its plan".into(),
        "" => "Working".into(),
        other => format!("Using {other}"),
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Registry { map: HashMap::new(), prompts: 0 }
    }

    /// Fold one hook payload into the registry.
    pub fn apply(&mut self, u: &HookUpdate, now: Instant) {
        if u.session_id.is_empty() {
            return;
        }
        let e = self.map.entry(u.session_id.clone()).or_insert_with(|| Session {
            id: u.session_id.clone(),
            project: project_of(&u.cwd),
            model: String::new(),
            action: String::new(),
            tool: String::new(),
            prompts: 0,
            tool_calls: 0,
            tokens: Tokens::default(),
            last: now,
            busy: false,
            transcript: PathBuf::from(&u.transcript),
            offset: 0,
        });
        if !u.cwd.is_empty() {
            e.project = project_of(&u.cwd);
        }
        if !u.transcript.is_empty() && e.transcript.as_os_str() != u.transcript.as_str() {
            e.transcript = PathBuf::from(&u.transcript);
            e.offset = 0;
        }
        if !u.action.is_empty() {
            e.action = u.action.clone();
        }
        if !u.tool.is_empty() {
            e.tool = u.tool.clone();
        }
        match u.hook.as_str() {
            "UserPromptSubmit" => {
                e.prompts += 1;
                self.prompts += 1;
            }
            "PreToolUse" => e.tool_calls += 1,
            _ => {}
        }
        e.busy = u.busy && !u.ended;
        e.last = now;
    }

    /// Read whatever is new in each transcript: model + token usage. Every
    /// sample is stamped with the assistant turn's own timestamp, which is
    /// what lets the ledger bucket it into the 5-hour and weekly windows.
    pub fn poll_usage(&mut self, ledger: &mut Ledger) {
        for s in self.map.values_mut() {
            if s.transcript.as_os_str().is_empty() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&s.transcript) else { continue };
            let len = meta.len();
            let mut from = ledger.offset(&s.transcript);
            if len < from {
                from = 0; // transcript rotated/compacted
            }
            if len == from {
                continue;
            }
            let mut model = s.model.clone();
            if let Some((read_to, added)) = scan_transcript(&s.transcript, from, len, ledger, Some(&mut model)) {
                ledger.set_offset(&s.transcript, read_to);
                s.offset = read_to;
                s.model = model;
                s.tokens.add(&added);
            }
        }
    }

    pub fn sessions(&mut self, now: Instant) -> Vec<SessionView> {
        self.map.retain(|_, s| now.duration_since(s.last) < FORGET);
        let mut sessions: Vec<SessionView> = self
            .map
            .values()
            .map(|s| {
                let idle = now.duration_since(s.last);
                SessionView {
                    id: s.id.chars().take(8).collect(),
                    project: s.project.clone(),
                    model: if s.model.is_empty() { "—".into() } else { s.model.clone() },
                    action: s.action.clone(),
                    prompts: s.prompts,
                    tool_calls: s.tool_calls,
                    tokens: s.tokens,
                    busy: s.busy && idle < BUSY_TIMEOUT,
                    idle_secs: idle.as_secs(),
                }
            })
            .filter(|v| v.idle_secs < STALE.as_secs() || v.tokens.total() > 0)
            .collect();
        // busy first, then most recently active
        sessions.sort_by(|a, b| b.busy.cmp(&a.busy).then(a.idle_secs.cmp(&b.idle_secs)));
        sessions
    }

    /// Prompts seen since this process started.
    pub fn prompts(&self) -> u64 {
        self.prompts
    }
}

/// Fold one transcript's new bytes into the ledger, returning how far we read.
/// Shared by live polling and the history backfill.
fn scan_transcript(path: &std::path::Path, from: u64, to: u64, ledger: &mut Ledger, model_out: Option<&mut String>) -> Option<(u64, Tokens)> {
    let (text, read_to) = read_from(path, from, to)?;
    let mut model = String::new();
    let mut total = Tokens::default();
    for line in text.lines() {
        if !line.contains("\"usage\"") && !line.contains("\"model\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(m) = v.pointer("/message/model").and_then(Value::as_str) {
            model = pretty_model(m);
        }
        if let Some(u) = v.pointer("/message/usage").or_else(|| v.get("usage")) {
            let g = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            let t = Tokens {
                input: g("input_tokens"),
                output: g("output_tokens"),
                cache_read: g("cache_read_input_tokens"),
                cache_write: g("cache_creation_input_tokens"),
            };
            if t.total() > 0 {
                let at = v.get("timestamp").and_then(Value::as_str).and_then(parse_rfc3339).unwrap_or_else(crate::usage::now_unix);
                ledger.record(at, PROVIDER, &model, t);
                total.add(&t);
            }
        }
    }
    if let Some(out) = model_out {
        if !model.is_empty() {
            *out = model;
        }
    }
    Some((read_to, total))
}

/// Fill the weekly window from transcripts Claude Code already wrote, so the
/// panel is correct the first time it opens rather than after a week of
/// running. The ledger's persisted offsets keep this from double counting on
/// the next start.
pub fn backfill(ledger: &mut Ledger) {
    let root = crate::paths::claude_projects_dir();
    let cutoff = crate::usage::now_unix().saturating_sub(8 * 24 * 3600);
    let Ok(projects) = std::fs::read_dir(&root) else { return };
    for project in projects.flatten() {
        let Ok(files) = std::fs::read_dir(project.path()) else { continue };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let Ok(md) = f.metadata() else { continue };
            let touched = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if touched < cutoff {
                continue;
            }
            let from = ledger.offset(&path);
            let len = md.len();
            if len <= from {
                continue;
            }
            if let Some((read_to, _)) = scan_transcript(&path, from, len, ledger, None) {
                ledger.set_offset(&path, read_to);
            }
        }
    }
}

/// Read `[from, to)` of a file as UTF-8, trimming a trailing partial line.
fn read_from(path: &std::path::Path, from: u64, to: u64) -> Option<(String, u64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = vec![0u8; (to - from).min(4 << 20) as usize];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    // only keep whole lines; the rest is re-read next tick
    let cut = buf.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
    buf.truncate(cut);
    Some((String::from_utf8_lossy(&buf).into_owned(), from + cut as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_names_are_readable() {
        assert_eq!(pretty_model("claude-opus-5-20260101"), "Opus 5");
        assert_eq!(pretty_model("claude-sonnet-4-5-20250929"), "Sonnet 4.5");
        assert_eq!(pretty_model("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(pretty_model("some-other-model"), "some-other-model");
    }

    #[test]
    fn actions_read_like_sentences() {
        let inp = json!({"file_path": "/a/b/state.rs"});
        assert_eq!(describe("PreToolUse", "Edit", Some(&inp)), "Editing state.rs");
        let inp = json!({"command": "cargo test -p petd"});
        assert_eq!(describe("PreToolUse", "Bash", Some(&inp)), "$ cargo test -p petd");
        assert_eq!(describe("Notification", "", None), "Waiting for permission");
        assert_eq!(describe("Stop", "", None), "Done — waiting for you");
    }

    #[test]
    fn tracks_two_sessions_independently() {
        let now = Instant::now();
        let mut r = Registry::new();
        for (id, proj) in [("aaa", "/x/app"), ("bbb", "/x/lib")] {
            r.apply(
                &HookUpdate {
                    session_id: id.into(),
                    cwd: proj.into(),
                    hook: "UserPromptSubmit".into(),
                    action: "Reading your prompt".into(),
                    busy: true,
                    ..Default::default()
                },
                now,
            );
        }
        let sessions = r.sessions(now);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions.iter().filter(|s| s.busy).count(), 2);
        assert_eq!(r.prompts(), 2);
        assert!(sessions.iter().any(|s| s.project == "app"));
        assert!(sessions.iter().any(|s| s.project == "lib"));
    }

    #[test]
    fn usage_is_summed_from_the_transcript_into_the_ledger() {
        let dir = std::env::temp_dir().join(format!("devpet-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tp = dir.join("t.jsonl");
        std::fs::write(
            &tp,
            "{\"timestamp\":\"2026-08-25T10:00:00.000Z\",\"message\":{\"model\":\"claude-opus-5-20260101\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":100}}}\n",
        )
        .unwrap();
        let now = Instant::now();
        let mut r = Registry::new();
        let mut ledger = Ledger::default();
        r.apply(
            &HookUpdate { session_id: "s1".into(), transcript: tp.display().to_string(), hook: "PreToolUse".into(), busy: true, ..Default::default() },
            now,
        );
        r.poll_usage(&mut ledger);
        let s = &r.sessions(now)[0];
        assert_eq!(s.model, "Opus 5");
        assert_eq!(s.tokens.total(), 115);
        // the sample landed in the ledger stamped with the transcript's own time
        assert_eq!(ledger.samples.len(), 1);
        assert_eq!(ledger.samples[0].at, crate::usage::parse_rfc3339("2026-08-25T10:00:00.000Z").unwrap());
        assert_eq!(ledger.samples[0].model, "Opus 5");
        assert_eq!(ledger.lifetime.output, 5);
        // a second poll with no new bytes must not double count
        r.poll_usage(&mut ledger);
        assert_eq!(r.sessions(now)[0].tokens.total(), 115);
        assert_eq!(ledger.samples.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
