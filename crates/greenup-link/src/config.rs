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

[serial]
# Serial port connected to the charging station microcontroller.
# Default: "/dev/ttyUSB0"
port = "/dev/ttyUSB0"

# Baud rate for communication with the ATmega2560 board.
# Default: 115200
baud_rate = 115200

# Silence timeout before alerting on loss of communication (seconds).
# Default: 6
watchdog_timeout_secs = 6
"#;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub bind: String,
    pub api_token: Option<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".to_string(),
            api_token: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub port: String,
    pub baud_rate: u32,
    pub watchdog_timeout_secs: u64,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 115_200,
            watchdog_timeout_secs: 6,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub serial: SerialConfig,
}

impl AppConfig {
    /// Loads configuration from `greenup.toml` (or path in `GREENUP_CONFIG`)
    /// and applies environment variable overrides (`GREENUP_BIND`, `GREENUP_API_TOKEN`).
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

        // Environment variable overrides
        if let Ok(env_bind) = env::var("GREENUP_BIND") {
            let trimmed = env_bind.trim();
            if !trimmed.is_empty() {
                config.http.bind = trimmed.to_string();
            }
        }

        if let Ok(env_token) = env::var("GREENUP_API_TOKEN") {
            let trimmed = env_token.trim();
            if trimmed.is_empty() {
                config.http.api_token = None;
            } else {
                config.http.api_token = Some(trimmed.to_string());
            }
        }

        if let Ok(env_port) = env::var("GREENUP_SERIAL_PORT") {
            let trimmed = env_port.trim();
            if !trimmed.is_empty() {
                config.serial.port = trimmed.to_string();
            }
        }

        config
    }

    /// Returns the resolved path to the `greenup.toml` file.
    pub fn get_config_path() -> String {
        env::var("GREENUP_CONFIG")
            .unwrap_or_else(|_| {
                if Path::new("/opt/greenup-link/greenup.toml").exists() {
                    "/opt/greenup-link/greenup.toml".to_string()
                } else if Path::new(DEFAULT_CONFIG_FILENAME).exists() {
                    DEFAULT_CONFIG_FILENAME.to_string()
                } else {
                    DEFAULT_CONFIG_FILENAME.to_string()
                }
            })
    }

    /// Saves the current configuration to `greenup.toml` on disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_path = Self::get_config_path();
        let path = Path::new(&config_path);
        let content = self.to_toml_string();
        fs::write(path, content)
    }

    /// Serializes configuration into a clean TOML string with documentation comments.
    pub fn to_toml_string(&self) -> String {
        let token_val = self.http.api_token.as_deref().unwrap_or("");
        format!(
r#"# ==============================================================================
# Green'Up Link - Configuration File
# ==============================================================================

[http]
# Network interface and port to listen on.
# Default: "0.0.0.0:8080" (all network interfaces).
bind = "{bind}"

# Optional API token / password to protect the Web GUI and REST API.
# - Leave empty ("") to disable authentication (open access on trusted LAN).
# - Set a secret string to require authentication on all /api/* routes.
#   Compatible with:
#     * Bearer Token : Authorization: Bearer <api_token>  (EVCC, Home Assistant, Web GUI)
#     * API Key      : X-API-Key: <api_token>
#     * Basic Auth   : Authorization: Basic <base64(user:api_token)>  (curl -u :token)
api_token = "{token}"

[serial]
# Serial port connected to the charging station microcontroller.
# Default: "/dev/ttyUSB0"
port = "{port}"

# Baud rate for communication with the ATmega2560 board.
# Default: 115200
baud_rate = {baud}

# Silence timeout before alerting on loss of communication (seconds).
# Default: 6
watchdog_timeout_secs = {watchdog}
"#,
            bind = self.http.bind,
            token = token_val,
            port = self.serial.port,
            baud = self.serial.baud_rate,
            watchdog = self.serial.watchdog_timeout_secs
        )
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
                        _ => {}
                    }
                } else if current_section == "serial" {
                    match key.as_str() {
                        "port" => {
                            if !val.is_empty() {
                                config.serial.port = val;
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
}
