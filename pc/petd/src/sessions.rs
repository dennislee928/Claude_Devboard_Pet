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

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
    fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }
}

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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub sessions: Vec<SessionView>,
    pub active: usize,
    pub session_tokens: Tokens,
    /// Everything DevPet has ever seen, persisted across restarts.
    pub lifetime_tokens: Tokens,
    pub lifetime_prompts: u64,
}

impl Snapshot {
    /// The single most interesting session (busy first, then most recent).
    pub fn focus(&self) -> Option<&SessionView> {
        self.sessions.iter().find(|s| s.busy).or_else(|| self.sessions.first())
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    lifetime: Tokens,
    prompts: u64,
}

pub struct Registry {
    map: HashMap<String, Session>,
    persisted: Persisted,
    dirty: bool,
}

fn store_path() -> PathBuf {
    crate::paths::state_dir().join("usage.json")
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
            let ver: String = rest.split('-').take(2).filter(|p| p.chars().all(|c| c.is_ascii_digit())).collect::<Vec<_>>().join(".");
            let mut name = f.to_string();
            name.get_mut(0..1).map(|c| c.make_ascii_uppercase());
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

impl Registry {
    pub fn load() -> Self {
        let persisted = std::fs::read_to_string(store_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Registry { map: HashMap::new(), persisted, dirty: false }
    }

    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        crate::paths::ensure_state_dir();
        if let Ok(s) = serde_json::to_string_pretty(&self.persisted) {
            let _ = std::fs::write(store_path(), s);
        }
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
                self.persisted.prompts += 1;
                self.dirty = true;
            }
            "PreToolUse" => e.tool_calls += 1,
            _ => {}
        }
        e.busy = u.busy && !u.ended;
        e.last = now;
    }

    /// Read whatever is new in each transcript: model + token usage.
    pub fn poll_usage(&mut self) {
        let mut lifetime_delta = Tokens::default();
        for s in self.map.values_mut() {
            if s.transcript.as_os_str().is_empty() {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&s.transcript) else { continue };
            let len = meta.len();
            if len < s.offset {
                s.offset = 0; // transcript rotated/compacted
            }
            if len == s.offset {
                continue;
            }
            let Some((text, read_to)) = read_from(&s.transcript, s.offset, len) else { continue };
            s.offset = read_to;
            for line in text.lines() {
                if !line.contains("\"usage\"") && !line.contains("\"model\"") {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                if let Some(m) = v.pointer("/message/model").and_then(Value::as_str) {
                    s.model = pretty_model(m);
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
                        s.tokens.add(&t);
                        lifetime_delta.add(&t);
                    }
                }
            }
        }
        if lifetime_delta.total() > 0 {
            self.persisted.lifetime.add(&lifetime_delta);
            self.dirty = true;
        }
    }

    pub fn snapshot(&mut self, now: Instant) -> Snapshot {
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
        let mut session_tokens = Tokens::default();
        for s in &sessions {
            session_tokens.add(&s.tokens);
        }
        Snapshot {
            active: sessions.iter().filter(|s| s.busy).count(),
            session_tokens,
            lifetime_tokens: self.persisted.lifetime,
            lifetime_prompts: self.persisted.prompts,
            sessions,
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
        let mut r = Registry { map: HashMap::new(), persisted: Persisted::default(), dirty: false };
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
        let snap = r.snapshot(now);
        assert_eq!(snap.sessions.len(), 2);
        assert_eq!(snap.active, 2);
        assert_eq!(snap.lifetime_prompts, 2);
        assert!(snap.sessions.iter().any(|s| s.project == "app"));
        assert!(snap.sessions.iter().any(|s| s.project == "lib"));
    }

    #[test]
    fn usage_is_summed_from_the_transcript() {
        let dir = std::env::temp_dir().join(format!("devpet-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tp = dir.join("t.jsonl");
        std::fs::write(
            &tp,
            "{\"message\":{\"model\":\"claude-opus-5-20260101\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":100}}}\n",
        )
        .unwrap();
        let now = Instant::now();
        let mut r = Registry { map: HashMap::new(), persisted: Persisted::default(), dirty: false };
        r.apply(
            &HookUpdate { session_id: "s1".into(), transcript: tp.display().to_string(), hook: "PreToolUse".into(), busy: true, ..Default::default() },
            now,
        );
        r.poll_usage();
        let snap = r.snapshot(now);
        let s = &snap.sessions[0];
        assert_eq!(s.model, "Opus 5");
        assert_eq!(s.tokens.total(), 115);
        assert_eq!(snap.lifetime_tokens.output, 5);
        // a second poll with no new bytes must not double count
        r.poll_usage();
        assert_eq!(r.snapshot(now).sessions[0].tokens.total(), 115);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
