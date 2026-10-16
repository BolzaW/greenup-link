mod api;
mod auth;
mod config;

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use greenup_driver::state::{AppState, BoardInfo, Telemetry};

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

    let (tx, rx) = mpsc::channel::<greenup_protocol::Command>(32);

    let mut initial_info = BoardInfo::default();
    initial_info.link_version = pkg_version.to_string();

    let app_state = Arc::new(AppState {
        telemetry: Mutex::new(Telemetry::default()),
        info: Mutex::new(initial_info),
        serial_tx: tx,
        tic_detection: Mutex::new(greenup_driver::state::TicDetectionState::default()),
        tokio_handle: tokio::runtime::Handle::current(),
    });

    // 1. Start the Serial communication daemon
    // We use spawn_blocking because reading from the serial port (port.read) is blocking
    let state_for_serial = app_state.clone();
    tokio::task::spawn_blocking(move || {
        greenup_driver::run_serial_loop(state_for_serial, rx);
    });

    // Send the initialization sequence on startup
    let state_for_init = app_state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        greenup_driver::logger::log("SYS", "🔄 Sending Legrand initialization sequence...");
        greenup_driver::serial::trigger_init_sequence(&state_for_init).await;
    });

    // 2. Start the Axum HTTP server
    let router = api::build_router(app_state, app_config.http.api_token);
    
    let addr = &app_config.http.bind;
    greenup_driver::logger::log("SYS", &format!("🌍 Web server started: http://{}", addr));
    
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    axum::serve(listener, router).await.expect("Fatal web server error");
}


