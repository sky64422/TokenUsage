//! Process spawn + short cache for `tokscale usage --json` (excluded from coverage gate).

use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct CacheEntry {
    at: Instant,
    raw: String,
}

static CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(45);

pub fn run_tokscale_usage_json() -> Result<String, String> {
    {
        let guard = CACHE.lock().unwrap();
        if let Some(entry) = guard.as_ref() {
            if entry.at.elapsed() < CACHE_TTL {
                return Ok(entry.raw.clone());
            }
        }
    }

    let output = spawn_tokscale()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "tokscale exited {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("tokscale produced empty stdout".into());
    }
    if !(stdout.starts_with('[') || stdout.starts_with('{')) {
        return Err(format!(
            "tokscale stdout is not JSON: {}",
            stdout.chars().take(120).collect::<String>()
        ));
    }

    {
        let mut guard = CACHE.lock().unwrap();
        *guard = Some(CacheEntry {
            at: Instant::now(),
            raw: stdout.clone(),
        });
    }
    Ok(stdout)
}

fn spawn_tokscale() -> Result<std::process::Output, String> {
    // 1) Global install
    if let Ok(out) = run_cmd("tokscale", &["usage", "--json"]) {
        if out.status.success() || !out.stdout.is_empty() {
            return Ok(out);
        }
    }

    // 2) npx (Windows needs .cmd; bare "npx" often fails from GUI PATH)
    #[cfg(windows)]
    {
        for program in ["npx.cmd", "npx"] {
            if let Ok(out) = run_cmd(program, &["--yes", "tokscale", "usage", "--json"]) {
                if out.status.success() || !out.stdout.is_empty() {
                    return Ok(out);
                }
            }
        }
        // 3) cmd /c with full shell PATH resolution
        if let Ok(out) = run_via_cmd("npx --yes tokscale usage --json") {
            return Ok(out);
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(out) = run_cmd("npx", &["--yes", "tokscale", "usage", "--json"]) {
            return Ok(out);
        }
        if let Ok(out) = run_cmd("bunx", &["tokscale@latest", "usage", "--json"]) {
            return Ok(out);
        }
    }

    Err(
        "tokscale not available. Install: npm i -g tokscale  (or ensure npx works in PATH)"
            .into(),
    )
}

fn run_cmd(program: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    // Inherit a fuller PATH for GUI-launched processes (often missing npm)
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
        .map_err(|e| format!("failed to spawn {program}: {e}"))
}

#[cfg(windows)]
fn run_via_cmd(cmdline: &str) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("cmd.exe")
        .args(["/C", cmdline])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to spawn cmd: {e}"))
}
