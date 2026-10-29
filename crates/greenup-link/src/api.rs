use axum::{
    extract::{DefaultBodyLimit, Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use serde_json::json;
use axum::extract::Query;
use greenup_driver::logger;
use greenup_driver::{SharedState, SharedStation};
use greenup_driver::serial::TicStartError;
use greenup_protocol::{Command, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS};

use std::sync::Arc;
use axum::http::header::{HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS};
use crate::auth::{self, SharedApiToken};

/// Axum middleware appending standard defensive HTTP security headers to all responses.
async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}

/// Axum middleware logging API requests at DEBUG level (or TRACE for high-frequency polling).
async fn api_debug_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // High frequency polling endpoints are logged at TRACE to avoid cluttering DEBUG logs
    if path == "/api/telemetry" || path == "/api/logs/stream" {
        logger::trace("API", &format!("{} {}", method, path));
    } else {
        logger::debug("API", &format!("→ {} {}", method, path));
    }

    let response = next.run(req).await;

    if path != "/api/telemetry" && path != "/api/logs/stream" {
        logger::debug("API", &format!("← {} {} [{}]", method, path, response.status()));
    }

    response
}

#[derive(serde::Deserialize, Debug, Clone, Default)]
pub struct SideQuery {
    pub side: Option<u8>,
}

pub fn build_router(station: SharedStation, api_token: Option<String>) -> Router {
    let token_state: SharedApiToken = api_token.map(Arc::new);

    let protected_api = Router::new()
        .route("/api/info", get(get_info))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/sessions", get(list_sessions_handler))
        .route("/api/sessions/:id/export", post(export_session_handler))
        .route("/api/sessions/:id", axum::routing::delete(delete_session_handler))
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
        .with_state(station)
        .route_layer(axum::middleware::from_fn_with_state(
            token_state.clone(),
            auth::require_auth,
        ));

    let auth_router = Router::new()
        .route("/api/auth/status", get(auth::auth_status))
        .with_state(token_state);

    let api_router = Router::new()
        .merge(auth_router)
        .merge(protected_api)
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_SIZE))
        .layer(axum::middleware::from_fn(api_debug_logger));

    Router::new()
        .route("/", get(serve_ui))
        .route("/favicon.ico", get(serve_favicon))
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
        .route("/js/sessions.js", get(serve_js_sessions))
        .route("/js/config.js", get(serve_js_config))
        .route("/js/main.js", get(serve_js_main))
        .merge(api_router)
        .layer(axum::middleware::from_fn(security_headers_middleware))
}

#[derive(serde::Deserialize)]
pub struct BluetoothPayload {
    pub enabled: bool,
}

/// GET /
async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

/// GET /favicon.ico
async fn serve_favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("web/favicon.ico"),
    )
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

/// GET /js/sessions.js
async fn serve_js_sessions() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/sessions.js"))
}

/// GET /js/config.js
async fn serve_js_config() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")], include_str!("web/js/config.js"))
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
    let logs = logger::get_recent_logs();
    logger::debug("API", &format!("📋 Fetching recent logs buffer ({} entries)", logs.len()));
    Json(logs)
}

/// GET /api/logs/stream (SSE)
async fn stream_logs() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    logger::debug("API", "📡 SSE log stream client connected");
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
    logger::debug("API", &format!("💾 Exporting log archive: {} ({} bytes)", filename, content.len()));
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
    logger::debug("API", &format!("🎚️ Log level requested: {}", current.as_str()));
    Json(json!({ "level": current.as_str() }))
}

/// POST /api/logs/level
async fn set_log_level_handler(Json(payload): Json<LogLevelPayload>) -> impl IntoResponse {
    if let Some(level) = logger::LogLevel::from_str_loose(&payload.level) {
        logger::set_log_level(level);
        logger::log("API", &format!("⚙️ Log level set to: {}", level.as_str()));
        (
            StatusCode::OK,
            Json(json!({ "status": "success", "level": level.as_str() })),
        )
    } else {
        logger::log("API", &format!("⚠️ Invalid log level requested: '{}'", payload.level));
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid log level: '{}'. Expected ERROR, WARN, INFO, DEBUG, or TRACE", payload.level) })),
        )
    }
}

/// GET /api/info
async fn get_info(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    logger::debug("API", &format!("🔍 GET request /api/info (side: {})", state.side()));
    let mut info = state.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
    info.is_dual = station.is_dual();
    info.available_sides = station.available_points();
    info.side = Some(state.side());
    Json(info)
}

/// GET /api/telemetry
async fn get_telemetry(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    logger::log("TRACE", &format!("📡 GET request /api/telemetry (side: {})", state.side()));
    let mut telemetry = state.telemetry.lock().unwrap_or_else(|e| e.into_inner()).clone();
    telemetry.side = state.side();
    Json(telemetry)
}

fn check_serial_connected(state: &SharedState) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let tel = state.telemetry.lock().unwrap_or_else(|e| e.into_inner());
    if tel.serial.status != greenup_driver::telemetry::SerialStatus::UsbConnected {
        let err_msg = match tel.serial.status {
            greenup_driver::telemetry::SerialStatus::UsbDisconnected => {
                "USB cable disconnected (no USB adapter detected)"
            }
            greenup_driver::telemetry::SerialStatus::UsbConnecting => {
                "USB serial link initializing (waiting for station board)"
            }
            greenup_driver::telemetry::SerialStatus::UsbNoResponse => {
                "No response from station board (station powered off or ATmega frozen)"
            }
            _ => "USB serial link unavailable",
        };
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "error",
                "error": format!("Command unavailable: {}", err_msg),
                "serial_status": tel.serial.status,
            })),
        ));
    }
    Ok(())
}

/// POST /api/current/:amps
async fn set_current(
    State(station): State<SharedStation>,
    Path(amps): Path<u32>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
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
async fn init_sequence(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side).clone();
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    logger::log("API", &format!("🔄 Starting initialization sequence (Side {})", state.side()));
    tokio::spawn(async move {
        // TIC detection is never started automatically:
        // it must be explicitly requested via POST /api/tic/refresh.
        greenup_driver::serial::trigger_init_sequence(&state).await;
    });
    (StatusCode::OK, Json(json!({"status": "success", "message": "Initialization sequence started"})))
}



/// POST /api/reset
async fn reset_board(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    logger::log("API", &format!("⚠️ Hardware restart request (Reset) on Side {}", state.side()));
    if state.serial_tx.send(Command::Reset).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "ATmega board restart requested"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error"})))
    }
}







/// Maximum allowed HTTP request body size (4 KB).
const MAX_REQUEST_BODY_SIZE: usize = 4096;

const MAX_RAW_COMMAND_LEN: usize = 20;

fn is_valid_raw_command(cmd: &str) -> bool {
    !cmd.is_empty()
        && cmd.len() <= MAX_RAW_COMMAND_LEN
        && cmd.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '?')
}

/// POST /api/command
async fn send_raw_command(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
    body: String,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    let cmd = body.trim();
    if !is_valid_raw_command(cmd) {
        logger::warn("API", &format!("⛔ Rejected invalid raw command: '{}'", cmd));
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "error": "Invalid command format. Allowed characters: [a-zA-Z0-9:?], length 1-20 characters."
            })),
        );
    }

    if cmd.eq_ignore_ascii_case("service") {
        logger::warn("API", "⚠️ Warning: ATmega interactive service menu triggered via raw command");
    } else {
        logger::info("API", &format!("🔧 Raw command: {}", cmd));
    }

    if state.serial_tx.send(Command::Raw(cmd.to_string())).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Command sent"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial communication error"})))
    }
}

/// POST /api/tic/refresh
async fn refresh_tic(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    logger::log("API", &format!("🔄 Manual TIC detection request (TICTM:1) on Side {}", state.side()));
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
async fn set_bluetooth(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
    Json(payload): Json<BluetoothPayload>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
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
async fn set_charge_state(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
    Json(payload): Json<ChargeActionPayload>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
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
        _ => {
            logger::log("API", &format!("⚠️ Invalid charge action received: '{}'", payload.action));
            (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid action"})))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_raw_command() {
        // Valid commands
        assert!(is_valid_raw_command("service"));
        assert!(is_valid_raw_command("0"));
        assert!(is_valid_raw_command("1"));
        assert!(is_valid_raw_command("WeekYearProduction?")); // 19 chars
        assert!(is_valid_raw_command("FM2:0"));
        assert!(is_valid_raw_command("TICTM:1"));
        assert!(is_valid_raw_command("12345678901234567890")); // exactly 20 chars

        // Invalid commands
        assert!(!is_valid_raw_command("")); // empty
        assert!(!is_valid_raw_command("123456789012345678901")); // 21 chars (> 20)
        assert!(!is_valid_raw_command("FM2: 0")); // contains space
        assert!(!is_valid_raw_command("cmd\n")); // newline injection
        assert!(!is_valid_raw_command("cmd\r\n")); // CRLF injection
        assert!(!is_valid_raw_command("test;rm")); // semicolon
        assert!(!is_valid_raw_command("cmd|pipe")); // pipe
        assert!(!is_valid_raw_command("$VAR")); // dollar sign
        assert!(!is_valid_raw_command("cmd=val")); // equals sign
    }
}


// ==============================================================================
// SESSION HISTORY API HANDLERS
// ==============================================================================

/// GET /api/sessions
/// Returns the list of all saved sessions and the live summary of the currently active session (if any).
async fn list_sessions_handler(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let target_side = query.side.unwrap_or(1);
    let state = station.get_point(Some(target_side));
    let active_summary = if let Ok(tel) = state.telemetry.lock() {
        greenup_driver::session::get_active_session_summary_for_side(target_side, &tel)
    } else {
        None
    };

    let saved_sessions = greenup_driver::session::list_saved_sessions_filtered(query.side);
    logger::debug(
        "API",
        &format!(
            "Listed {} saved sessions (active session for side {}: {})",
            saved_sessions.len(),
            target_side,
            active_summary.is_some()
        ),
    );

    Json(json!({
        "active_session": active_summary,
        "sessions": saved_sessions,
    }))
}

#[derive(serde::Deserialize)]
pub struct ExportSessionPayload {
    pub format: String,
}

/// POST /api/sessions/:id/export
/// Exports a session file in the requested format (csv, txt, zip).
async fn export_session_handler(
    Path(id): Path<String>,
    Json(payload): Json<ExportSessionPayload>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if !greenup_driver::session::is_valid_session_id(&id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid session ID format" })),
        ));
    }

    let sessions_dir = greenup_driver::session::get_sessions_dir();
    let csv_path = sessions_dir.join(format!("{}.csv", id));
    let txt_path = sessions_dir.join(format!("{}.txt", id));

    let fmt = payload.format.trim().to_lowercase();
    logger::debug("API", &format!("Export requested for session '{}' in format '{}'", id, fmt));

    match fmt.as_str() {
        "csv" => {
            if !csv_path.exists() {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("Session CSV file not found for {}", id) })),
                ));
            }
            let content = std::fs::read(&csv_path).map_err(|e| {
                logger::error("API", &format!("Failed to read CSV file {:?}: {}", csv_path, e));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to read CSV file from disk" })),
                )
            })?;

            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            if let Ok(disp) = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{}.csv\"", id)) {
                headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
            }
            Ok((headers, content).into_response())
        }
        "txt" => {
            if !txt_path.exists() {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("Session TXT file not found for {}", id) })),
                ));
            }
            let content = std::fs::read(&txt_path).map_err(|e| {
                logger::error("API", &format!("Failed to read TXT file {:?}: {}", txt_path, e));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to read TXT file from disk" })),
                )
            })?;

            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            if let Ok(disp) = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{}.txt\"", id)) {
                headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
            }
            Ok((headers, content).into_response())
        }
        "zip" => {
            if !csv_path.exists() && !txt_path.exists() {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("No session files found on disk for {}", id) })),
                ));
            }

            let mut buffer = std::io::Cursor::new(Vec::new());
            {
                let mut zip = zip::ZipWriter::new(&mut buffer);
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                if csv_path.exists() {
                    let csv_bytes = std::fs::read(&csv_path).map_err(|e| {
                        logger::error("API", &format!("Failed to read CSV for ZIP: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to read CSV for ZIP" })),
                        )
                    })?;
                    zip.start_file(format!("{}.csv", id), options).map_err(|e| {
                        logger::error("API", &format!("Failed to start ZIP CSV entry: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to create ZIP entry" })),
                        )
                    })?;
                    use std::io::Write;
                    zip.write_all(&csv_bytes).map_err(|e| {
                        logger::error("API", &format!("Failed to write CSV bytes into ZIP: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to write into ZIP archive" })),
                        )
                    })?;
                }

                if txt_path.exists() {
                    let txt_bytes = std::fs::read(&txt_path).map_err(|e| {
                        logger::error("API", &format!("Failed to read TXT for ZIP: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to read TXT for ZIP" })),
                        )
                    })?;
                    zip.start_file(format!("{}.txt", id), options).map_err(|e| {
                        logger::error("API", &format!("Failed to start ZIP TXT entry: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to create ZIP entry" })),
                        )
                    })?;
                    use std::io::Write;
                    zip.write_all(&txt_bytes).map_err(|e| {
                        logger::error("API", &format!("Failed to write TXT bytes into ZIP: {}", e));
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "Failed to write into ZIP archive" })),
                        )
                    })?;
                }

                zip.finish().map_err(|e| {
                    logger::error("API", &format!("Failed to finalize ZIP archive: {}", e));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to finalize ZIP archive" })),
                    )
                })?;
            }

            let zip_data = buffer.into_inner();
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/zip"),
            );
            if let Ok(disp) = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{}.zip\"", id)) {
                headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
            }
            Ok((headers, zip_data).into_response())
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Unsupported export format. Supported formats: csv, txt, zip" })),
        )),
    }
}

/// DELETE /api/sessions/:id
/// Deletes the session files from disk.
async fn delete_session_handler(
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if id.trim().is_empty() || id.eq_ignore_ascii_case("active") {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Cannot delete currently active charging session" })),
        ));
    }

    logger::info("API", &format!("Delete requested for session '{}'", id));
    match greenup_driver::session::delete_session(&id) {
        Ok(true) => Ok(Json(json!({
            "message": "Session deleted successfully",
            "id": id,
        }))),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Session '{}' not found", id) })),
        )),
        Err(err_msg) => {
            let status = if err_msg.contains("active") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            Err((
                status,
                Json(json!({ "error": err_msg })),
            ))
        }
    }
}
