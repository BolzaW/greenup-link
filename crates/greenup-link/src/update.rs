use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use chrono::Local;
use greenup_driver::logger;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

pub const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/BolzaW/greenup-link/releases/latest";
pub const GITHUB_RELEASES_PAGE_URL: &str =
    "https://github.com/BolzaW/greenup-link/releases";

const MANUAL_CHECK_COOLDOWN_SECS: u64 = 30;
const RELEASE_NOTES_MAX_CHARS: usize = 1500;

/// High-level update availability status exposed in REST, MQTT, and Web UI.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// GitHub unreachable, check disabled, or not yet checked.
    #[default]
    Unknown,
    /// Running version is up to date (or ahead of latest release).
    UpToDate,
    /// A newer minor or patch release is available (or stable release for a `-dev` build).
    MinorAvailable,
    /// A newer major release is available.
    MajorAvailable,
}

impl UpdateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::UpToDate => "up_to_date",
            Self::MinorAvailable => "minor_available",
            Self::MajorAvailable => "major_available",
        }
    }

    pub fn is_update_available(&self) -> bool {
        matches!(self, Self::MinorAvailable | Self::MajorAvailable)
    }
}

/// Software update metadata serialized under `station.update`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub status: UpdateStatus,
    pub update_available: bool,
    pub current_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    pub release_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl UpdateInfo {
    pub fn initial(current_version: &str) -> Self {
        Self {
            status: UpdateStatus::Unknown,
            update_available: false,
            current_version: normalize_version_display(current_version),
            latest_version: None,
            release_url: GITHUB_RELEASES_PAGE_URL.to_string(),
            published_at: None,
            release_notes: None,
            last_checked: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    is_prerelease: bool,
}

impl SemVer {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches(['v', 'V']);
        if trimmed.is_empty() {
            return None;
        }
        let (core, is_prerelease) = match trimmed.split_once('-') {
            Some((c, pre)) => (c, !pre.trim().is_empty()),
            None => (trimmed, false),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            is_prerelease,
        })
    }
}

pub fn normalize_version_display(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).to_string()
}

/// Compares `current` against `latest` using SemVer rules.
/// - `latest.major > current.major` => `MajorAvailable`
/// - `latest.minor > current.minor` or `latest.patch > current.patch` => `MinorAvailable`
/// - `1.1.0-dev` vs `1.1.0` => `MinorAvailable` (final release available)
/// - `1.1.0-dev` vs `1.0.0` => `UpToDate` (local build is ahead)
pub fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
    let Some(cur) = SemVer::parse(current) else {
        return UpdateStatus::Unknown;
    };
    let Some(lat) = SemVer::parse(latest) else {
        return UpdateStatus::Unknown;
    };

    if lat.major > cur.major {
        return UpdateStatus::MajorAvailable;
    }
    if lat.major < cur.major {
        return UpdateStatus::UpToDate;
    }
    if lat.minor > cur.minor {
        return UpdateStatus::MinorAvailable;
    }
    if lat.minor < cur.minor {
        return UpdateStatus::UpToDate;
    }
    if lat.patch > cur.patch {
        return UpdateStatus::MinorAvailable;
    }
    if lat.patch < cur.patch {
        return UpdateStatus::UpToDate;
    }
    if cur.is_prerelease && !lat.is_prerelease {
        return UpdateStatus::MinorAvailable;
    }
    UpdateStatus::UpToDate
}

#[derive(Deserialize)]
struct GitHubReleasePayload {
    tag_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

fn truncate_notes(raw: Option<String>) -> Option<String> {
    let text = raw?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= RELEASE_NOTES_MAX_CHARS {
        return Some(text);
    }
    let truncated: String = text.chars().take(RELEASE_NOTES_MAX_CHARS).collect();
    Some(format!("{}…", truncated.trim_end()))
}

/// Parses a GitHub Releases API JSON response into [`UpdateInfo`].
pub fn parse_github_release_json(json_str: &str, current_version: &str) -> Result<UpdateInfo, String> {
    let payload: GitHubReleasePayload =
        serde_json::from_str(json_str).map_err(|e| format!("Invalid GitHub JSON: {}", e))?;

    if payload.draft || payload.prerelease {
        return Err("Latest release entry is draft or prerelease".to_string());
    }

    let latest_clean = normalize_version_display(&payload.tag_name);
    if SemVer::parse(&latest_clean).is_none() {
        return Err(format!("Unrecognized release tag '{}'", payload.tag_name));
    }

    let status = compare_versions(current_version, &latest_clean);
    let release_url = payload
        .html_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| GITHUB_RELEASES_PAGE_URL.to_string());

    Ok(UpdateInfo {
        status,
        update_available: status.is_update_available(),
        current_version: normalize_version_display(current_version),
        latest_version: Some(latest_clean),
        release_url,
        published_at: payload.published_at.filter(|s| !s.trim().is_empty()),
        release_notes: truncate_notes(payload.body),
        last_checked: Some(Local::now().to_rfc3339()),
        error: None,
    })
}

async fn fetch_url_text(url: &str, user_agent: &str) -> Result<String, String> {
    let curl_res = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "4",
            "--max-time",
            "8",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            &format!("User-Agent: {}", user_agent),
            url,
        ])
        .output()
        .await;

    match curl_res {
        Ok(out) if out.status.success() => {
            return String::from_utf8(out.stdout)
                .map_err(|e| format!("Invalid UTF-8 response: {}", e));
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // Fall through to wget if curl failed only because of binary issues; otherwise report error
            if !stderr.is_empty() {
                // Still try wget in case curl had TLS/environment issue on minimal Linux
                if let Ok(wget_out) = Command::new("wget")
                    .args([
                        "-qO-",
                        "--timeout=8",
                        "--header=Accept: application/vnd.github+json",
                        &format!("--header=User-Agent: {}", user_agent),
                        url,
                    ])
                    .output()
                    .await
                {
                    if wget_out.status.success() {
                        return String::from_utf8(wget_out.stdout)
                            .map_err(|e| format!("Invalid UTF-8 response: {}", e));
                    }
                }
                return Err(stderr);
            }
        }
        Err(curl_err) => {
            // `curl` not installed: fallback to `wget`
            if let Ok(wget_out) = Command::new("wget")
                .args([
                    "-qO-",
                    "--timeout=8",
                    "--header=Accept: application/vnd.github+json",
                    &format!("--header=User-Agent: {}", user_agent),
                    url,
                ])
                .output()
                .await
            {
                if wget_out.status.success() {
                    return String::from_utf8(wget_out.stdout)
                        .map_err(|e| format!("Invalid UTF-8 response: {}", e));
                }
                let stderr = String::from_utf8_lossy(&wget_out.stderr).trim().to_string();
                return Err(if stderr.is_empty() {
                    format!("wget exited with {}", wget_out.status)
                } else {
                    stderr
                });
            }
            return Err(format!("Failed to invoke curl: {}", curl_err));
        }
    }

    Err("GitHub request failed".to_string())
}

#[derive(Debug, Clone)]
pub enum ManualCheckResult {
    Checked(UpdateInfo),
    Cooldown {
        remaining_secs: u64,
        info: UpdateInfo,
    },
}

/// Manages cached GitHub release status and periodic/manual version checks.
pub struct UpdateManager {
    current_version: String,
    info: RwLock<UpdateInfo>,
    last_manual_check: Mutex<Option<Instant>>,
}

impl UpdateManager {
    pub fn new(current_version: &str) -> Self {
        Self {
            current_version: normalize_version_display(current_version),
            info: RwLock::new(UpdateInfo::initial(current_version)),
            last_manual_check: Mutex::new(None),
        }
    }

    pub fn snapshot(&self) -> UpdateInfo {
        self.info
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub async fn check_manual(&self) -> ManualCheckResult {
        {
            let mut guard = self
                .last_manual_check
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(prev) = *guard {
                let elapsed = prev.elapsed().as_secs();
                if elapsed < MANUAL_CHECK_COOLDOWN_SECS {
                    return ManualCheckResult::Cooldown {
                        remaining_secs: MANUAL_CHECK_COOLDOWN_SECS - elapsed,
                        info: self.snapshot(),
                    };
                }
            }
            *guard = Some(Instant::now());
        }

        ManualCheckResult::Checked(self.run_check().await)
    }

    pub async fn run_check(&self) -> UpdateInfo {
        let ua = format!("greenup-link/{}", self.current_version);
        let now_str = Local::now().to_rfc3339();

        let new_info = match fetch_url_text(GITHUB_RELEASES_API_URL, &ua).await {
            Ok(body) => match parse_github_release_json(&body, &self.current_version) {
                Ok(parsed) => {
                    match parsed.status {
                        UpdateStatus::UpToDate => {
                            logger::log(
                                "SYS",
                                &format!(
                                    "🔄 Update check: v{} is up to date (latest: v{})",
                                    parsed.current_version,
                                    parsed.latest_version.as_deref().unwrap_or("?")
                                ),
                            );
                        }
                        UpdateStatus::MinorAvailable => {
                            logger::warn(
                                "SYS",
                                &format!(
                                    "🟠 Update available: v{} → v{} ({})",
                                    parsed.current_version,
                                    parsed.latest_version.as_deref().unwrap_or("?"),
                                    parsed.release_url
                                ),
                            );
                        }
                        UpdateStatus::MajorAvailable => {
                            logger::warn(
                                "SYS",
                                &format!(
                                    "🔴 Major update available: v{} → v{} ({})",
                                    parsed.current_version,
                                    parsed.latest_version.as_deref().unwrap_or("?"),
                                    parsed.release_url
                                ),
                            );
                        }
                        UpdateStatus::Unknown => {}
                    }
                    parsed
                }
                Err(err) => {
                    logger::debug("SYS", &format!("Update check parse warning: {}", err));
                    UpdateInfo {
                        status: UpdateStatus::Unknown,
                        update_available: false,
                        current_version: self.current_version.clone(),
                        latest_version: None,
                        release_url: GITHUB_RELEASES_PAGE_URL.to_string(),
                        published_at: None,
                        release_notes: None,
                        last_checked: Some(now_str),
                        error: Some(err),
                    }
                }
            },
            Err(err) => {
                logger::debug(
                    "SYS",
                    &format!("Update check failed (GitHub unreachable): {}", err),
                );
                UpdateInfo {
                    status: UpdateStatus::Unknown,
                    update_available: false,
                    current_version: self.current_version.clone(),
                    latest_version: None,
                    release_url: GITHUB_RELEASES_PAGE_URL.to_string(),
                    published_at: None,
                    release_notes: None,
                    last_checked: Some(now_str),
                    error: Some(err),
                }
            }
        };

        if let Ok(mut guard) = self.info.write() {
            *guard = new_info.clone();
        }
        new_info
    }
}

/// Spawns the background update checker after station initialization.
/// Retries quickly (+60s, +5min) if GitHub was unreachable at boot (e.g. router still booting),
/// then checks every 24 hours.
pub fn spawn_update_checker(manager: std::sync::Arc<UpdateManager>) {
    tokio::spawn(async move {
        let first = manager.run_check().await;
        if first.status == UpdateStatus::Unknown {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let second = manager.run_check().await;
            if second.status == UpdateStatus::Unknown {
                tokio::time::sleep(Duration::from_secs(300)).await;
                let _ = manager.run_check().await;
            }
        }

        loop {
            tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
            let _ = manager.run_check().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_semver() {
        assert_eq!(compare_versions("1.1.0", "1.1.0"), UpdateStatus::UpToDate);
        assert_eq!(compare_versions("1.1.0", "1.0.0"), UpdateStatus::UpToDate);
        assert_eq!(compare_versions("1.1.0-dev", "1.0.0"), UpdateStatus::UpToDate);
        assert_eq!(compare_versions("1.1.0-dev", "1.1.0"), UpdateStatus::MinorAvailable);
        assert_eq!(compare_versions("1.1.0", "1.1.1"), UpdateStatus::MinorAvailable);
        assert_eq!(compare_versions("1.1.0", "1.2.0"), UpdateStatus::MinorAvailable);
        assert_eq!(compare_versions("1.1.0", "2.0.0"), UpdateStatus::MajorAvailable);
        assert_eq!(compare_versions("1.9.5-dev", "2.0.0"), UpdateStatus::MajorAvailable);
        assert_eq!(compare_versions("bad", "1.0.0"), UpdateStatus::Unknown);
    }

    #[test]
    fn test_parse_github_release_json() {
        let json = r#"{
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/BolzaW/greenup-link/releases/tag/v1.2.0",
            "published_at": "2026-11-19T14:00:00Z",
            "body": "New feature release",
            "draft": false,
            "prerelease": false
        }"#;

        let info = parse_github_release_json(json, "1.1.0").unwrap();
        assert_eq!(info.status, UpdateStatus::MinorAvailable);
        assert!(info.update_available);
        assert_eq!(info.current_version, "1.1.0");
        assert_eq!(info.latest_version.as_deref(), Some("1.2.0"));
        assert_eq!(
            info.release_url,
            "https://github.com/BolzaW/greenup-link/releases/tag/v1.2.0"
        );
        assert_eq!(info.release_notes.as_deref(), Some("New feature release"));
    }
}
