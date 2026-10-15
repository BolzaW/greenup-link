use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
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
        .route("/css/style.css", get(serve_css))
        .route("/js/dom.js", get(serve_js_dom))
        .route("/js/i18n.js", get(serve_js_i18n))
        .route("/js/linky.js", get(serve_js_linky))
        .route("/js/controls.js", get(serve_js_controls))
        .route("/js/visuals.js", get(serve_js_visuals))
        .route("/js/api.js", get(serve_js_api))
        .route("/js/console.js", get(serve_js_console))
        .route("/js/main.js", get(serve_js_main))
        .route("/api/info", get(get_info))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/logs", get(get_logs))
        .route("/api/logs/stream", get(stream_logs))
        .route("/api/logs/export", get(export_logs))
        .route("/api/logs/level", get(get_log_level_handler).post(set_log_level_handler))
        .route("/api/current/:amps", post(set_current))
        .route("/api/charge", post(set_charge_state))
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

/// GET /css/style.css
async fn serve_css() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("web/css/style.css"))
}

/// GET /js/dom.js
async fn serve_js_dom() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/dom.js"))
}

/// GET /js/i18n.js
async fn serve_js_i18n() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/i18n.js"))
}

/// GET /js/linky.js
async fn serve_js_linky() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/linky.js"))
}

/// GET /js/controls.js
async fn serve_js_controls() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/controls.js"))
}

/// GET /js/visuals.js
async fn serve_js_visuals() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/visuals.js"))
}

/// GET /js/api.js
async fn serve_js_api() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/api.js"))
}

/// GET /js/main.js
async fn serve_js_main() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/main.js"))
}

/// GET /js/console.js
async fn serve_js_console() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/console.js"))
}

/// GET /api/logs
async fn get_logs() -> impl IntoResponse {
    Json(logger::get_recent_logs())
}

/// GET /api/logs/stream (SSE)
async fn stream_logs() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = logger::subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(entry) => {
            if let Ok(data) = serde_json::to_string(&entry) {
                Some(Ok(Event::default().data(data)))
            } else {
                None
            }
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// GET /api/logs/export
async fn export_logs() -> impl IntoResponse {
    let (filename, content) = logger::get_current_log_content();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    if let Ok(val) = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)) {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, val);
    }
    (headers, content)
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogLevelPayload {
    pub level: String,
}

/// GET /api/logs/level
async fn get_log_level_handler() -> impl IntoResponse {
    let current = logger::get_log_level();
    Json(json!({ "level": current.as_str() }))
}

/// POST /api/logs/level
async fn set_log_level_handler(Json(payload): Json<LogLevelPayload>) -> impl IntoResponse {
    if let Some(level) = logger::LogLevel::from_str_loose(&payload.level) {
        logger::set_log_level(level);
        (
            StatusCode::OK,
            Json(json!({ "status": "success", "level": level.as_str() })),
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid log level: '{}'. Expected ERROR, WARN, INFO, DEBUG, or TRACE", payload.level) })),
        )
    }
}

/// GET /api/info
async fn get_info(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "🔍 GET request /api/info");
    let info = state.info.lock().unwrap().clone();
    Json(info)
}

/// GET /api/telemetry
async fn get_telemetry(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("DEBUG", "📡 GET request /api/telemetry");
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



/// POST /api/charge/disable

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



#[derive(serde::Deserialize)]
pub struct ChargeActionPayload { action: String }

/// POST /api/charge
async fn set_charge_state(State(state): State<SharedState>, Json(payload): Json<ChargeActionPayload>) -> impl IntoResponse {
    match payload.action.as_str() {
        "enable" => {
            logger::log("API", "⚡ Charge enable requested (T2COK + FM2:0 + DOK)");
            let _ = state.serial_tx.send(Command::AuthorizeType2(true)).await;
            let _ = state.serial_tx.send(Command::SetExternalSignal(false)).await;
            if state.serial_tx.send(Command::SetDerogation(true)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "disable" => {
            logger::log("API", "🛑 Charge disable requested (FM2:1 + DNOK)");
            let _ = state.serial_tx.send(Command::SetExternalSignal(true)).await;
            if state.serial_tx.send(Command::SetDerogation(false)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "pause" => {
            logger::log("API", "⏸ Charge pause requested (SBNOK)");
            if state.serial_tx.send(Command::SetStartButton(false)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "resume" => {
            logger::log("API", "▶ Charge resume requested (SBOK)");
            if state.serial_tx.send(Command::SetStartButton(true)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        _ => (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid action"})))
    }
}



