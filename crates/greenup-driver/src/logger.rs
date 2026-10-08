use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

/// Centralized logger that writes to a timestamped file + stdout.
/// A new file is created at each program startup.
static LOG_FILE: Mutex<Option<String>> = Mutex::new(None);

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

/// Writes an entry to the log file AND to stdout.
///   category : "SERIE_RX", "SERIE_TX", "API", "IHM", "SYS"
///   message  : the content to log
pub fn log(category: &str, message: &str) {
    let now = Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    let entry = format!("[{}] [{}] {}", timestamp, category, message);

    // Stdout (console)
    if category != "TRACE" {
        println!("{}", entry);
    }

    // File
    if let Ok(guard) = LOG_FILE.lock() {
        if let Some(ref path) = *guard {
            if let Ok(mut f) = OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "{}", entry);
            }
        }
    }
}
