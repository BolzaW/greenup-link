use std::env;
use std::fs;
use std::path::Path;
use greenup_driver::logger;

pub const DEFAULT_CONFIG_FILENAME: &str = "greenup.toml";

pub const DEFAULT_CONFIG_TEMPLATE: &str = r#"# ==============================================================================
# Green'Up Link - Configuration File
# ==============================================================================

[http]
# Network interface and port to listen on.
# Default: "0.0.0.0:8080" (all network interfaces).
bind = "0.0.0.0:8080"

# Optional API token / password to protect the Web GUI and REST API.
# - Leave empty ("") to disable authentication (open access on trusted LAN).
# - Set a secret string to require authentication on all /api/* routes.
#   Compatible with:
#     * Bearer Token : Authorization: Bearer <api_token>  (EVCC, Home Assistant, Web GUI)
#     * API Key      : X-API-Key: <api_token>
#     * Basic Auth   : Authorization: Basic <base64(user:api_token)>  (curl -u :token)
api_token = ""

# Web GUI authentication mode:
# - true  : (Default) Full authentication required to view and interact with the Web GUI.
# - false : Open consultation mode ("Kiosk"). Allows browsing dashboard, logs, sessions,
#           and station metrics without login. Control actions (charge, current, commands,
#           settings) remain strictly protected by api_token.
auth_ihm = true

[storage]
# Directory where daily log files are stored.
# Default: "logs" (or "/var/log/greenup-link" if running as system service)
log_dir = "logs"

# Directory where charging sessions (CSV data and summaries) are stored.
# Default: "sessions" (or "/var/lib/greenup-link/sessions")
sessions_dir = "sessions"

[serial]
# Dual charge point mode:
# - "auto"  : Automatically detects 1 or 2 charge points based on the station hardware reference (Default)
# - "true"  : Force 2 charge points mode
# - "false" : Force 1 charge point mode
dual_point = "auto"

# Optional serial port(s) to bind.
# If omitted or left empty, Green'Up Link automatically discovers available USB serial ports.
# port = "/dev/ttyUSB0"
# port2 = "/dev/ttyUSB1"

# Baud rate for communication with the ATmega2560 board.
# Default: 115200
baud_rate = 115200

# Silence timeout before alerting on loss of communication (seconds).
# Default: 60
watchdog_timeout_secs = 60

[station]
# Supervision mode at startup:
# - "remote" : Active supervision (default) - manages charging and communicates with ATmega
# - "local"  : Passive monitor mode - listens to serial telemetry only, zero transmissions to ATmega
mode = "remote"
"#;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub bind: String,
    pub api_token: Option<String>,
    pub auth_ihm: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            api_token: None,
            auth_ihm: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub log_dir: String,
    pub sessions_dir: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            log_dir: "logs".to_string(),
            sessions_dir: "sessions".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualPointMode {
    Auto,
    True,
    False,
}

impl Default for DualPointMode {
    fn default() -> Self {
        DualPointMode::Auto
    }
}

impl DualPointMode {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            DualPointMode::Auto => "auto",
            DualPointMode::True => "true",
            DualPointMode::False => "false",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub dual_point: DualPointMode,
    pub port: Option<String>,
    pub port2: Option<String>,
    pub baud_rate: u32,
    pub watchdog_timeout_secs: u64,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            dual_point: DualPointMode::Auto,
            port: None,
            port2: None,
            baud_rate: 115_200,
            watchdog_timeout_secs: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StationConfig {
    pub mode: crate::telemetry::SupervisionMode,
}

impl Default for StationConfig {
    fn default() -> Self {
        Self {
            mode: crate::telemetry::SupervisionMode::Remote,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub storage: StorageConfig,
    pub serial: SerialConfig,
    pub station: StationConfig,
}

impl AppConfig {
    /// Loads configuration from `greenup.toml` (or path in `GREENUP_CONFIG`)
    /// and applies environment variable overrides (`GREENUP_API_TOKEN`).
    pub fn load() -> Self {
        let config_path = env::var("GREENUP_CONFIG")
            .unwrap_or_else(|_| {
                if Path::new(DEFAULT_CONFIG_FILENAME).exists() {
                    DEFAULT_CONFIG_FILENAME.to_string()
                } else if Path::new("/opt/greenup-link/greenup.toml").exists() {
                    "/opt/greenup-link/greenup.toml".to_string()
                } else {
                    DEFAULT_CONFIG_FILENAME.to_string()
                }
            });
        let path = Path::new(&config_path);

        let mut config = if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    logger::log("SYS", &format!("📄 Configuration loaded from '{}'", config_path));
                    Self::parse_toml(&content)
                }
                Err(err) => {
                    logger::log(
                        "SYS",
                        &format!("⚠️ Failed to read '{}': {}. Using defaults.", config_path, err),
                    );
                    Self::default()
                }
            }
        } else {
            // Create default configuration file on disk for user convenience
            if let Err(err) = fs::write(path, DEFAULT_CONFIG_TEMPLATE) {
                logger::log(
                    "DEBUG",
                    &format!("Could not write default '{}': {}", config_path, err),
                );
            } else {
                logger::log("SYS", &format!("📄 Created default configuration file '{}'", config_path));
            }
            Self::default()
        };

        // Environment variable override (API token only, for security/secrets injection)
        if let Ok(env_token) = env::var("GREENUP_API_TOKEN") {
            let trimmed = env_token.trim();
            if trimmed.is_empty() {
                config.http.api_token = None;
            } else {
                config.http.api_token = Some(trimmed.to_string());
            }
        }

        config
    }

    /// Minimal, zero-dependency parser for `greenup.toml`.
    pub fn parse_toml(content: &str) -> Self {
        let mut config = Self::default();
        let mut current_section = String::new();

        for raw_line in content.lines() {
            let line = strip_toml_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim().to_lowercase();
                continue;
            }

            if let Some((raw_key, raw_val)) = line.split_once('=') {
                let key = raw_key.trim().to_lowercase();
                let val = parse_toml_string_value(raw_val.trim());

                if current_section.is_empty() || current_section == "http" {
                    match key.as_str() {
                        "bind" => {
                            if !val.is_empty() {
                                config.http.bind = val;
                            }
                        }
                        "api_token" => {
                            let trimmed = val.trim();
                            if trimmed.is_empty() {
                                config.http.api_token = None;
                            } else {
                                config.http.api_token = Some(trimmed.to_string());
                            }
                        }
                        "auth_ihm" => {
                            match val.trim().to_lowercase().as_str() {
                                "false" | "0" | "no" => config.http.auth_ihm = false,
                                "true" | "1" | "yes" => config.http.auth_ihm = true,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                } else if current_section == "storage" {
                    match key.as_str() {
                        "log_dir" => {
                            if !val.is_empty() {
                                config.storage.log_dir = val;
                            }
                        }
                        "sessions_dir" => {
                            if !val.is_empty() {
                                config.storage.sessions_dir = val;
                            }
                        }
                        _ => {}
                    }
                } else if current_section == "serial" {
                    match key.as_str() {
                        "dual_point" => {
                            match val.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" => config.serial.dual_point = DualPointMode::True,
                                "false" | "0" | "no" => config.serial.dual_point = DualPointMode::False,
                                _ => config.serial.dual_point = DualPointMode::Auto,
                            }
                        }
                        "port" => {
                            let trimmed = val.trim();
                            if trimmed.is_empty() || trimmed == "auto" {
                                config.serial.port = None;
                            } else {
                                config.serial.port = Some(trimmed.to_string());
                            }
                        }
                        "port2" => {
                            let trimmed = val.trim();
                            if trimmed.is_empty() || trimmed == "auto" {
                                config.serial.port2 = None;
                            } else {
                                config.serial.port2 = Some(trimmed.to_string());
                            }
                        }
                        "baud_rate" => {
                            if let Ok(b) = val.parse::<u32>() {
                                config.serial.baud_rate = b;
                            }
                        }
                        "watchdog_timeout_secs" => {
                            if let Ok(w) = val.parse::<u64>() {
                                config.serial.watchdog_timeout_secs = w;
                            }
                        }
                        _ => {}
                    }
                } else if current_section == "station" {
                    if key.as_str() == "mode" {
                        if let Some(m) = crate::telemetry::SupervisionMode::from_str(&val) {
                            config.station.mode = m;
                        }
                    }
                }
            }
        }

        config
    }
}

/// Strips `#` comments that occur outside of quoted strings.
fn strip_toml_comment(line: &str) -> &str {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double_quote {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        } else if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if ch == '#' && !in_single_quote && !in_double_quote {
            return &line[..idx];
        }
    }
    line
}

/// Extracts string value from `"..."`, `'...'`, or unquoted value.
fn parse_toml_string_value(raw: &str) -> String {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        let inner = &raw[1..raw.len() - 1];
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        raw[1..raw.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_template_parses_cleanly() {
        let cfg = AppConfig::parse_toml(DEFAULT_CONFIG_TEMPLATE);
        assert_eq!(cfg.http.bind, "0.0.0.0:8080");
        assert_eq!(cfg.http.api_token, None);
        assert!(cfg.http.auth_ihm, "Default auth_ihm should be true");
        assert_eq!(cfg.storage.log_dir, "logs");
        assert_eq!(cfg.storage.sessions_dir, "sessions");
    }

    #[test]
    fn test_storage_config_parsing() {
        let toml = r#"
        [storage]
        log_dir = "/var/log/greenup-link"
        sessions_dir = "/var/lib/greenup-link/sessions"
        "#;
        let cfg = AppConfig::parse_toml(toml);
        assert_eq!(cfg.storage.log_dir, "/var/log/greenup-link");
        assert_eq!(cfg.storage.sessions_dir, "/var/lib/greenup-link/sessions");
    }

    #[test]
    fn test_auth_ihm_parsing() {
        let toml_false = r#"
        [http]
        api_token = "secret123"
        auth_ihm = false
        "#;
        let cfg1 = AppConfig::parse_toml(toml_false);
        assert_eq!(cfg1.http.api_token.as_deref(), Some("secret123"));
        assert!(!cfg1.http.auth_ihm);

        let toml_true = r#"
        [http]
        auth_ihm = "true"
        "#;
        let cfg2 = AppConfig::parse_toml(toml_true);
        assert!(cfg2.http.auth_ihm);

        let toml_zero = r#"
        [http]
        auth_ihm = "0"
        "#;
        let cfg3 = AppConfig::parse_toml(toml_zero);
        assert!(!cfg3.http.auth_ihm);
    }

    #[test]
    fn test_custom_token_and_bind() {
        let toml = r#"
        [http]
        bind = "127.0.0.1:3000" # local only
        api_token = "my_super_secret#token" # inline comment
        "#;
        let cfg = AppConfig::parse_toml(toml);
        assert_eq!(cfg.http.bind, "127.0.0.1:3000");
        assert_eq!(cfg.http.api_token.as_deref(), Some("my_super_secret#token"));
    }

    #[test]
    fn test_single_quotes_and_empty_token() {
        let toml = r#"
        [http]
        api_token = '   '
        "#;
        let cfg = AppConfig::parse_toml(toml);
        assert_eq!(cfg.http.api_token, None);
    }

    #[test]
    fn test_dual_port_config_parsing() {
        let toml = r#"
        [serial]
        dual_point = "true"
        port = "/dev/ttyUSB0"
        port2 = "/dev/ttyUSB1"
        "#;
        let cfg = AppConfig::parse_toml(toml);
        assert_eq!(cfg.serial.dual_point, DualPointMode::True);
        assert_eq!(cfg.serial.port.as_deref(), Some("/dev/ttyUSB0"));
        assert_eq!(cfg.serial.port2.as_deref(), Some("/dev/ttyUSB1"));

        let toml_auto = r#"
        [serial]
        dual_point = "auto"
        "#;
        let cfg_auto = AppConfig::parse_toml(toml_auto);
        assert_eq!(cfg_auto.serial.dual_point, DualPointMode::Auto);
        assert_eq!(cfg_auto.serial.port, None);
        assert_eq!(cfg_auto.serial.port2, None);
    }
}
