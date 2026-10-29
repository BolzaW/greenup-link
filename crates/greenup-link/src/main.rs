mod api;
mod auth;
mod config;

use std::sync::{atomic::AtomicU8, Arc, Mutex};
use tokio::sync::mpsc;
use greenup_driver::state::{AppState, BoardInfo, StationManager, Telemetry};

#[tokio::main]
async fn main() {
    greenup_driver::logger::init();

    let pkg_version = env!("CARGO_PKG_VERSION");
    
    greenup_driver::logger::log("SYS", "===================================================");
    greenup_driver::logger::log("SYS", &format!("               GREEN'UP LINK v{:<15}     ", pkg_version));
    greenup_driver::logger::log("SYS", "===================================================");

    let app_config = config::AppConfig::load();
    if app_config.http.api_token.is_some() {
        greenup_driver::logger::log("SYS", "🔒 API authentication: ENABLED (Bearer token / Basic Auth)");
    } else {
        greenup_driver::logger::log("SYS", "🔓 API authentication: DISABLED (open LAN mode — set api_token in greenup.toml to secure)");
    }

    // 1. Point 1 Setup
    let (tx1, rx1) = mpsc::channel::<greenup_protocol::Command>(32);

    let mut initial_info_1 = BoardInfo::default();
    initial_info_1.link_version = pkg_version.to_string();
    initial_info_1.side = Some(1);

    let app_state_1 = Arc::new(AppState {
        side: AtomicU8::new(1),
        telemetry: Mutex::new(Telemetry::default()),
        info: Mutex::new(initial_info_1),
        serial_tx: tx1,
        tic_detection: Mutex::new(greenup_driver::state::TicDetectionState::default()),
        tokio_handle: tokio::runtime::Handle::current(),
    });

    let state_for_serial_1 = app_state_1.clone();
    let serial_options_1 = greenup_driver::SerialOptions {
        port: app_config.serial.port.clone(),
        baud_rate: app_config.serial.baud_rate,
        timeout_ms: 100,
        watchdog_timeout_secs: app_config.serial.watchdog_timeout_secs,
    };
    tokio::task::spawn_blocking(move || {
        greenup_driver::run_serial_loop_with_options(state_for_serial_1, rx1, serial_options_1);
    });

    let state_for_init_1 = app_state_1.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        greenup_driver::logger::log("SYS", "🔄 Sending Point 1 initialization sequence...");
        greenup_driver::serial::trigger_init_sequence(&state_for_init_1).await;
    });

    // 2. Point 2 Setup (Dual charge point mode)
    let app_state_2 = if let Some(ref port2) = app_config.serial.port2 {
        greenup_driver::logger::log(
            "SYS",
            &format!("⚡ Dual charge point mode: ACTIVATED (Port 1: {}, Port 2: {})", app_config.serial.port, port2),
        );
        let (tx2, rx2) = mpsc::channel::<greenup_protocol::Command>(32);
        let mut initial_info_2 = BoardInfo::default();
        initial_info_2.link_version = pkg_version.to_string();
        initial_info_2.side = Some(2);

        let mut tel2 = Telemetry::default();
        tel2.side = 2;

        let state_2 = Arc::new(AppState {
            side: AtomicU8::new(2),
            telemetry: Mutex::new(tel2),
            info: Mutex::new(initial_info_2),
            serial_tx: tx2,
            tic_detection: Mutex::new(greenup_driver::state::TicDetectionState::default()),
            tokio_handle: tokio::runtime::Handle::current(),
        });

        let state_for_serial_2 = state_2.clone();
        let serial_options_2 = greenup_driver::SerialOptions {
            port: port2.clone(),
            baud_rate: app_config.serial.baud_rate,
            timeout_ms: 100,
            watchdog_timeout_secs: app_config.serial.watchdog_timeout_secs,
        };
        tokio::task::spawn_blocking(move || {
            greenup_driver::run_serial_loop_with_options(state_for_serial_2, rx2, serial_options_2);
        });

        let state_for_init_2 = state_2.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            greenup_driver::logger::log("SYS", "🔄 Sending Point 2 initialization sequence...");
            greenup_driver::serial::trigger_init_sequence(&state_for_init_2).await;
        });

        Some(state_2)
    } else {
        greenup_driver::logger::log("SYS", "⚡ Single charge point mode active");
        None
    };

    let station = Arc::new(StationManager::new(app_state_1, app_state_2));

    // 3. Start the Axum HTTP server
    let router = api::build_router(station, app_config.http.api_token);
    
    let addr = &app_config.http.bind;
    greenup_driver::logger::log("SYS", &format!("🌍 Web server started: http://{}", addr));
    
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    axum::serve(listener, router).await.expect("Fatal web server error");
}
