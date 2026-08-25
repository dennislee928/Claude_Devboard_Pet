//! devpet-setup: self-extracting installer. The DevPet payload zip is embedded
//! at build time (PAYLOAD_ZIP env var, set by scripts\package.ps1); running
//! the exe extracts it to a temp folder and hands off to setup.ps1.

use std::io::{BufRead, Write};

static PAYLOAD: &[u8] = include_bytes!(env!("PAYLOAD_ZIP"));

fn main() {
    println!("== DevPet Setup ==");
    let dir = std::env::temp_dir().join(format!("devpet_setup_{}", std::process::id()));
    let result = install(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(code) if code == 0 => println!("\nInstall finished. DevPet is running."),
        Ok(code) => println!("\nsetup.ps1 exited with code {code} — see messages above."),
        Err(e) => println!("\nInstall failed: {e}"),
    }
    print!("Press Enter to close...");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
}

fn install(dir: &std::path::Path) -> Result<i32, Box<dyn std::error::Error>> {
    println!("extracting payload...");
    let mut ar = zip::ZipArchive::new(std::io::Cursor::new(PAYLOAD))?;
    ar.extract(dir)?;
    println!("running setup.ps1...");
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(dir.join("setup.ps1"))
        .status()?;
    Ok(status.code().unwrap_or(1))
}
