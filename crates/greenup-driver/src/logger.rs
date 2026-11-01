use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    Mutex, OnceLock,
};
use tokio::sync::broadcast;

/// Structured log entry suitable for real-time streaming and Web UI rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub category: String, // "SERIAL_RX", "SERIAL_TX", "API", "IHM", "SYS", "DEBUG", "TRACE", etc.
    pub level: String,    // "INFO", "DEBUG", "WARN", "ERROR"
    pub message: String,
}

/// Hierarchical log levels for filtering.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "ERROR" => Some(LogLevel::Error),
            "WARN" | "WARNING" => Some(LogLevel::Warn),
            "INFO" => Some(LogLevel::Info),
            "DEBUG" => Some(LogLevel::Debug),
            "TRACE" => Some(LogLevel::Trace),
            _ => None,
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => LogLevel::Error,
            2 => LogLevel::Warn,
            4 => LogLevel::Debug,
            5 => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }
}

const RETENTION_DAYS: i64 = 7;
/// Threshold in bytes before warning that a daily log file is growing large (10 MB).
pub const LOG_WARN_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

/// Evaluates whether a daily log file has exceeded the size warning threshold.
/// Returns `Some((warned_mb, size_mb))` if a warning should be emitted.
pub fn should_warn_file_size(len: u64, last_warned_mb: u64) -> Option<(u64, f64)> {
    if len >= LOG_WARN_THRESHOLD_BYTES {
        let current_mb = len / (1024 * 1024);
        if (last_warned_mb == 0 && current_mb >= 10) || current_mb >= last_warned_mb + 10 {
            let size_mb = len as f64 / (1024.0 * 1024.0);
            return Some((current_mb, size_mb));
        }
    }
    None
}

static LAST_WARNED_MB: AtomicU64 = AtomicU64::new(0);
static WRITE_COUNT: AtomicU64 = AtomicU64::new(0);
static IS_CHECKING_SIZE: AtomicBool = AtomicBool::new(false);
static CONFIGURED_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);
static CURRENT_LOG_DATE: Mutex<Option<String>> = Mutex::new(None);
static RECENT_LOGS: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const MAX_RECENT_LOGS: usize = 500;

/// Returns current configured log level.
pub fn get_log_level() -> LogLevel {
    LogLevel::from_u8(CONFIGURED_LOG_LEVEL.load(Ordering::Relaxed))
}

/// Sets log level dynamically.
pub fn set_log_level(level: LogLevel) {
    CONFIGURED_LOG_LEVEL.store(level as u8, Ordering::Relaxed);
    println!("[Logger] 🎚️ Log level set to: {}", level.as_str());
}

fn get_broadcast_sender() -> &'static broadcast::Sender<LogEntry> {
    static SENDER: OnceLock<broadcast::Sender<LogEntry>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(500);
        tx
    })
}

/// Subscribes to the live log stream.
pub fn subscribe() -> broadcast::Receiver<LogEntry> {
    get_broadcast_sender().subscribe()
}

/// Retrieves a snapshot of the most recent in-memory logs.
pub fn get_recent_logs() -> Vec<LogEntry> {
    if let Ok(guard) = RECENT_LOGS.lock() {
        guard.iter().cloned().collect()
    } else {
        Vec::new()
    }
}

/// Returns the configured log directory.
/// Checks `GREENUP_LOG_DIR` env var, local `logs/`, or `/opt/greenup-link/logs/`.
pub fn get_log_dir() -> String {
    if let Ok(dir) = std::env::var("GREENUP_LOG_DIR") {
        return dir;
    }
    if Path::new("logs").exists() {
        return "logs".to_string();
    }
    if Path::new("/opt/greenup-link/logs").exists() {
        return "/opt/greenup-link/logs".to_string();
    }
    "logs".to_string()
}

/// Returns the path of today's log file.
pub fn get_log_file_path() -> Option<String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let dir = get_log_dir();
    Some(format!("{}/greenup_{}.log", dir, today))
}

/// Reads the current daily log file content or returns recent in-memory logs formatted as text.
pub fn get_current_log_content() -> (String, String) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let filename = format!("greenup_{}.log", today);
    let dir = get_log_dir();
    let path = format!("{}/{}", dir, filename);

    if let Ok(content) = fs::read_to_string(&path) {
        return (filename, content);
    }

    // Fallback: format recent in-memory logs
    let recent = get_recent_logs();
    let mut out = String::new();
    for entry in recent {
        out.push_str(&format!("[{}] [{}] [{}] {}\n", entry.timestamp, entry.level, entry.category, entry.message));
    }
    (filename, out)
}

/// Cleans up log files older than RETENTION_DAYS.
fn cleanup_old_logs(log_dir: &str) {
    let cutoff = Local::now().date_naive() - Duration::days(RETENTION_DAYS);
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.starts_with("greenup_") && file_name.ends_with(".log") {
                    let date_part = file_name
                        .trim_start_matches("greenup_")
                        .trim_end_matches(".log");
                    if let Ok(file_date) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                        if file_date < cutoff {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

/// Ensures the daily log file is open, rotating when midnight passes.
fn get_or_rotate_daily_file() -> String {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let dir = get_log_dir();
    let path = format!("{}/greenup_{}.log", dir, today);

    let mut guard = CURRENT_LOG_DATE.lock().unwrap_or_else(|e| e.into_inner());
    let is_new_day = match *guard {
        Some(ref date) => date != &today,
        None => true,
    };

    if is_new_day {
        *guard = Some(today.clone());
        let _ = fs::create_dir_all(&dir);
        cleanup_old_logs(&dir);
        LAST_WARNED_MB.store(0, Ordering::Relaxed);

        // Write daily header if newly created
        if !Path::new(&path).exists() {
            if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(&path) {
                let _ = writeln!(f, "=== GreenUp Log started on {} ===", today);
            }
        }
    }

    path
}

/// Initializes the logger and sets initial log level from environment.
pub fn init() {
    let dir = get_log_dir();
    let _ = fs::create_dir_all(&dir);

    // Initialize level from GREENUP_LOG_LEVEL or GREENUP_DEBUG
    if let Ok(val) = std::env::var("GREENUP_LOG_LEVEL") {
        if let Some(level) = LogLevel::from_str_loose(&val) {
            set_log_level(level);
        }
    } else if std::env::var("GREENUP_DEBUG").is_ok() {
        set_log_level(LogLevel::Debug);
    } else {
        set_log_level(LogLevel::Info);
    }

    let initial_path = get_or_rotate_daily_file();
    println!("[Logger] 📝 Daily rolling log active: {} (level: {})", initial_path, get_log_level().as_str());

    // Check if initial log file already exceeds threshold
    if let Ok(meta) = fs::metadata(&initial_path) {
        if let Some((current_mb, size_mb)) = should_warn_file_size(meta.len(), LAST_WARNED_MB.load(Ordering::Relaxed)) {
            LAST_WARNED_MB.store(current_mb, Ordering::Relaxed);
            warn(
                "SYS",
                &format!(
                    "⚠️ Daily log file exceeds {} MB ({:.1} MB). Storage usage will increase while DEBUG/TRACE is active.",
                    current_mb, size_mb
                ),
            );
        }
    }
}

/// Writes an entry to stdout, daily log file, ring buffer, and live SSE broadcast with an explicit category and level.
pub fn log_with_level(category: &str, level: LogLevel, message: &str) {
    let now = Local::now();
    let time_str = now.format("%H:%M:%S%.3f").to_string();
    let full_timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    let entry_text = format!("[{}] [{}] [{}] {}", full_timestamp, level.as_str(), category, message);

    let configured_level = get_log_level();
    let should_output = level <= configured_level;

    // Console output
    if should_output {
        println!("{}", entry_text);
    }

    // Daily file output
    let mut warning_to_emit: Option<(u64, f64)> = None;

    if should_output {
        let path = get_or_rotate_daily_file();
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{}", entry_text);

            let count = WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
            if count % 256 == 0 && !IS_CHECKING_SIZE.swap(true, Ordering::SeqCst) {
                if let Ok(meta) = f.metadata() {
                    if let Some((current_mb, size_mb)) = should_warn_file_size(meta.len(), LAST_WARNED_MB.load(Ordering::Relaxed)) {
                        LAST_WARNED_MB.store(current_mb, Ordering::Relaxed);
                        warning_to_emit = Some((current_mb, size_mb));
                    }
                }
                IS_CHECKING_SIZE.store(false, Ordering::SeqCst);
            }
        }
    }

    let log_item = LogEntry {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        timestamp: time_str,
        category: category.to_string(),
        level: level.as_str().to_string(),
        message: message.to_string(),
    };

    // Store in in-memory ring buffer (up to 500 lines for the Web console)
    if let Ok(mut guard) = RECENT_LOGS.lock() {
        if guard.len() >= MAX_RECENT_LOGS {
            guard.pop_front();
        }
        guard.push_back(log_item.clone());
    }

    // Broadcast to SSE subscribers
    let _ = get_broadcast_sender().send(log_item);

    if let Some((warned_mb, size_mb)) = warning_to_emit {
        let warn_msg = format!(
            "⚠️ Daily log file exceeds {} MB ({:.1} MB). Storage usage will increase while DEBUG/TRACE is active.",
            warned_mb, size_mb
        );
        log_with_level("SYS", LogLevel::Warn, &warn_msg);
    }
}

/// Convenience function for logging at ERROR level.
pub fn error(category: &str, message: &str) {
    log_with_level(category, LogLevel::Error, message);
}

/// Convenience function for logging at WARN level.
pub fn warn(category: &str, message: &str) {
    log_with_level(category, LogLevel::Warn, message);
}

/// Convenience function for logging at INFO level.
pub fn info(category: &str, message: &str) {
    log_with_level(category, LogLevel::Info, message);
}

/// Convenience function for logging at DEBUG level.
pub fn debug(category: &str, message: &str) {
    log_with_level(category, LogLevel::Debug, message);
}

/// Convenience function for logging at TRACE level.
pub fn trace(category: &str, message: &str) {
    log_with_level(category, LogLevel::Trace, message);
}

/// Writes an entry to stdout, daily log file, ring buffer, and live SSE broadcast (inferring level from category/content).
pub fn log(category: &str, message: &str) {
    let level_enum = match category {
        "DEBUG" => LogLevel::Debug,
        "TRACE" => LogLevel::Trace,
        "ERROR" => LogLevel::Error,
        "WARN" => LogLevel::Warn,
        _ => {
            if message.contains("❌") || message.to_lowercase().contains("error") {
                LogLevel::Error
            } else if message.contains("⚠️") {
                LogLevel::Warn
            } else {
                LogLevel::Info
            }
        }
    };

    log_with_level(category, level_enum, message);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_warn_file_size() {
        // Below 10 MB: no warning
        assert_eq!(should_warn_file_size(5 * 1024 * 1024, 0), None);
        assert_eq!(should_warn_file_size(9 * 1024 * 1024 + 500_000, 0), None);

        // At or above 10 MB for the first time: warning emitted
        assert_eq!(should_warn_file_size(10 * 1024 * 1024, 0), Some((10, 10.0)));
        let warn_10_5 = should_warn_file_size((10.5 * 1024.0 * 1024.0) as u64, 0);
        assert!(warn_10_5.is_some());
        assert_eq!(warn_10_5.unwrap().0, 10);

        // Already warned at 10 MB, below next 10 MB step: no warning
        assert_eq!(should_warn_file_size(15 * 1024 * 1024, 10), None);
        assert_eq!(should_warn_file_size(19 * 1024 * 1024, 10), None);

        // Reaching next 10 MB step (20 MB): warning emitted
        assert_eq!(should_warn_file_size(20 * 1024 * 1024, 10), Some((20, 20.0)));
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_str_loose("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str_loose("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str_loose("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str_loose("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str_loose("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str_loose("trace"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_str_loose("invalid"), None);
    }
}
