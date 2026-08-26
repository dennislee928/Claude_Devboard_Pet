//! Every coding agent the pet watches, behind one shape.
//!
//! Claude Code pushes hook events at us; Codex is tailed off its rollout
//! files. Both end up as a `ProviderView` so the status panel, the board
//! screen and the pet's state machine do not care which one they are looking
//! at, and the user can run either, both, or pick which one drives the pet.

use crate::codex;
use crate::sessions::{self, HookUpdate, Registry, SessionView};
use crate::state::Event;
use crate::usage::{now_unix, Budgets, Ledger, ModelUsage, Source, Tokens, Window, FIVE_HOURS, ONE_WEEK};
use std::path::PathBuf;
use std::time::Instant;

pub const CLAUDE: &str = sessions::PROVIDER;
pub const CODEX: &str = codex::PROVIDER;
pub const ALL: [&str; 2] = [CLAUDE, CODEX];

pub fn display_name(id: &str) -> &'static str {
    match id {
        CODEX => "Codex",
        _ => "Claude Code",
    }
}

/// One provider as the panel shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    /// Is this agent installed / has it ever been seen?
    pub present: bool,
    pub sessions: Vec<SessionView>,
    pub active: usize,
    /// Tokens for the session in focus — "current session" in the panel.
    pub current_session: Tokens,
    /// 5-hour and weekly windows. Reported by the provider where it tells us
    /// (Codex), estimated against the user's budget where it does not (Claude).
    pub windows: Vec<Window>,
    /// This week, per model — including Fable.
    pub models: Vec<ModelUsage>,
    pub lifetime: Tokens,
    pub plan: Option<String>,
}

impl ProviderView {
    pub fn focus(&self) -> Option<&SessionView> {
        self.sessions.iter().find(|s| s.busy).or_else(|| self.sessions.first())
    }
    pub fn window(&self, label: &str) -> Option<&Window> {
        self.windows.iter().find(|w| w.label == label)
    }
    /// Weekly usage for one model, e.g. Fable.
    pub fn model(&self, name: &str) -> Option<&ModelUsage> {
        self.models.iter().find(|m| m.model.eq_ignore_ascii_case(name))
    }
}

/// What the UI reads every frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub providers: Vec<ProviderView>,
    /// Which provider drives the pet's state and the board screen.
    pub primary: String,
    pub now: u64,
}

impl Snapshot {
    pub fn get(&self, id: &str) -> Option<&ProviderView> {
        self.providers.iter().find(|p| p.id == id)
    }
    pub fn primary_view(&self) -> Option<&ProviderView> {
        self.get(&self.primary).or_else(|| self.providers.first())
    }
    /// The one session worth showing on a 240x240 screen.
    pub fn focus(&self) -> Option<&SessionView> {
        self.providers
            .iter()
            .flat_map(|p| p.sessions.iter())
            .find(|s| s.busy)
            .or_else(|| self.primary_view().and_then(|p| p.focus()))
    }
    /// Agents working right now, across every enabled provider.
    pub fn active(&self) -> usize {
        self.providers.iter().map(|p| p.active).sum()
    }
    pub fn session_tokens(&self) -> Tokens {
        let mut t = Tokens::default();
        for p in &self.providers {
            t.add(&p.current_session);
        }
        t
    }
}

fn ledger_path() -> PathBuf {
    crate::paths::state_dir().join("usage.json")
}

/// Owns every provider plus the shared token ledger.
pub struct Hub {
    claude: Registry,
    codex: codex::Watcher,
    ledger: Ledger,
    budgets: Budgets,
    enabled: Vec<String>,
    primary: String,
    dirty: bool,
    last_save: Instant,
    backfilled: bool,
}

impl Hub {
    pub fn new(enabled: Vec<String>, primary: String, budgets: Budgets) -> Self {
        let ledger = std::fs::read_to_string(ledger_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Hub {
            claude: Registry::new(),
            codex: codex::Watcher::default(),
            ledger,
            budgets,
            enabled,
            primary,
            dirty: false,
            last_save: Instant::now(),
            backfilled: false,
        }
    }

    pub fn set_providers(&mut self, enabled: Vec<String>, primary: String) {
        self.enabled = enabled;
        self.primary = primary;
    }

    pub fn set_budgets(&mut self, b: Budgets) {
        self.budgets = b;
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.iter().any(|e| e == id)
    }

    /// Fold in a Claude Code hook payload.
    pub fn apply_hook(&mut self, u: &HookUpdate, now: Instant) {
        if self.is_enabled(CLAUDE) {
            self.claude.apply(u, now);
        }
    }

    /// Poll every enabled provider. Returns the events Codex generated, which
    /// the caller feeds to the pet only when Codex is the primary provider —
    /// Claude Code's own events arrive through the hook server instead.
    pub fn poll(&mut self) -> Vec<Event> {
        let unix = now_unix();
        if self.is_enabled(CLAUDE) {
            // On the first pass, read a week of existing transcripts so the
            // weekly window is right immediately.
            if !self.backfilled {
                sessions::backfill(&mut self.ledger);
            }
            self.claude.poll_usage(&mut self.ledger);
        }
        self.backfilled = true;
        let mut events = Vec::new();
        if self.is_enabled(CODEX) {
            events = self.codex.poll(unix, &mut self.ledger);
        }
        self.dirty = true;
        if self.ledger.prune(unix) || self.last_save.elapsed().as_secs() >= 30 {
            self.save();
        }
        if self.primary != CODEX {
            events.clear();
        }
        events
    }

    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.last_save = Instant::now();
        crate::paths::ensure_state_dir();
        if let Ok(s) = serde_json::to_string(&self.ledger) {
            let _ = std::fs::write(ledger_path(), s);
        }
    }

    pub fn snapshot(&mut self, now: Instant) -> Snapshot {
        let unix = now_unix();
        let mut providers = Vec::new();

        if self.is_enabled(CLAUDE) {
            let sessions = self.claude.sessions(now);
            providers.push(self.view(CLAUDE, sessions, unix, None));
        }
        if self.is_enabled(CODEX) {
            let sessions = self.codex.sessions(unix);
            let reported = self.codex.reported().clone();
            providers.push(self.view(CODEX, sessions, unix, Some(reported)));
        }
        Snapshot { providers, primary: self.primary.clone(), now: unix }
    }

    fn view(&self, id: &str, sessions: Vec<SessionView>, unix: u64, reported: Option<codex::Reported>) -> ProviderView {
        // A provider's own numbers always win over anything we estimate; where
        // it reports only some windows, ours fill the gaps.
        let mut windows = self.ledger.windows(id, unix, &self.budgets);
        let mut plan = None;
        if let Some(rep) = reported {
            plan = rep.plan;
            for r in rep.windows {
                let measured = match r.label.as_str() {
                    "5h" => self.ledger.window(id, unix, FIVE_HOURS),
                    "Week" => self.ledger.window(id, unix, ONE_WEEK),
                    _ => Tokens::default(),
                };
                let filled = Window { tokens: measured, ..r };
                match windows.iter_mut().find(|w| w.label == filled.label) {
                    Some(slot) => *slot = filled,
                    None => windows.push(filled),
                }
            }
        }
        let present = match id {
            CODEX => codex::is_present(),
            _ => true,
        };
        ProviderView {
            id: id.to_string(),
            name: display_name(id).to_string(),
            present,
            current_session: sessions.first().map(|s| s.tokens).unwrap_or_default(),
            active: sessions.iter().filter(|s| s.busy).count(),
            sessions,
            models: self.ledger.by_model(id, unix, ONE_WEEK, &self.budgets),
            windows,
            lifetime: self.ledger.lifetime,
            plan,
        }
    }
}

/// How a window should be labelled in the UI, given where its number came from.
pub fn source_note(source: Source) -> &'static str {
    match source {
        Source::Reported => "reported by the provider",
        Source::Estimated => "estimated from a token budget you set",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::Tokens;

    fn hub() -> Hub {
        Hub {
            claude: Registry::new(),
            codex: codex::Watcher::default(),
            ledger: Ledger::default(),
            budgets: Budgets::default(),
            enabled: vec![CLAUDE.into(), CODEX.into()],
            primary: CLAUDE.into(),
            dirty: false,
            last_save: Instant::now(),
            backfilled: false,
        }
    }

    fn tok(n: u64) -> Tokens {
        Tokens { input: n, output: n, cache_read: 0, cache_write: 0 }
    }

    #[test]
    fn a_reported_window_beats_our_estimate() {
        let mut h = hub();
        let unix = now_unix();
        h.ledger.record(unix - 60, CODEX, "gpt-5.6-sol", tok(1000));
        h.budgets.five_hour_tokens = 10_000; // would estimate 20%

        let rep = codex::Reported {
            plan: Some("plus".into()),
            windows: vec![Window {
                label: "5h".into(),
                tokens: Tokens::default(),
                used_percent: Some(17.0),
                source: Source::Reported,
                resets_at: Some(unix + 600),
            }],
        };
        let v = h.view(CODEX, vec![], unix, Some(rep));
        let five = v.window("5h").unwrap();
        assert_eq!(five.used_percent, Some(17.0)); // the provider's number
        assert_eq!(five.source, Source::Reported);
        assert_eq!(five.tokens.total(), 2000); // still shows what we measured
        assert_eq!(five.resets_in(unix), Some(600));
        assert_eq!(v.plan.as_deref(), Some("plus"));
        // the weekly window has no reported figure, so ours stands
        assert_eq!(v.window("Week").unwrap().source, Source::Estimated);
    }

    #[test]
    fn per_model_weekly_surfaces_fable() {
        let mut h = hub();
        let unix = now_unix();
        h.ledger.record(unix - 60, CLAUDE, "Opus 5", tok(700));
        h.ledger.record(unix - 30, CLAUDE, "Fable 5", tok(300));
        h.budgets.per_model_weekly.insert("Fable 5".into(), 1_200);

        let v = h.view(CLAUDE, vec![], unix, None);
        let fable = v.model("fable 5").expect("fable tracked");
        assert_eq!(fable.share_percent, 30.0);
        assert_eq!(fable.budget_percent, Some(50.0)); // 600 billable of 1200
    }

    #[test]
    fn providers_can_be_turned_off_independently() {
        let mut h = hub();
        h.set_providers(vec![CODEX.into()], CODEX.into());
        assert!(!h.is_enabled(CLAUDE));
        let snap = h.snapshot(Instant::now());
        assert_eq!(snap.providers.len(), 1);
        assert_eq!(snap.providers[0].id, CODEX);
        assert_eq!(snap.primary, CODEX);
    }

    #[test]
    fn codex_events_only_drive_the_pet_when_it_is_primary() {
        let mut h = hub();
        h.set_providers(vec![CLAUDE.into(), CODEX.into()], CLAUDE.into());
        // Claude is primary, so nothing Codex saw is allowed through.
        assert!(h.poll().is_empty());
    }
}
