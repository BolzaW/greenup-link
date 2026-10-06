use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

/// Logger centralisé qui écrit dans un fichier horodaté + stdout.
/// Un nouveau fichier est créé à chaque démarrage du programme.
static LOG_FILE: Mutex<Option<String>> = Mutex::new(None);

/// Initialise le logger en créant le fichier de log.
/// Le fichier est nommé avec la date/heure de démarrage.
/// Exemple : logs/greenup_2026-09-30_23-15-42.log
pub fn init() {
    let log_dir = "logs";
    let _ = fs::create_dir_all(log_dir);

    let now = Local::now();
    let filename = format!("{}/greenup_{}.log", log_dir, now.format("%Y-%m-%d_%H-%M-%S"));

    // Crée le fichier
    if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(&filename) {
        let _ = writeln!(f, "=== GreenUp Logger démarré le {} ===", now.format("%d/%m/%Y à %H:%M:%S"));
    }

    if let Ok(mut guard) = LOG_FILE.lock() {
        *guard = Some(filename.clone());
    }

    println!("[Logger] 📝 Fichier de log : {}", filename);
}

/// Écrit une entrée dans le fichier de log ET dans stdout.
///   category : "SERIE_RX", "SERIE_TX", "API", "IHM", "SYS"
///   message  : le contenu à logger
pub fn log(category: &str, message: &str) {
    let now = Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f");
    let entry = format!("[{}] [{}] {}", timestamp, category, message);

    // Stdout (console)
    if category != "TRACE" {
        println!("{}", entry);
    }

    // Fichier
    if let Ok(guard) = LOG_FILE.lock() {
        if let Some(ref path) = *guard {
            if let Ok(mut f) = OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "{}", entry);
            }
        }
    }
}
