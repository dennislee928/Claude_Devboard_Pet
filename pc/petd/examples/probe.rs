//! Read-only probe: what would the status panel show right now?
//! `cargo run -p petd --example probe`
use petd::providers::Hub;
use petd::usage::Budgets;
use std::time::Instant;

fn main() {
    let mut budgets = Budgets::default();
    // pretend limits, just to exercise the estimated-percentage path
    budgets.five_hour_tokens = 20_000_000;
    budgets.weekly_tokens = 200_000_000;
    budgets.per_model_weekly.insert("Fable 5".into(), 50_000_000);

    let mut hub = Hub::new(vec!["claude".into(), "codex".into()], "claude".into(), budgets);
    let events = hub.poll();
    let snap = hub.snapshot(Instant::now());
    println!("codex events derived on first poll: {} (expected 0 — no replay)", events.len());
    for p in &snap.providers {
        println!("\n== {} (present: {}, plan: {:?})", p.name, p.present, p.plan);
        println!("   sessions: {}, working now: {}", p.sessions.len(), p.active);
        for w in &p.windows {
            println!(
                "   {:5} {:>8} tokens  {}  [{:?}]{}",
                w.label,
                w.tokens.total(),
                w.used_percent.map(|p| format!("{p:.1}%")).unwrap_or_else(|| "—".into()),
                w.source,
                w.resets_in(snap.now).map(|r| format!("  resets in {}m", r / 60)).unwrap_or_default()
            );
        }
        for m in p.models.iter().take(5) {
            println!(
                "   model {:12} {:>9} tokens  {:.1}% of week{}",
                m.model,
                m.tokens.total(),
                m.share_percent,
                m.budget_percent.map(|b| format!("  ({b:.1}% of budget)")).unwrap_or_default()
            );
        }
        for s in p.sessions.iter().take(4) {
            println!("   · {:16} {:10} {:>8} tok  {}", s.project, s.model, s.tokens.total(), s.action);
        }
    }
}
