//! Time-windowed token accounting, shared by every provider.
//!
//! Each provider reports token samples stamped with the moment they happened
//! and the model that produced them. The ledger keeps a week of those samples
//! (persisted, so restarting the pet does not reset your weekly figure) and
//! answers the questions the status panel asks: how much this session, how
//! much in the last five hours, how much this week, and how much of that was
//! any one model — Fable, for instance.
//!
//! Percentages come from two very different places and the panel must not
//! confuse them:
//!   * `Source::Reported` — the provider told us a real percentage of a real
//!     plan limit (Codex does this in every `token_count` event).
//!   * `Source::Estimated` — we divided our own token count by a budget the
//!     user configured. Honest, but ours, not the provider's.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rolling windows the panel reports on.
pub const FIVE_HOURS: u64 = 5 * 3600;
pub const ONE_WEEK: u64 = 7 * 24 * 3600;
/// Samples older than this are dropped from the ledger.
const KEEP: u64 = ONE_WEEK + 24 * 3600;

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
    /// Tokens that actually cost quota: cache reads are billed differently and
    /// are excluded from the "billable" figure the windows report.
    pub fn billable(&self) -> u64 {
        self.input + self.output + self.cache_write
    }
    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }
    pub fn is_zero(&self) -> bool {
        self.total() == 0
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Parse the RFC 3339 stamps both Claude Code and Codex write
/// (`2026-07-24T09:24:29.870Z`) into unix seconds. Deliberately tiny: only the
/// shape those two tools emit is supported, and anything else returns None
/// rather than a wrong answer.
pub fn parse_rfc3339(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    // days from civil (Howard Hinnant's algorithm)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + h * 3600 + mi * 60 + sec;
    // a trailing offset (+08:00) shifts the instant; Z / absent means UTC
    let mut secs = secs;
    if let Some(pos) = s.rfind(['+', '-']).filter(|p| *p > 10) {
        let sign = if b[pos] == b'+' { -1 } else { 1 };
        let off = &s[pos + 1..];
        if let (Some(oh), Some(om)) = (off.get(0..2).and_then(|x| x.parse::<i64>().ok()), off.get(3..5).and_then(|x| x.parse::<i64>().ok())) {
            secs += sign * (oh * 3600 + om * 60);
        }
    }
    u64::try_from(secs).ok()
}

/// One provider's token spend at one moment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub at: u64,
    pub provider: String,
    pub model: String,
    pub tokens: Tokens,
}

/// Where a percentage came from. The panel labels these differently on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// The provider's own number, against its own plan limit.
    Reported,
    /// Ours: measured tokens over a budget the user configured.
    Estimated,
}

/// A usage window as shown in the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub label: String,
    pub tokens: Tokens,
    /// None when nothing says what the limit is.
    pub used_percent: Option<f32>,
    pub source: Source,
    /// Unix seconds at which the window resets, when the provider says so.
    pub resets_at: Option<u64>,
}

impl Window {
    pub fn resets_in(&self, now: u64) -> Option<u64> {
        self.resets_at.filter(|t| *t > now).map(|t| t - now)
    }
}

/// Per-model spend, e.g. how much of this week went to Fable.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelUsage {
    pub model: String,
    pub tokens: Tokens,
    /// Share of the window's total tokens — always available.
    pub share_percent: f32,
    /// Share of a per-model budget, when the user configured one.
    pub budget_percent: Option<f32>,
}

/// Token budgets the user can set so estimated percentages become meaningful.
/// Zero means "unknown" and the panel then shows tokens without a percentage,
/// rather than inventing a limit.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Budgets {
    pub five_hour_tokens: u64,
    pub weekly_tokens: u64,
    /// Per-model weekly budgets, keyed by the pretty model name ("Fable 5").
    pub per_model_weekly: BTreeMap<String, u64>,
}

fn pct(used: u64, budget: u64) -> Option<f32> {
    (budget > 0).then(|| (used as f64 / budget as f64 * 100.0) as f32)
}

/// A week of token samples, plus a record of how much of every transcript has
/// already been counted.
///
/// Those offsets are what make the weekly figure correct: on first run the
/// providers scan a week of history to fill the window, and because the byte
/// offsets are persisted alongside the samples, restarting the pet never
/// counts the same assistant turn twice.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub samples: Vec<Sample>,
    pub lifetime: Tokens,
    pub prompts: u64,
    /// transcript path -> bytes already folded into `samples`
    #[serde(default)]
    pub offsets: BTreeMap<String, u64>,
}

impl Ledger {
    /// How much of `path` has already been counted.
    pub fn offset(&self, path: &std::path::Path) -> u64 {
        self.offsets.get(&path.to_string_lossy().into_owned()).copied().unwrap_or(0)
    }

    pub fn set_offset(&mut self, path: &std::path::Path, off: u64) {
        self.offsets.insert(path.to_string_lossy().into_owned(), off);
    }

    pub fn record(&mut self, at: u64, provider: &str, model: &str, tokens: Tokens) {
        if tokens.is_zero() {
            return;
        }
        self.lifetime.add(&tokens);
        self.samples.push(Sample { at, provider: provider.to_string(), model: model.to_string(), tokens });
    }

    /// Drop samples older than a week, and offsets for transcripts that have
    /// gone away, so neither grows without bound. True if anything went.
    pub fn prune(&mut self, now: u64) -> bool {
        let before = (self.samples.len(), self.offsets.len());
        self.samples.retain(|s| now.saturating_sub(s.at) <= KEEP);
        self.offsets.retain(|p, _| std::path::Path::new(p).exists());
        (self.samples.len(), self.offsets.len()) != before
    }

    fn sum(&self, provider: &str, since: u64) -> Tokens {
        let mut t = Tokens::default();
        for s in self.samples.iter().filter(|s| s.provider == provider && s.at >= since) {
            t.add(&s.tokens);
        }
        t
    }

    /// Tokens for `provider` over the last `secs`.
    pub fn window(&self, provider: &str, now: u64, secs: u64) -> Tokens {
        self.sum(provider, now.saturating_sub(secs))
    }

    /// Per-model breakdown over the last `secs`, biggest first.
    pub fn by_model(&self, provider: &str, now: u64, secs: u64, budgets: &Budgets) -> Vec<ModelUsage> {
        let since = now.saturating_sub(secs);
        let mut acc: BTreeMap<String, Tokens> = BTreeMap::new();
        for s in self.samples.iter().filter(|s| s.provider == provider && s.at >= since) {
            acc.entry(s.model.clone()).or_default().add(&s.tokens);
        }
        let total: u64 = acc.values().map(|t| t.total()).sum();
        let mut v: Vec<ModelUsage> = acc
            .into_iter()
            .map(|(model, tokens)| ModelUsage {
                share_percent: if total > 0 { (tokens.total() as f64 / total as f64 * 100.0) as f32 } else { 0.0 },
                budget_percent: budgets.per_model_weekly.get(&model).and_then(|b| pct(tokens.billable(), *b)),
                model,
                tokens,
            })
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.tokens.total()));
        v
    }

    /// The two estimated windows the panel always shows for a provider.
    pub fn windows(&self, provider: &str, now: u64, budgets: &Budgets) -> Vec<Window> {
        let five = self.window(provider, now, FIVE_HOURS);
        let week = self.window(provider, now, ONE_WEEK);
        vec![
            Window {
                label: "5h".into(),
                used_percent: pct(five.billable(), budgets.five_hour_tokens),
                tokens: five,
                source: Source::Estimated,
                resets_at: None,
            },
            Window {
                label: "Week".into(),
                used_percent: pct(week.billable(), budgets.weekly_tokens),
                tokens: week,
                source: Source::Estimated,
                resets_at: None,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_stamps_both_tools_write() {
        // Codex: 2026-07-24T09:24:29.870Z
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(parse_rfc3339("1970-01-02T00:00:01Z"), Some(86_401));
        assert_eq!(parse_rfc3339("2000-03-01T00:00:00Z"), Some(951_868_800));
        // an explicit offset moves the instant
        assert_eq!(parse_rfc3339("2000-03-01T08:00:00+08:00"), Some(951_868_800));
        assert_eq!(parse_rfc3339("nonsense"), None);
        assert_eq!(parse_rfc3339("2026-13-01T00:00:00Z"), None);
    }

    fn tok(n: u64) -> Tokens {
        Tokens { input: n, output: n, cache_read: n, cache_write: 0 }
    }

    #[test]
    fn windows_only_count_what_falls_inside_them() {
        let now = 1_000_000u64;
        let mut l = Ledger::default();
        l.record(now - 10, "claude", "Opus 5", tok(100)); // in both windows
        l.record(now - FIVE_HOURS - 60, "claude", "Fable 5", tok(50)); // week only
        l.record(now - ONE_WEEK - 60, "claude", "Opus 5", tok(999)); // in neither
        l.record(now - 10, "codex", "gpt-5.6", tok(7)); // other provider

        assert_eq!(l.window("claude", now, FIVE_HOURS).input, 100);
        assert_eq!(l.window("claude", now, ONE_WEEK).input, 150);
        assert_eq!(l.window("codex", now, FIVE_HOURS).input, 7);
    }

    #[test]
    fn per_model_share_and_budget() {
        let now = 1_000_000u64;
        let mut l = Ledger::default();
        l.record(now - 10, "claude", "Opus 5", tok(300));
        l.record(now - 20, "claude", "Fable 5", tok(100));
        let mut b = Budgets::default();
        b.per_model_weekly.insert("Fable 5".into(), 1_000);

        let models = l.by_model("claude", now, ONE_WEEK, &b);
        assert_eq!(models[0].model, "Opus 5"); // biggest first
        let fable = models.iter().find(|m| m.model == "Fable 5").unwrap();
        assert_eq!(fable.share_percent, 25.0); // 100 of 400 total
        // billable excludes cache reads: 100 in + 100 out of a 1000 budget
        assert_eq!(fable.budget_percent, Some(20.0));
    }

    #[test]
    fn no_budget_means_no_invented_percentage() {
        let now = 1_000_000u64;
        let mut l = Ledger::default();
        l.record(now - 10, "claude", "Opus 5", tok(100));
        let w = l.windows("claude", now, &Budgets::default());
        assert!(w.iter().all(|w| w.used_percent.is_none()));
        assert_eq!(w[0].tokens.total(), 300);

        let b = Budgets { five_hour_tokens: 400, weekly_tokens: 1000, ..Default::default() };
        let w = l.windows("claude", now, &b);
        assert_eq!(w[0].used_percent, Some(50.0)); // 200 billable of 400
        assert_eq!(w[1].used_percent, Some(20.0));
    }

    #[test]
    fn offsets_survive_a_restart_so_nothing_is_counted_twice() {
        let mut l = Ledger::default();
        let p = std::path::Path::new("/tmp/does-not-exist.jsonl");
        assert_eq!(l.offset(p), 0);
        l.set_offset(p, 4096);
        let round: Ledger = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(round.offset(p), 4096);
        // an offset for a transcript that no longer exists is dropped
        let mut round = round;
        assert!(round.prune(now_unix()));
        assert!(round.offsets.is_empty());
    }

    #[test]
    fn pruning_keeps_a_week() {
        let now = 2_000_000u64;
        let mut l = Ledger::default();
        l.record(now - 10, "claude", "Opus 5", tok(1));
        l.record(now - ONE_WEEK - 2 * 86_400, "claude", "Opus 5", tok(1));
        assert!(l.prune(now));
        assert_eq!(l.samples.len(), 1);
        assert_eq!(l.lifetime.input, 2); // lifetime survives pruning
    }
}
