use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

fn log_file_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let dir = PathBuf::from(local).join("TokenUsage").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("capture-debug.log"))
}

pub fn log_event(tag: &str, message: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED
        .get_or_init(|| std::env::var_os("TOKENUSAGE_CAPTURE_DEBUG").is_some_and(|v| v == "1"))
    {
        return;
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{now}] [{tag}] {message}\n");
    if let Some(path) = log_file_path() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 2 * 1024 * 1024 {
                let _ = std::fs::remove_file(&path);
            }
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
