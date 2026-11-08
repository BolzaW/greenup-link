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

# Kiosk mode (open consultation):
# - false : (Default) Standard mode. Full authentication required on the Web GUI and all REST APIs (when api_token is set).
# - true  : Kiosk mode. Read-only GET APIs and Web GUI consultation are accessible without authentication.
#           Write/control actions (charge, current, commands, settings) remain protected by api_token.
kiosk_mode = false

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
# Sniffer mode (passive UART listener):
# - false : (Default) Active supervision — manages charging and communicates with the ATmega board.
# - true  : Sniffer mode — passive RX-only monitoring, zero TX commands are ever sent to the ATmega.
sniffer_mode = false

[mqtt]
# Enable or disable the MQTT client.
# Default: false
enabled = false

# MQTT broker host and port (e.g. Mosquitto / Home Assistant broker).
# Default: "127.0.0.1:1883"
broker = "127.0.0.1:1883"

# Optional MQTT username.
# When set (or when api_token is configured under [http]), greenup-link authenticates
# using `username` (defaults to "greenup-link" if empty) and `http.api_token` as password.
username = ""

# Base MQTT topic prefix.
# Default: "greenup"
base_topic = "greenup"

# Publish Home Assistant MQTT Auto-Discovery payloads (`homeassistant/.../config`).
# Default: true
ha_discovery = true
"#;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub bind: String,
    pub api_token: Option<String>,
    pub kiosk_mode: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            api_token: None,
            kiosk_mode: false,
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

#[derive(Debug, Clone, Default)]
pub struct StationConfig {
    pub sniffer_mode: bool,
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub enabled: bool,
    pub broker: String,
    pub username: Option<String>,
    pub base_topic: String,
    pub ha_discovery: bool,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            broker: "127.0.0.1:1883".to_string(),
            username: None,
            base_topic: "greenup".to_string(),
            ha_discovery: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub storage: StorageConfig,
    pub serial: SerialConfig,
    pub station: StationConfig,
    pub mqtt: MqttConfig,
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
                        "kiosk_mode" => {
                            match val.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" => config.http.kiosk_mode = true,
                                "false" | "0" | "no" => config.http.kiosk_mode = false,
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
                    if key.as_str() == "sniffer_mode" {
                        match val.trim().to_lowercase().as_str() {
                            "true" | "1" | "yes" => config.station.sniffer_mode = true,
                            "false" | "0" | "no" => config.station.sniffer_mode = false,
                            _ => {}
                        }
                    }
                } else if current_section == "mqtt" {
                    match key.as_str() {
                        "enabled" => {
                            match val.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" => config.mqtt.enabled = true,
                                "false" | "0" | "no" => config.mqtt.enabled = false,
                                _ => {}
                            }
                        }
                        "broker" => {
                            let trimmed = val.trim();
                            if !trimmed.is_empty() {
                                config.mqtt.broker = trimmed.to_string();
                            }
                        }
                        "username" => {
                            let trimmed = val.trim();
                            if trimmed.is_empty() {
                                config.mqtt.username = None;
                            } else {
                                config.mqtt.username = Some(trimmed.to_string());
                            }
                        }
                        "base_topic" => {
                            let trimmed = val.trim().trim_matches('/');
                            if !trimmed.is_empty() {
                                config.mqtt.base_topic = trimmed.to_string();
                            }
                        }
                        "ha_discovery" => {
                            match val.trim().to_lowercase().as_str() {
                                "true" | "1" | "yes" => config.mqtt.ha_discovery = true,
                                "false" | "0" | "no" => config.mqtt.ha_discovery = false,
                                _ => {}
                            }
                        }
                        _ => {}
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
        assert!(!cfg.http.kiosk_mode, "Default kiosk_mode should be false");
        assert!(!cfg.station.sniffer_mode, "Default sniffer_mode should be false");
        assert_eq!(cfg.storage.log_dir, "logs");
        assert_eq!(cfg.storage.sessions_dir, "sessions");
        assert!(!cfg.mqtt.enabled, "Default mqtt.enabled should be false");
        assert_eq!(cfg.mqtt.broker, "127.0.0.1:1883");
        assert_eq!(cfg.mqtt.username, None);
        assert_eq!(cfg.mqtt.base_topic, "greenup");
        assert!(cfg.mqtt.ha_discovery, "Default mqtt.ha_discovery should be true");
    }

    #[test]
    fn test_mqtt_config_parsing() {
        let toml = r#"
        [mqtt]
        enabled = true
        broker = "192.168.1.50:1883"
        username = "my_mqtt_user"
        base_topic = "home/evse"
        ha_discovery = false
        "#;
        let cfg = AppConfig::parse_toml(toml);
        assert!(cfg.mqtt.enabled);
        assert_eq!(cfg.mqtt.broker, "192.168.1.50:1883");
        assert_eq!(cfg.mqtt.username.as_deref(), Some("my_mqtt_user"));
        assert_eq!(cfg.mqtt.base_topic, "home/evse");
        assert!(!cfg.mqtt.ha_discovery);
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
    fn test_kiosk_and_sniffer_mode_parsing() {
        let toml_true = r#"
        [http]
        api_token = "secret123"
        kiosk_mode = true

        [station]
        sniffer_mode = true
        "#;
        let cfg1 = AppConfig::parse_toml(toml_true);
        assert_eq!(cfg1.http.api_token.as_deref(), Some("secret123"));
        assert!(cfg1.http.kiosk_mode);
        assert!(cfg1.station.sniffer_mode);

        let toml_false = r#"
        [http]
        kiosk_mode = "false"

        [station]
        sniffer_mode = "0"
        "#;
        let cfg2 = AppConfig::parse_toml(toml_false);
        assert!(!cfg2.http.kiosk_mode);
        assert!(!cfg2.station.sniffer_mode);

        let toml_one = r#"
        [http]
        kiosk_mode = "1"
        "#;
        let cfg3 = AppConfig::parse_toml(toml_one);
        assert!(cfg3.http.kiosk_mode);
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
