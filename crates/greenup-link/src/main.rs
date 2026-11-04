mod api;
mod auth;
mod config;
pub mod hardware_specs;
pub mod session;
pub mod station;
pub mod telemetry;

use std::sync::Arc;
use station::{AppState, StationManager};

fn spawn_supervised_point(
    side: u8,
    mode: telemetry::SupervisionMode,
    port_name: String,
    baud_rate: u32,
    watchdog_timeout_secs: u64,
    link_version: &str,
) -> Arc<AppState> {
    let driver = greenup_driver::spawn_port(
        side,
        mode,
        greenup_driver::SerialOptions {
            port: port_name,
            baud_rate,
            timeout_ms: 100,
            watchdog_timeout_secs,
        },
    );
    let app_state = Arc::new(AppState::from_driver(driver, link_version));
    app_state.start_driver_watcher();
    app_state
}

#[tokio::main]
async fn main() {
    let app_config = config::AppConfig::load();

    // Configure storage directories from greenup.toml before logger initialization
    greenup_driver::logger::set_custom_log_dir(Some(app_config.storage.log_dir.clone()));
    session::set_custom_sessions_dir(Some(std::path::PathBuf::from(
        &app_config.storage.sessions_dir,
    )));
    greenup_driver::logger::init();

    let pkg_version = env!("CARGO_PKG_VERSION");

    greenup_driver::logger::log("SYS", "===================================================");
    greenup_driver::logger::log(
        "SYS",
        &format!("               GREEN'UP LINK v{:<15}     ", pkg_version),
    );
    greenup_driver::logger::log("SYS", "===================================================");
    greenup_driver::logger::log(
        "SYS",
        &format!(
            "📁 Storage paths: logs = {}, sessions = {}",
            app_config.storage.log_dir, app_config.storage.sessions_dir
        ),
    );
    if app_config.http.api_token.is_some() {
        let auth_desc = if app_config.http.auth_ihm {
            "STRICT (Web GUI & REST APIs protected)"
        } else {
            "KIOSK (Web GUI open for consultation, commands & settings protected)"
        };
        greenup_driver::logger::log("SYS", &format!("🔒 API authentication: ENABLED [{}]", auth_desc));
    } else {
        greenup_driver::logger::warn(
            "SYS",
            "🔓 API authentication: DISABLED (open LAN mode — set api_token in greenup.toml to secure)",
        );
    }

    match app_config.station.mode {
        telemetry::SupervisionMode::Local => {
            greenup_driver::logger::warn(
                "SYS",
                "👂 Supervision mode: LOCAL (passive monitoring - no commands will be sent to ATmega)",
            );
        }
        telemetry::SupervisionMode::Remote => {
            greenup_driver::logger::log(
                "SYS",
                "⚡ Supervision mode: REMOTE (active supervision enabled)",
            );
        }
    }

    let (port1_name, port2_candidate) = greenup_driver::discover_candidate_ports(
        app_config.serial.port.as_deref(),
        app_config.serial.port2.as_deref(),
    );

    greenup_driver::logger::log(
        "SYS",
        &format!(
            "🔌 Serial port resolved: Point 1 = {} (Point 2 candidate = {})",
            port1_name, port2_candidate
        ),
    );

    let app_state_1 = spawn_supervised_point(
        1,
        app_config.station.mode,
        port1_name,
        app_config.serial.baud_rate,
        app_config.serial.watchdog_timeout_secs,
        pkg_version,
    );

    let station = match app_config.serial.dual_point {
        config::DualPointMode::True => {
            greenup_driver::logger::log(
                "SYS",
                &format!(
                    "⚡ Dual charge point mode: FORCED (Point 1: active, Point 2: {})",
                    port2_candidate
                ),
            );
            let app_state_2 = spawn_supervised_point(
                2,
                app_config.station.mode,
                port2_candidate,
                app_config.serial.baud_rate,
                app_config.serial.watchdog_timeout_secs,
                pkg_version,
            );
            greenup_driver::logger::set_dual_station(true);
            Arc::new(StationManager::new(app_state_1, Some(app_state_2)))
        }
        config::DualPointMode::False => {
            greenup_driver::logger::log("SYS", "⚡ Single charge point mode: FORCED");
            greenup_driver::logger::set_dual_station(false);
            Arc::new(StationManager::new(app_state_1, None))
        }
        config::DualPointMode::Auto => {
            greenup_driver::logger::log(
                "SYS",
                "⚡ Dual charge point mode: AUTO (detecting from board reference via driver watch)",
            );
            greenup_driver::logger::set_dual_station(false);
            let st = Arc::new(StationManager::new(app_state_1, None));
            st.watch_auto_dual_port(
                app_config.serial.port.clone(),
                app_config.serial.port2.clone(),
                app_config.serial.baud_rate,
                app_config.serial.watchdog_timeout_secs,
                pkg_version.to_string(),
            );
            st
        }
    };

    // Start the Axum HTTP server
    let router = api::build_router(station, app_config.http.clone());

    let addr = &app_config.http.bind;
    greenup_driver::logger::log("SYS", &format!("🌍 Web server started: http://{}", addr));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    axum::serve(listener, router)
        .await
        .expect("Fatal web server error");
}
