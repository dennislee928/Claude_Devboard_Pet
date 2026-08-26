//! Work-state machine: hook events in, pet state out.

use std::time::{Duration, Instant};

// Indices into assets_gen::STATE_NAMES.
pub const IDLE: usize = 0;
pub const CODING: usize = 1;
pub const THINKING: usize = 2;
pub const SEARCHING: usize = 3;
pub const TESTING: usize = 4;
pub const BUILDING: usize = 5;
pub const DEBUGGING: usize = 6;
pub const ERROR: usize = 7;
pub const SUCCESS: usize = 8;
pub const WAITING: usize = 9;
pub const NOTIFY: usize = 10;
pub const CELEBRATING: usize = 11;
pub const SLEEP: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolKind {
    Edit,
    RunTest,
    RunBuild,
    Search,
    Agent,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Prompt,
    ToolStart(ToolKind),
    ToolEnd { error: bool },
    Stopped,
    PermissionWait,
    SessionStart,
    ForceState(usize), // test/manual override via HTTP
    SetChar(usize),    // handled by the dispatcher, not the machine
    SetWander(bool),   // handled by the dispatcher, not the machine
    Petted,            // user clicked the pet
    Feed,              // user fed the pet a treat
    ToggleSleep,       // user toggled nap mode
    SetPanel(bool),    // show/hide the status panel (handled by the dispatcher)
    SetProvider(String, bool),   // watch / stop watching a coding agent
    SetPrimaryProvider(String),  // which agent drives the pet
    SubagentDone,      // a delegated subagent finished
}

/// XP awarded for an event (mirrors growth rules).
pub fn xp_for(ev: &Event) -> u64 {
    match ev {
        Event::Prompt => 5,
        Event::ToolStart(_) => 1,
        _ => 0,
    }
}

pub struct Machine {
    pub state: usize,
    revert: Option<(Instant, usize)>, // timed fallback (success->idle, error->thinking)
    recent_error: Option<Instant>,
    last_activity: Instant,
}

const ERROR_STICKY: Duration = Duration::from_secs(5);
const SUCCESS_HOLD: Duration = Duration::from_secs(10);
const NOTIFY_HOLD: Duration = Duration::from_secs(4);
const RECENT_ERROR_WINDOW: Duration = Duration::from_secs(60);
const IDLE_TO_SLEEP: Duration = Duration::from_secs(180);

impl Machine {
    pub fn new(now: Instant) -> Self {
        Machine { state: IDLE, revert: None, recent_error: None, last_activity: now }
    }

    /// Returns Some(bonus_xp) extra award (error recovery), None otherwise.
    pub fn on_event(&mut self, ev: &Event, now: Instant) -> Option<u64> {
        self.last_activity = now;
        let mut bonus = None;
        match ev {
            Event::Prompt => {
                self.set(THINKING, None);
            }
            // a new session gets a wave before the pet settles into thinking
            Event::SessionStart => {
                self.set(NOTIFY, Some((now + NOTIFY_HOLD, THINKING)));
            }
            Event::ToolStart(kind) => {
                let recent_err = self.recent_error.is_some_and(|t| now - t < RECENT_ERROR_WINDOW);
                let s = match kind {
                    ToolKind::Edit => {
                        if recent_err {
                            DEBUGGING
                        } else {
                            CODING
                        }
                    }
                    ToolKind::RunTest => TESTING,
                    ToolKind::RunBuild => {
                        if recent_err {
                            DEBUGGING
                        } else {
                            BUILDING
                        }
                    }
                    ToolKind::Search => SEARCHING,
                    ToolKind::Agent => THINKING,
                    ToolKind::Other => BUILDING,
                };
                self.set(s, None);
            }
            Event::ToolEnd { error: true } => {
                self.recent_error = Some(now);
                self.set(ERROR, Some((now + ERROR_STICKY, THINKING)));
            }
            Event::ToolEnd { error: false } => {
                if self.recent_error.take().is_some() {
                    bonus = Some(3); // recovered from an error
                }
                if self.state != ERROR || self.revert_expired(now) {
                    self.set(THINKING, None);
                }
            }
            Event::Stopped => {
                self.recent_error = None;
                self.set(SUCCESS, Some((now + SUCCESS_HOLD, IDLE)));
            }
            Event::PermissionWait => {
                self.set(NOTIFY, Some((now + NOTIFY_HOLD, WAITING)));
            }
            Event::ForceState(s) => {
                if *s < 13 {
                    self.set(*s, None);
                }
            }
            Event::Petted => {
                self.set(CELEBRATING, Some((now + Duration::from_millis(2500), IDLE)));
            }
            Event::Feed => {
                self.set(CELEBRATING, Some((now + Duration::from_secs(4), IDLE)));
            }
            Event::ToggleSleep => {
                if self.state == SLEEP {
                    self.set(IDLE, None);
                } else {
                    self.set(SLEEP, None);
                }
            }
            Event::SubagentDone => {
                bonus = Some(2);
                self.set(SUCCESS, Some((now + Duration::from_secs(3), THINKING)));
            }
            Event::SetChar(_) | Event::SetWander(_) | Event::SetPanel(_) | Event::SetProvider(..) | Event::SetPrimaryProvider(_) => {}
        }
        bonus
    }

    /// Level-up celebration: temporary override, then back to work.
    pub fn celebrate(&mut self, now: Instant) {
        self.set(CELEBRATING, Some((now + Duration::from_secs(4), THINKING)));
    }

    fn revert_expired(&self, now: Instant) -> bool {
        self.revert.map(|(t, _)| now >= t).unwrap_or(true)
    }

    fn set(&mut self, s: usize, revert: Option<(Instant, usize)>) {
        // A sticky error may not be interrupted by passive transitions, but
        // real new activity (coding etc.) always wins.
        self.state = s;
        self.revert = revert;
    }

    /// Advance timers. Returns true if the state changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some((t, target)) = self.revert {
            if now >= t {
                self.revert = None;
                if self.state != target {
                    self.state = target;
                    return true;
                }
            }
        }
        if self.state == IDLE && now - self.last_activity > IDLE_TO_SLEEP {
            self.state = SLEEP;
            return true;
        }
        false
    }

    /// Is the user actively working (for per-minute XP)?
    pub fn active(&self) -> bool {
        !matches!(self.state, IDLE | SLEEP | WAITING)
    }
}

/// Classify a Bash command line into a tool kind.
pub fn classify_bash(cmd: &str) -> ToolKind {
    let c = cmd.to_lowercase();
    if ["test", "pytest", "jest", "vitest", "ctest"].iter().any(|k| c.contains(k)) {
        ToolKind::RunTest
    } else if ["build", "make", "compile", "cargo run", "pio run", "cmake", "gradle", "msbuild"].iter().any(|k| c.contains(k)) {
        ToolKind::RunBuild
    } else if ["grep", "find ", "rg ", "ls", "dir", "cat ", "git log", "git diff", "git show"].iter().any(|k| c.contains(k)) {
        ToolKind::Search
    } else {
        ToolKind::Other
    }
}

/// Map a Claude Code tool name (+ optional bash command) to a kind.
pub fn classify_tool(tool: &str, bash_cmd: Option<&str>) -> ToolKind {
    match tool {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "Bash" | "PowerShell" => bash_cmd.map(classify_bash).unwrap_or(ToolKind::Other),
        "Grep" | "Glob" | "Read" | "WebSearch" | "WebFetch" | "LS" => ToolKind::Search,
        "Agent" | "Task" => ToolKind::Agent,
        _ => ToolKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn session_start_waves_then_thinks() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::SessionStart, now);
        assert_eq!(m.state, NOTIFY);
        m.tick(now + Duration::from_secs(5));
        assert_eq!(m.state, THINKING);
    }

    #[test]
    fn prompt_thinks_then_codes() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::Prompt, now);
        assert_eq!(m.state, THINKING);
        m.on_event(&Event::ToolStart(ToolKind::Edit), now);
        assert_eq!(m.state, CODING);
    }

    #[test]
    fn error_then_edit_is_debugging_and_recovery_pays() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::ToolEnd { error: true }, now);
        assert_eq!(m.state, ERROR);
        m.on_event(&Event::ToolStart(ToolKind::Edit), now + Duration::from_secs(1));
        assert_eq!(m.state, DEBUGGING);
        let bonus = m.on_event(&Event::ToolEnd { error: false }, now + Duration::from_secs(2));
        assert_eq!(bonus, Some(3));
    }

    #[test]
    fn stop_holds_success_then_idles_then_sleeps() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::Stopped, now);
        assert_eq!(m.state, SUCCESS);
        assert!(!m.tick(now + Duration::from_secs(5)));
        assert!(m.tick(now + Duration::from_secs(11)));
        assert_eq!(m.state, IDLE);
        assert!(m.tick(now + Duration::from_secs(200)));
        assert_eq!(m.state, SLEEP);
    }

    #[test]
    fn bash_classification() {
        assert_eq!(classify_bash("cargo test -p petd"), ToolKind::RunTest);
        assert_eq!(classify_bash("pio run -t upload"), ToolKind::RunBuild);
        assert_eq!(classify_bash("rg TODO src/"), ToolKind::Search);
        assert_eq!(classify_bash("echo hi"), ToolKind::Other);
    }

    #[test]
    fn pet_feed_and_nap() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::Petted, now);
        assert_eq!(m.state, CELEBRATING);
        m.tick(now + Duration::from_secs(3));
        assert_eq!(m.state, IDLE);
        m.on_event(&Event::ToggleSleep, now);
        assert_eq!(m.state, SLEEP);
        m.on_event(&Event::ToggleSleep, now);
        assert_eq!(m.state, IDLE);
    }

    #[test]
    fn permission_notifies_then_waits() {
        let now = t0();
        let mut m = Machine::new(now);
        m.on_event(&Event::PermissionWait, now);
        assert_eq!(m.state, NOTIFY);
        m.tick(now + Duration::from_secs(5));
        assert_eq!(m.state, WAITING);
    }
}
