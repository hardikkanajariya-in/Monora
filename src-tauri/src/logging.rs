use chrono::Local;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

static LOG_PATH: OnceCell<PathBuf> = OnceCell::new();
static LOG_MUTEX: Mutex<()> = Mutex::new(());

pub fn init_logging() {
    let path = log_file_path();
    let _ = fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);
    LOG_PATH.set(path).ok();
    info("Application logging initialized");
}

fn log_file_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let portable = dir.join("logs");
            return portable.join("simple-recorder.log");
        }
    }
    std::env::temp_dir().join("simple-recorder.log")
}

pub fn info(message: impl AsRef<str>) {
    write_line("INFO", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write_line("WARN", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write_line("ERROR", message.as_ref());
}

fn write_line(level: &str, message: &str) {
    let _guard = LOG_MUTEX.lock();
    let path = LOG_PATH.get().cloned().unwrap_or_else(log_file_path);
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{timestamp}] [{level}] {message}\n");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}
