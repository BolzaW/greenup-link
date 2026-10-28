use chrono::{DateTime, Local};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::logger;
use crate::telemetry::Telemetry;

/// Returns true if the state indicates an immediate session termination
/// (unlatched, fault, power outage, or emergency stop).
pub fn is_immediate_termination_state(state: &str) -> bool {
    matches!(state, "L" | "R" | "ST" | "S" | "T" | "U" | "V" | "W" | "X" | "Y" | "Z")
}

/// Returns true if the state indicates a connected vehicle session.
pub fn is_connected_state(state: &str) -> bool {
    !is_immediate_termination_state(state) && state != "A" && state != "DISCONNECTED"
}

/// Formats duration in seconds into a human-readable string (e.g. "2h 10m 18s" or "45m 12s" or "32s").
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{}h {:02}m {:02}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {:02}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

static CUSTOM_SESSIONS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Overrides the sessions directory (primarily for testing or custom configuration).
pub fn set_custom_sessions_dir(dir: Option<PathBuf>) {
    *CUSTOM_SESSIONS_DIR.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}

/// Returns the configured sessions directory path.
/// Checks custom override, `GREENUP_SESSIONS_DIR` env var, `/opt/greenup-link/sessions`, or local `sessions/`.
pub fn get_sessions_dir() -> PathBuf {
    if let Some(ref dir) = *CUSTOM_SESSIONS_DIR.lock().unwrap_or_else(|e| e.into_inner()) {
        return dir.clone();
    }
    if let Ok(dir) = std::env::var("GREENUP_SESSIONS_DIR") {
        return PathBuf::from(dir);
    }
    if Path::new("/opt/greenup-link/sessions").exists() {
        return PathBuf::from("/opt/greenup-link/sessions");
    }
    if Path::new("/opt/greenup-link").exists() {
        return PathBuf::from("/opt/greenup-link/sessions");
    }
    PathBuf::from("sessions")
}

/// Tracks the active charging session and manages adaptive CSV logging.
pub struct ActiveSession {
    pub session_id: String,
    pub start_time: DateTime<Local>,
    pub start_instant: Instant,
    pub last_sample_instant: Option<Instant>,
    pub last_sample_current: f32,
    pub last_sample_state: String,
    pub max_current: f32,
    pub max_power: f32,
    pub cumulative_energy_wh: f64,
    pub active_charging_duration_secs_f64: f64,
    pub last_tick_instant: Instant,
    pub phase: String,
    pub legrand_ticket: Option<String>,
    pub csv_path: PathBuf,
    pub txt_path: PathBuf,
    pub csv_writer: Option<File>,
    pub sample_count: usize,
    pub pending_state_a_since: Option<Instant>,
}

impl ActiveSession {
    /// Determines whether a new sample should be recorded to CSV.
    ///
    /// Adaptive high-fidelity rules:
    /// - Immediate sampling on `State` change.
    /// - 1 sample per second during transitions (measured current variation > 0.5A).
    /// - 1 sample every 60 seconds during steady state (variation <= 0.5A or idle/waiting).
    pub fn should_sample(&self, state: &str, current: f32, now: Instant) -> bool {
        let state_changed = state != self.last_sample_state;
        let current_diff = (current - self.last_sample_current).abs();
        let current_changed = current_diff > 0.5;

        match self.last_sample_instant {
            None => true,
            Some(last) => {
                let elapsed = now.duration_since(last);
                if state_changed {
                    true
                } else if current_changed {
                    elapsed >= Duration::from_millis(900)
                } else {
                    elapsed >= Duration::from_secs(60)
                }
            }
        }
    }

    /// Computes total energy in kWh from the highest available source
    /// (local integration or telemetry totalizer).
    pub fn get_total_energy_kwh(&self, tel: &Telemetry) -> f64 {
        let session_kwh = self.cumulative_energy_wh / 1000.0;
        let tel_kwh = tel.energy as f64 / 1000.0;
        f64::max(session_kwh, tel_kwh)
    }

    /// Records a single CSV data row and flushes to disk.
    pub fn record_sample(&mut self, tel: &Telemetry, now_instant: Instant, now_local: DateTime<Local>) {
        let elapsed_s = now_instant.duration_since(self.start_instant).as_secs();
        let energy_kwh = self.get_total_energy_kwh(tel);

        if let Some(ref mut writer) = self.csv_writer {
            let line = format!(
                "{},{},{},{:.2},{},{:.1},{:.0},{:.3}\n",
                now_local.to_rfc3339(),
                elapsed_s,
                tel.greenup_state,
                tel.current,
                tel.limit_amps,
                tel.voltage,
                tel.power,
                energy_kwh
            );
            if let Err(e) = writer.write_all(line.as_bytes()) {
                logger::error("SESSION", &format!("Failed to write CSV record: {}", e));
            } else {
                let _ = writer.flush();
            }
        }

        self.last_sample_instant = Some(now_instant);
        self.last_sample_current = tel.current;
        self.last_sample_state = tel.greenup_state.clone();
        self.sample_count += 1;
    }
}

static ACTIVE_SESSION: Mutex<Option<ActiveSession>> = Mutex::new(None);
static LAST_CLOSED_SESSION: Mutex<Option<(PathBuf, Instant)>> = Mutex::new(None);

/// Returns true if a charging session is currently active.
pub fn is_active() -> bool {
    ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Returns the current active session ID, if any.
pub fn get_active_session_id() -> Option<String> {
    ACTIVE_SESSION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|s| s.session_id.clone())
}

/// Starts a new session when vehicle is plugged in.
pub fn start_session(initial_state: &str, tel: &Telemetry) {
    let mut lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());

    // If an existing session is somehow still active, finalize it first
    if lock.is_some() {
        logger::warn("SESSION", "Previous session was still open, closing it before starting new session");
        drop(lock);
        end_session("Previous session superseded by new plug event", tel);
        lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    }

    let sessions_dir = get_sessions_dir();
    if let Err(e) = fs::create_dir_all(&sessions_dir) {
        logger::error("SESSION", &format!("Failed to create sessions directory {:?}: {}", sessions_dir, e));
        return;
    }

    let now_local = Local::now();
    let now_instant = Instant::now();
    let session_id = format!("session_{}", now_local.format("%Y-%m-%d_%H-%M-%S"));
    let csv_path = sessions_dir.join(format!("{}.csv", session_id));
    let txt_path = sessions_dir.join(format!("{}.txt", session_id));

    let mut writer = match OpenOptions::new().create(true).write(true).truncate(true).open(&csv_path) {
        Ok(file) => Some(file),
        Err(e) => {
            logger::error("SESSION", &format!("Failed to create CSV file {:?}: {}", csv_path, e));
            None
        }
    };

    // Write standard CSV header
    if let Some(ref mut f) = writer {
        let header = "timestamp,elapsed_s,state,current_a,limit_a,voltage_v,power_w,energy_kwh\n";
        let _ = f.write_all(header.as_bytes());
        let _ = f.flush();
    }

    logger::info("SESSION", &format!("Started charging session {} (initial state: {})", session_id, initial_state));

    let mut session = ActiveSession {
        session_id,
        start_time: now_local,
        start_instant: now_instant,
        last_sample_instant: None,
        last_sample_current: tel.current,
        last_sample_state: initial_state.to_string(),
        max_current: tel.current,
        max_power: tel.power,
        cumulative_energy_wh: 0.0,
        active_charging_duration_secs_f64: 0.0,
        last_tick_instant: now_instant,
        phase: "Mono".to_string(),
        legrand_ticket: None,
        csv_path,
        txt_path,
        csv_writer: writer,
        sample_count: 0,
        pending_state_a_since: None,
    };

    // Record initial point (t = 0)
    session.record_sample(tel, now_instant, now_local);

    *lock = Some(session);
}

/// Called when `State:` changes while session is active.
pub fn on_state_change(new_state: &str, tel: &Telemetry) {
    let mut lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref mut session) = *lock {
        let now_instant = Instant::now();
        let now_local = Local::now();

        if new_state == "A" {
            if session.pending_state_a_since.is_none() {
                session.pending_state_a_since = Some(now_instant);
                logger::info("SESSION", "Entered State:A, waiting up to 60s before stopping session");
            }
        } else if let Some(since) = session.pending_state_a_since.take() {
            let elapsed_secs = since.elapsed().as_secs();
            logger::info(
                "SESSION",
                &format!("State:A ended after {}s, continuing session in State:{}", elapsed_secs, new_state),
            );
        }

        session.record_sample(tel, now_instant, now_local);
        session.last_sample_state = new_state.to_string();
    }
}

/// Checks if an active session has been in State A for more than 60 seconds.
/// If so, the session is formally terminated and returns true.
pub fn check_state_a_timeout(tel: &Telemetry) -> bool {
    let should_terminate = {
        let lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref session) = *lock {
            if let Some(since) = session.pending_state_a_since {
                since.elapsed() >= Duration::from_secs(60)
            } else {
                false
            }
        } else {
            false
        }
    };

    if should_terminate {
        logger::info("SESSION", "State:A persisted for > 60s, stopping session");
        end_session("Session stopped (State: A > 60s)", tel);
        true
    } else {
        false
    }
}

/// Called on every telemetry tick (e.g. `CCI:` reading) to update metrics and write samples adaptively.
pub fn on_telemetry_tick(tel: &Telemetry) {
    if check_state_a_timeout(tel) {
        return;
    }

    let mut lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref mut session) = *lock {
        let now_instant = Instant::now();
        let now_local = Local::now();

        // 1. Durations and energy integration
        let dt = now_instant.duration_since(session.last_tick_instant).as_secs_f64();
        session.last_tick_instant = now_instant;

        let is_charging = tel.greenup_state == "D" || tel.greenup_state == "E";
        if is_charging && tel.current > 0.05 {
            session.active_charging_duration_secs_f64 += dt;
            let wh = (tel.power as f64) * (dt / 3600.0);
            session.cumulative_energy_wh += wh;
        }

        // 2. Track peak metrics
        if tel.current > session.max_current {
            session.max_current = tel.current;
        }
        if tel.power > session.max_power {
            session.max_power = tel.power;
        }

        // 3. Adaptive sampling check
        if session.should_sample(&tel.greenup_state, tel.current, now_instant) {
            session.record_sample(tel, now_instant, now_local);
        }
    }
}

/// Updates the detected electrical phase ("Mono" or "Tri").
pub fn update_phase(phase: &str) {
    let mut lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref mut session) = *lock {
        session.phase = phase.to_string();
    }
}

/// Records a raw Legrand session ticket (`WT:...:CT:...:EVplug:...`).
pub fn record_ticket(ticket: &str) {
    let mut lock = ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref mut session) = *lock {
        session.legrand_ticket = Some(ticket.to_string());
    } else {
        // Late arrival tolerance: check if a session closed in the last 10 seconds
        let closed = LAST_CLOSED_SESSION.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((ref path, instant)) = *closed {
            if instant.elapsed() < Duration::from_secs(10) && path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if content.contains("N/A (Ticket not emitted)") {
                        let updated = content.replace("N/A (Ticket not emitted)", ticket);
                        let _ = fs::write(path, updated);
                        logger::info("SESSION", &format!("Attached late Legrand ticket to {:?}", path));
                    }
                }
            }
        }
    }
}

/// Ends the active session and creates the companion TXT summary.
/// If no charge actually took place (< 0.02 kWh and < 15s charge), discards the session files.
pub fn end_session(reason: &str, tel: &Telemetry) {
    let mut session = match ACTIVE_SESSION.lock().unwrap_or_else(|e| e.into_inner()).take() {
        Some(s) => s,
        None => return,
    };

    let now_instant = Instant::now();
    let now_local = Local::now();
    let final_energy_kwh = session.get_total_energy_kwh(tel);

    // Record final data point
    session.record_sample(tel, now_instant, now_local);

    // Flush and close CSV file
    if let Some(mut writer) = session.csv_writer.take() {
        let _ = writer.flush();
    }

    let active_duration_secs = session.active_charging_duration_secs_f64.round() as u64;
    let total_duration_secs = now_instant.duration_since(session.start_instant).as_secs();

    // Check discard threshold (< 0.02 kWh and < 15s active charge)
    let is_negligible = final_energy_kwh < 0.02 && active_duration_secs < 15;

    if is_negligible {
        if session.csv_path.exists() {
            let _ = fs::remove_file(&session.csv_path);
        }
        logger::info(
            "SESSION",
            &format!(
                "Discarded negligible charging session {} (< 0.02 kWh and < 15s charge)",
                session.session_id
            ),
        );
        return;
    }

    let idle_duration_secs = total_duration_secs.saturating_sub(active_duration_secs);

    let phase_str = match session.phase.as_str() {
        "Mono" | "1" => "Mono (1-Phase)",
        "Tri" | "3" => "Tri (3-Phase)",
        other if !other.is_empty() => other,
        _ => "Mono (1-Phase)",
    };

    let avg_active_power_w = if active_duration_secs > 0 && final_energy_kwh > 0.0 {
        (final_energy_kwh * 1000.0) / (active_duration_secs as f64 / 3600.0)
    } else {
        0.0
    };

    let ticket_str = session.legrand_ticket.as_deref().unwrap_or("N/A (Ticket not emitted)");

    let summary_content = format!(
"================================================================================
GREENUP-LINK CHARGING SESSION SUMMARY
================================================================================
Session ID:           {}
Start Time:           {}
End Time:             {}
Termination Reason:   {}

--------------------------------------------------------------------------------
DURATIONS
--------------------------------------------------------------------------------
Total Connected:      {} ({} s)
Active Charging:      {} ({} s)
Idle / Waiting:       {} ({} s)

--------------------------------------------------------------------------------
ENERGY & ELECTRICAL METRICS
--------------------------------------------------------------------------------
Total Energy:         {:.3} kWh
Phase Detection:      {}
Max Current:          {:.2} A
Max Power:            {:.0} W
Average Active Power: {:.0} W

--------------------------------------------------------------------------------
RAW LEGRAND TICKET
--------------------------------------------------------------------------------
{}
================================================================================
",
        session.session_id,
        session.start_time.format("%Y-%m-%d %H:%M:%S"),
        now_local.format("%Y-%m-%d %H:%M:%S"),
        reason,
        format_duration(total_duration_secs),
        total_duration_secs,
        format_duration(active_duration_secs),
        active_duration_secs,
        format_duration(idle_duration_secs),
        idle_duration_secs,
        final_energy_kwh,
        phase_str,
        session.max_current,
        session.max_power,
        avg_active_power_w,
        ticket_str
    );

    if let Err(e) = fs::write(&session.txt_path, summary_content) {
        logger::error("SESSION", &format!("Failed to write session summary TXT {:?}: {}", session.txt_path, e));
    } else {
        logger::info(
            "SESSION",
            &format!(
                "Saved charging session {} ({:.3} kWh, {})",
                session.session_id,
                final_energy_kwh,
                format_duration(total_duration_secs)
            ),
        );
        *LAST_CLOSED_SESSION.lock().unwrap_or_else(|e| e.into_inner()) = Some((session.txt_path.clone(), Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn setup_test_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("greenup_test_sessions_{}_{}", std::process::id(), id));
        let _ = fs::create_dir_all(&path);
        set_custom_sessions_dir(Some(path.clone()));
        path
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3600), "1h 00m 00s");
        assert_eq!(format_duration(7818), "2h 10m 18s");
    }

    #[test]
    fn test_adaptive_sampling_logic() {
        let now = Instant::now();
        let session = ActiveSession {
            session_id: "test".into(),
            start_time: Local::now(),
            start_instant: now,
            last_sample_instant: Some(now - Duration::from_millis(950)),
            last_sample_current: 16.0,
            last_sample_state: "D".into(),
            max_current: 16.0,
            max_power: 3680.0,
            cumulative_energy_wh: 100.0,
            active_charging_duration_secs_f64: 60.0,
            last_tick_instant: now,
            phase: "Mono".into(),
            legrand_ticket: None,
            csv_path: PathBuf::new(),
            txt_path: PathBuf::new(),
            csv_writer: None,
            sample_count: 1,
            pending_state_a_since: None,
        };

        // 1. State change -> immediate sample
        assert!(session.should_sample("B", 16.0, now));

        // 2. Current variation > 0.5A with >= 900ms elapsed -> sample
        assert!(session.should_sample("D", 16.6, now));

        // 3. Current variation <= 0.5A and elapsed < 60s -> DO NOT sample
        assert!(!session.should_sample("D", 16.3, now));

        // 4. Stable current after 60s -> sample
        let now_60s = now + Duration::from_secs(61);
        assert!(session.should_sample("D", 16.1, now_60s));
    }

    #[test]
    fn test_discard_negligible_session() {
        let dir = setup_test_dir();
        let tel = Telemetry::default();

        start_session("B", &tel);
        assert!(is_active());

        end_session("Stopped without charge", &tel);
        assert!(!is_active());

        // Both CSV and TXT must NOT exist
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 0, "Negligible session should leave no files on disk");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_valid_session_creates_csv_and_txt() {
        let dir = setup_test_dir();
        let mut tel = Telemetry::default();
        tel.greenup_state = "B".into();
        tel.voltage = 230.0;
        tel.limit_amps = 16;

        start_session("B", &tel);
        assert!(is_active());

        // State changes to D (charging)
        tel.greenup_state = "D".into();
        tel.current = 16.0;
        tel.power = 3680.0;
        on_state_change("D", &tel);

        // Advance charging time and energy
        if let Some(ref mut session) = *ACTIVE_SESSION.lock().unwrap() {
            session.active_charging_duration_secs_f64 = 120.0;
            session.cumulative_energy_wh = 125.0; // 0.125 kWh > 0.02 kWh
        }

        record_ticket("WT:10:1138:50:CT:2:6:37:EVplug:5546.38:0.00:");
        update_phase("Mono");

        end_session("Session stopped (State: A > 60s)", &tel);
        assert!(!is_active());

        let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 2, "Expected 1 CSV and 1 TXT file");

        let csv_entry = entries.iter().find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("csv")).unwrap();
        let txt_entry = entries.iter().find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("txt")).unwrap();

        let csv_content = fs::read_to_string(csv_entry.path()).unwrap();
        assert!(csv_content.starts_with("timestamp,elapsed_s,state,current_a,limit_a,voltage_v,power_w,energy_kwh"));
        assert!(csv_content.contains(",B,") || csv_content.contains(",D,"));

        let txt_content = fs::read_to_string(txt_entry.path()).unwrap();
        assert!(txt_content.contains("GREENUP-LINK CHARGING SESSION SUMMARY"));
        assert!(txt_content.contains("0.125 kWh"));
        assert!(txt_content.contains("Mono (1-Phase)"));
        assert!(txt_content.contains("WT:10:1138:50:CT:2:6:37:EVplug:5546.38:0.00:"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_immediate_termination_states() {
        for state in &["L", "R", "ST", "S", "T", "U", "V", "W", "X", "Y", "Z"] {
            assert!(is_immediate_termination_state(state), "State {} should be termination state", state);
            assert!(!is_connected_state(state), "State {} should NOT be connected state", state);
        }
        for state in &["B", "C", "D", "E", "I", "J"] {
            assert!(!is_immediate_termination_state(state), "State {} should NOT be termination state", state);
            assert!(is_connected_state(state), "State {} should be connected state", state);
        }
        assert!(!is_connected_state("A"));
        assert!(!is_connected_state("DISCONNECTED"));
    }

    #[test]
    fn test_state_a_grace_period_and_cancellation() {
        let dir = setup_test_dir();
        let mut tel = Telemetry::default();
        tel.greenup_state = "B".into();

        start_session("B", &tel);
        assert!(is_active());

        // Vehicle enters State A (temporary interruption)
        tel.greenup_state = "A".into();
        on_state_change("A", &tel);
        assert!(is_active(), "Session must remain active during State A grace period");

        if let Some(ref session) = *ACTIVE_SESSION.lock().unwrap() {
            assert!(session.pending_state_a_since.is_some(), "State A grace timer should be running");
        }

        // Within grace period (< 60s), state returns to B
        tel.greenup_state = "B".into();
        on_state_change("B", &tel);
        assert!(is_active(), "Session must remain active after resuming");

        if let Some(ref session) = *ACTIVE_SESSION.lock().unwrap() {
            assert!(session.pending_state_a_since.is_none(), "State A grace timer should be canceled");
        }

        end_session("Test cleanup", &tel);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_state_a_timeout_after_60s() {
        let dir = setup_test_dir();
        let mut tel = Telemetry::default();
        tel.greenup_state = "B".into();
        tel.voltage = 230.0;

        start_session("B", &tel);
        assert!(is_active());

        // Simulate some charge
        if let Some(ref mut session) = *ACTIVE_SESSION.lock().unwrap() {
            session.active_charging_duration_secs_f64 = 120.0;
            session.cumulative_energy_wh = 500.0; // 0.5 kWh
        }

        // Enter State A
        tel.greenup_state = "A".into();
        on_state_change("A", &tel);
        assert!(is_active());

        // Artificially advance State A timer past 60s
        if let Some(ref mut session) = *ACTIVE_SESSION.lock().unwrap() {
            session.pending_state_a_since = Some(Instant::now() - Duration::from_secs(65));
        }

        // Checking timeout should now terminate session
        assert!(check_state_a_timeout(&tel));
        assert!(!is_active(), "Session should be terminated after 60s in State A");

        let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 2, "Expected 1 CSV and 1 TXT file");

        let txt_entry = entries.iter().find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("txt")).unwrap();
        let txt_content = fs::read_to_string(txt_entry.path()).unwrap();
        assert!(txt_content.contains("Session stopped (State: A > 60s)"));

        let _ = fs::remove_dir_all(&dir);
    }
}
