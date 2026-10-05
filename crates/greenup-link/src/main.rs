mod api;
pub 



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

    let (tx, rx) = mpsc::channel::<String>(32);

    let mut initial_info = BoardInfo::default();
    initial_info.link_version = pkg_version.to_string();

    let app_state = Arc::new(AppState {
        telemetry: Mutex::new(Telemetry::default()),
        info: Mutex::new(initial_info),
        serial_tx: tx,
        tic_detection: Mutex::new(greenup_driver::state::TicDetectionState::default()),
    });

    // 1. Démarrer le daemon de communication Série
    // On utilise spawn_blocking car la lecture sur le port série (port.read) est bloquante
    let state_for_serial = app_state.clone();
    tokio::task::spawn_blocking(move || {
        greenup_driver::run_serial_loop(state_for_serial, rx);
    });

    // 2. Démarrer le serveur HTTP Axum
    let router = api::build_router(app_state);
    
    // On écoute sur toutes les interfaces réseau (0.0.0.0) sur le port 8080
    let addr = "0.0.0.0:8080";
    greenup_driver::logger::log("SYS", &format!("🌍 Serveur Web démarré : http://{}", addr));
    greenup_driver::logger::log("SYS", "   - GET  /api/info, /api/telemetry");
    greenup_driver::logger::log("SYS", "   - POST /api/charge/start, /api/charge/stop, /api/current/:amps");
    greenup_driver::logger::log("SYS", "   - POST /api/tic/refresh, /api/bluetooth");
    
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Impossible de lier le port 8080");
    axum::serve(listener, router).await.expect("Erreur fatale du serveur web");
}


