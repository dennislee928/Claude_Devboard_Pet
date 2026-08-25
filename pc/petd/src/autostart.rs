//! Start the pet silently at login, on every OS.
//!
//!   petd --install-autostart      petd --uninstall-autostart
//!
//! Windows — HKCU\...\CurrentVersion\Run value (no console window is created
//!           because the release binary is a GUI subsystem app).
//! macOS   — ~/Library/LaunchAgents/dev.devpet.petd.plist, loaded with launchctl.
//! Linux   — ~/.config/systemd/user/devpet.service, enabled with systemctl --user.

use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "dev.devpet.petd";

fn exe() -> std::io::Result<PathBuf> {
    std::env::current_exe()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

#[cfg(target_os = "macos")]
fn plist_path() -> PathBuf {
    home().join("Library").join("LaunchAgents").join(format!("{LABEL}.plist"))
}

#[cfg(target_os = "linux")]
fn unit_path() -> PathBuf {
    home().join(".config").join("systemd").join("user").join("devpet.service")
}

pub fn install(args: &[String]) -> std::io::Result<String> {
    let exe = exe()?;
    let exe_s = exe.display().to_string();

    #[cfg(windows)]
    {
        let mut cmdline = format!("\"{exe_s}\"");
        for a in args {
            cmdline.push_str(&format!(" \"{a}\""));
        }
        let st = Command::new("reg")
            .args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "DevPet", "/t", "REG_SZ", "/d", &cmdline, "/f"])
            .status()?;
        if !st.success() {
            return Err(std::io::Error::other("reg add failed"));
        }
        return Ok("autostart installed (HKCU\\...\\Run\\DevPet)".into());
    }

    #[cfg(target_os = "macos")]
    {
        let p = plist_path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        let mut argv = format!("    <string>{exe_s}</string>\n");
        for a in args {
            argv.push_str(&format!("    <string>{a}</string>\n"));
        }
        let log = crate::paths::log_file();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\"><dict>\n\
             \x20 <key>Label</key><string>{LABEL}</string>\n\
             \x20 <key>ProgramArguments</key><array>\n{argv}  </array>\n\
             \x20 <key>RunAtLoad</key><true/>\n\
             \x20 <key>KeepAlive</key><false/>\n\
             \x20 <key>ProcessType</key><string>Background</string>\n\
             \x20 <key>StandardOutPath</key><string>{log}</string>\n\
             \x20 <key>StandardErrorPath</key><string>{log}</string>\n\
             </dict></plist>\n",
            log = log.display()
        );
        std::fs::write(&p, plist)?;
        let _ = Command::new("launchctl").args(["unload", &p.display().to_string()]).status();
        let _ = Command::new("launchctl").args(["load", "-w", &p.display().to_string()]).status();
        return Ok(format!("autostart installed ({})", p.display()));
    }

    #[cfg(target_os = "linux")]
    {
        let p = unit_path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        let argv = if args.is_empty() { String::new() } else { format!(" {}", args.join(" ")) };
        let unit = format!(
            "[Unit]\nDescription=DevPet desk pet\nAfter=graphical-session.target\n\n\
             [Service]\nType=simple\nExecStart={exe_s}{argv}\nRestart=on-failure\nEnvironment=DEVPET_DAEMON=1\n\n\
             [Install]\nWantedBy=default.target\n"
        );
        std::fs::write(&p, unit)?;
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        let _ = Command::new("systemctl").args(["--user", "enable", "--now", "devpet.service"]).status();
        return Ok(format!("autostart installed ({})", p.display()));
    }

    #[allow(unreachable_code)]
    Err(std::io::Error::other("autostart not supported on this platform"))
}

pub fn uninstall() -> std::io::Result<String> {
    #[cfg(windows)]
    {
        let _ = Command::new("reg")
            .args(["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "DevPet", "/f"])
            .status();
        return Ok("autostart removed".into());
    }
    #[cfg(target_os = "macos")]
    {
        let p = plist_path();
        let _ = Command::new("launchctl").args(["unload", "-w", &p.display().to_string()]).status();
        let _ = std::fs::remove_file(&p);
        return Ok("autostart removed".into());
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl").args(["--user", "disable", "--now", "devpet.service"]).status();
        let _ = std::fs::remove_file(unit_path());
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        return Ok("autostart removed".into());
    }
    #[allow(unreachable_code)]
    Err(std::io::Error::other("autostart not supported on this platform"))
}
