//! Running silently in the background on every OS.
//!
//! `--daemon` re-launches the same executable detached from the terminal with
//! its output redirected to petd.log, then the launcher process exits. The
//! child is marked with DEVPET_DAEMON=1 so it does not detach again.
//!
//! * Windows — the release binary is already a GUI subsystem app (no console);
//!   the child is additionally spawned DETACHED_PROCESS | CREATE_NO_WINDOW.
//! * macOS / Linux — the child gets its own process group (`setsid` equivalent)
//!   so closing the terminal does not kill it.

use crate::paths;
use std::process::{Command, Stdio};

pub const ENV_MARKER: &str = "DEVPET_DAEMON";

pub fn already_detached() -> bool {
    std::env::var_os(ENV_MARKER).is_some()
}

/// Re-spawn self detached. Returns Ok(true) when the caller is the launcher
/// and should exit immediately.
pub fn detach(extra_args: &[String]) -> std::io::Result<bool> {
    if already_detached() {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    paths::ensure_state_dir();
    let log = std::fs::OpenOptions::new().create(true).append(true).open(paths::log_file())?;
    let log2 = log.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.args(extra_args)
        .env(ENV_MARKER, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log2));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()?;
    Ok(true)
}

/// Send output to petd.log even when not daemonised, so a silently started pet
/// still leaves a trail. Only used when stdout is not a terminal.
pub fn log_line(msg: &str) {
    println!("{msg}");
}
