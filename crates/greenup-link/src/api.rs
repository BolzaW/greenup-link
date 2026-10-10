use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use greenup_driver::logger;
use greenup_driver::SharedState;
use greenup_driver::serial::TicStartError;
use greenup_protocol::{Command, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS};

pub fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(serve_ui))
        .route("/locales/fr.json", get(serve_fr_json))
        .route("/locales/en.json", get(serve_en_json))
        .route("/api/info", get(get_info))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/current/:amps", post(set_current))
        .route("/api/charge/start", post(start_charge))
        .route("/api/charge/stop", post(stop_charge))
        .route("/api/init", post(init_sequence))
        .route("/api/reset", post(reset_board))
        .route("/api/tic/refresh", post(refresh_tic))
        .route("/api/bluetooth", post(set_bluetooth))
        .route("/api/command", post(send_raw_command))
        .with_state(state)
}

#[derive(serde::Deserialize)]
pub struct BluetoothPayload {
    pub enabled: bool,
}

/// GET /
async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

/// GET /locales/fr.json
async fn serve_fr_json() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/json")], include_str!("locales/fr.json"))
}

/// GET /locales/en.json
async fn serve_en_json() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/json")], include_str!("locales/en.json"))
}

/// GET /api/info
async fn get_info(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "🔍 GET request /api/info");
    let info = state.info.lock().unwrap().clone();
    Json(info)
}

/// GET /api/telemetry
async fn get_telemetry(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("TRACE", "📡 GET request /api/telemetry");
    let telemetry = state.telemetry.lock().unwrap().clone();
    Json(telemetry)
}

/// POST /api/current/:amps
async fn set_current(State(state): State<SharedState>, Path(amps): Path<u32>) -> impl IntoResponse {
    let cmd = match u8::try_from(amps).ok().map(Command::set_current_limit) {
        Some(Ok(cmd)) => cmd,
        _ => {
            logger::log("API", &format!("⛔ Rejecting out of bounds current: {}A", amps));
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!(
                    "Current must be between {} and {} Amps",
                    MIN_CURRENT_AMPS, MAX_CURRENT_AMPS
                )})),
            );
        }
    };

    logger::log("API", &format!("⚡ Current limit modification → {}A", amps));
    
    if state.serial_tx.send(cmd).await.is_ok() {
        (
            StatusCode::OK,
            Json(json!({
                "status": "success", 
                "message": format!("Current limit set to {}A", amps)
            })),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal error: Failed to communicate with serial thread"})),
        )
    }
}

/// POST /api/charge/start
async fn start_charge(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "▶ Charge resume request (DOK)");
    if state.serial_tx.send(Command::SetDerogation(true)).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Charge authorized (DOK)"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error"})))
    }
}

/// POST /api/charge/stop

/// POST /api/t2/enable

/// POST /api/init
async fn init_sequence(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "🔄 Starting initialization sequence");
    tokio::spawn(async move {
        // TIC detection is never started automatically:
        // it must be explicitly requested via POST /api/tic/refresh.
        greenup_driver::serial::trigger_init_sequence(&state).await;
    });
    (StatusCode::OK, Json(json!({"status": "success", "message": "Initialization sequence started"})))
}



/// POST /api/reset
async fn reset_board(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "⚠️ Hardware restart request (Reset)");
    if state.serial_tx.send(Command::Reset).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "ATmega board restart requested"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error"})))
    }
}

/// POST /api/charge/stop
async fn stop_charge(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "⏹ Charge pause request (FM2:1 + DNOK)");
    
    // First we send FM2:1 to explicitly block the charge
    if let Err(_) = state.serial_tx.send(Command::SetExternalSignal(true)).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error (FM2:1)"})));
    }
    
    // Then we send DNOK to release the derogation
    if state.serial_tx.send(Command::SetDerogation(false)).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Charge stopped (FM2:1 + DNOK)"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error (DNOK)"})))
    }
}

/// POST /api/command
async fn send_raw_command(State(state): State<SharedState>, body: String) -> impl IntoResponse {
    let cmd = body.trim().to_string();
    logger::log("API", &format!("🔧 Raw command: {}", body.trim()));
    if state.serial_tx.send(Command::Raw(cmd)).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Command sent"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error"})))
    }
}

/// POST /api/tic/refresh
async fn refresh_tic(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "🔄 Manual TIC detection request (TICTM:1)");
    match greenup_driver::serial::start_tic_detection(&state).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "success", "message": "TIC detection started"}))),
        Err(TicStartError::NotInStateA(current)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("TIC detection can only be done when the charging station is free (State:A). Current state: {}", current)})),
        ),
        Err(TicStartError::Serial) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Serial communication error"})),
        ),
    }
}

/// POST /api/bluetooth
async fn set_bluetooth(State(state): State<SharedState>, Json(payload): Json<BluetoothPayload>) -> impl IntoResponse {
    let cmd = Command::SetBluetooth(payload.enabled);
    let log_msg = if payload.enabled { "🔵 Activating" } else { "⚪ Deactivating" };
    
    logger::log("API", &format!("{} Bluetooth requested", log_msg));
    
    if state.serial_tx.send(cmd).await.is_ok() {
        (
            StatusCode::OK,
            Json(json!({"status": "success", "message": format!("Bluetooth successfully {}", if payload.enabled { "activated" } else { "deactivated" })}))
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Serial communication error"}))
        )
    }
}
