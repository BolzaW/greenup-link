use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{
    atomic::{AtomicU64, Ordering},
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

static LOG_FILE: Mutex<Option<String>> = Mutex::new(None);
static RECENT_LOGS: Mutex<VecDeque<LogEntry>> = Mutex::new(VecDeque::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const MAX_RECENT_LOGS: usize = 500;

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

/// Initializes the logger by creating the log file.
/// The file is named with the startup date/time.
/// Example: logs/greenup_2026-09-30_23-15-42.log
pub fn init() {
    let log_dir = "logs";
    let _ = fs::create_dir_all(log_dir);

    let now = Local::now();
    let filename = format!("{}/greenup_{}.log", log_dir, now.format("%Y-%m-%d_%H-%M-%S"));

    // Create the file
    if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(&filename) {
        let _ = writeln!(f, "=== GreenUp Logger started on {} ===", now.format("%d/%m/%Y at %H:%M:%S"));
    }

    if let Ok(mut guard) = LOG_FILE.lock() {
        *guard = Some(filename.clone());
    }

    println!("[Logger] 📝 Log file: {}", filename);
}

/// Writes an entry to the log file, stdout, in-memory ring buffer, and live broadcast stream.
///   category : "SERIAL_RX", "SERIAL_TX", "API", "IHM", "SYS", "DEBUG", "TRACE"
///   message  : the content to log
pub fn log(category: &str, message: &str) {
    let now = Local::now();
    let time_str = now.format("%H:%M:%S%.3f").to_string();
    let full_timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    let entry_text = format!("[{}] [{}] {}", full_timestamp, category, message);

    // Stdout (console): skip TRACE and DEBUG by default to avoid spamming the console
    if category != "TRACE" && (category != "DEBUG" || std::env::var("GREENUP_DEBUG").is_ok()) {
        println!("{}", entry_text);
    }

    // File: all categories are written
    if let Ok(guard) = LOG_FILE.lock() {
        if let Some(ref path) = *guard {
            if let Ok(mut f) = OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "{}", entry_text);
            }
        }
    }

    // Determine level
    let level = if category == "DEBUG" || category == "TRACE" {
        "DEBUG"
    } else if category == "ERROR" || message.contains("❌") || message.to_lowercase().contains("error") {
        "ERROR"
    } else if category == "WARN" || message.contains("⚠️") {
        "WARN"
    } else {
        "INFO"
    };

    let log_item = LogEntry {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        timestamp: time_str,
        category: category.to_string(),
        level: level.to_string(),
        message: message.to_string(),
    };

    // Store in ring buffer
    if let Ok(mut guard) = RECENT_LOGS.lock() {
        if guard.len() >= MAX_RECENT_LOGS {
            guard.pop_front();
        }
        guard.push_back(log_item.clone());
    }

    // Broadcast to SSE subscribers
    let _ = get_broadcast_sender().send(log_item);
}
