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
use greenup_driver::serial::TicStartError;
use greenup_protocol::{Command, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS};
use crate::station::{SharedState, SharedStation};

use std::sync::Arc;
use axum::http::header::{
    HeaderValue, CACHE_CONTROL, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use crate::auth::{self, AuthState, SharedAuthState};
use crate::config::HttpConfig;

/// Axum middleware appending standard defensive HTTP security headers to all responses.
async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, must-revalidate"),
    );
    response
}

/// Axum middleware logging API requests at DEBUG level (or TRACE for high-frequency polling).
async fn api_debug_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // High frequency polling endpoints are logged at TRACE to avoid cluttering DEBUG logs
    if path == "/api/state" || path == "/api/logs/stream" {
        logger::trace("API", &format!("{} {}", method, path));
    } else {
        logger::debug("API", &format!("→ {} {}", method, path));
    }

    let response = next.run(req).await;

    if path != "/api/state" && path != "/api/logs/stream" {
        logger::debug("API", &format!("← {} {} [{}]", method, path, response.status()));
    }

    response
}

#[derive(serde::Deserialize, Debug, Clone, Default)]
pub struct SideQuery {
    pub side: Option<u8>,
}

pub fn build_router(station: SharedStation, config: HttpConfig) -> Router {
    let auth_state: SharedAuthState = Arc::new(AuthState {
        api_token: config.api_token.map(Arc::new),
        kiosk_mode: config.kiosk_mode,
    });

    let protected_api = Router::new()
        .route("/api/sessions", get(list_sessions_handler))
        .route("/api/sessions/:id/export", get(export_session_handler))
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
        .route("/api/voltage", post(set_voltage))
        .route("/api/state", get(get_state))
        .route("/api/station", get(get_station))
        .route("/api/connectors", get(get_connectors))
        .route("/api/connectors/:id", get(get_connector_by_id))
        .route("/api/connectors/:id/meter", get(get_connector_meter_by_id))
        .route("/api/connectors/:id/status", get(get_connector_status_by_id))
        .route("/api/meter", get(get_meter))
        .route("/api/status", get(get_status))
        .route("/api/driver/raw", get(get_driver_raw))
        .with_state(station)
        .route_layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ));

    let auth_router = Router::new()
        .route("/api/auth/status", get(auth::auth_status))
        .with_state(auth_state);

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

/// GET /api/state
/// Returns the full unified station snapshot (OCPP 1.6-J & EVCC aligned).
async fn get_state(State(station): State<SharedStation>) -> impl IntoResponse {
    logger::debug("API", "🔍 GET request /api/state");
    Json(station.snapshot())
}

/// GET /api/station
/// Returns the station hardware identity and system status.
async fn get_station(State(station): State<SharedStation>) -> impl IntoResponse {
    logger::debug("API", "🔍 GET request /api/station");
    Json(station.snapshot().station)
}

/// GET /api/connectors
/// Returns all charge point connectors and their current states.
async fn get_connectors(State(station): State<SharedStation>) -> impl IntoResponse {
    logger::debug("API", "🔍 GET request /api/connectors");
    Json(station.snapshot().connectors)
}

/// GET /api/connectors/:id
/// Returns a specific charge point connector by its ID (1 or 2).
async fn get_connector_by_id(
    State(station): State<SharedStation>,
    axum::extract::Path(id): axum::extract::Path<u8>,
) -> impl IntoResponse {
    logger::debug("API", &format!("🔍 GET request /api/connectors/{}", id));
    let snap = station.snapshot();
    if let Some(conn) = snap.connectors.into_iter().find(|c| c.id == id) {
        (StatusCode::OK, Json(json!(conn)))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "error",
                "error": format!("Connector {} not found. Available connectors: {:?}", id, snap.station.available_connectors)
            })),
        )
    }
}

/// GET /api/connectors/:id/meter
/// Returns electrical metering for a specific connector.
async fn get_connector_meter_by_id(
    State(station): State<SharedStation>,
    axum::extract::Path(id): axum::extract::Path<u8>,
) -> impl IntoResponse {
    let snap = station.snapshot();
    if let Some(conn) = snap.connectors.into_iter().find(|c| c.id == id) {
        (StatusCode::OK, Json(json!(conn.meter)))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "error",
                "error": format!("Connector {} not found", id)
            })),
        )
    }
}

/// GET /api/connectors/:id/status
/// Returns charge automaton state (OCPP / IEC) for a specific connector.
async fn get_connector_status_by_id(
    State(station): State<SharedStation>,
    axum::extract::Path(id): axum::extract::Path<u8>,
) -> impl IntoResponse {
    let snap = station.snapshot();
    if let Some(conn) = snap.connectors.into_iter().find(|c| c.id == id) {
        (StatusCode::OK, Json(json!(conn.status)))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "error",
                "error": format!("Connector {} not found", id)
            })),
        )
    }
}

/// GET /api/meter
/// Returns electrical metering for the primary (or queried ?side=X) connector.
/// Tailored for fast EVCC polling.
async fn get_meter(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    Json(state.to_connector_data().meter)
}

/// GET /api/status
/// Returns charge automaton status (OCPP / IEC) for the primary (or queried ?side=X) connector.
/// Tailored for fast EVCC / Home Assistant status polling.
async fn get_status(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    Json(state.to_connector_data().status)
}

/// GET /api/driver/raw
/// Returns the raw low-level JSON snapshot (`AtmegaRawState` + `UsbLinkState`) directly from `greenup-driver`.
async fn get_driver_raw(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    if let Some(side) = query.side {
        let point = station.get_point(Some(side));
        Json(point.driver.get_snapshot_json())
    } else {
        Json(station.raw_driver_snapshot_json())
    }
}

fn check_serial_connected(state: &SharedState) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    state.sync_from_driver();
    let tel = state.telemetry.lock().unwrap_or_else(|e| e.into_inner());
    if tel.serial.status != crate::telemetry::SerialStatus::UsbConnected {
        let err_msg = match tel.serial.status {
            crate::telemetry::SerialStatus::UsbDisconnected => {
                "USB cable disconnected (no USB adapter detected)"
            }
            crate::telemetry::SerialStatus::UsbConnecting => {
                "USB serial link initializing (waiting for station board)"
            }
            crate::telemetry::SerialStatus::UsbNoResponse => {
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

fn check_mode_allows_commands(state: &SharedState) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if state.sniffer_mode() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "status": "error",
                "error": "Station is operating in Sniffer (passive monitor) mode. Serial commands are disabled.",
                "sniffer_mode": true
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
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
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
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    logger::log("API", &format!("🔄 Starting initialization sequence (Side {})", state.side()));
    tokio::spawn(async move {
        // TIC detection is never started automatically:
        // it must be explicitly requested via POST /api/tic/refresh.
        crate::station::trigger_init_sequence(&state).await;
    });
    (StatusCode::OK, Json(json!({"status": "success", "message": "Initialization sequence started"})))
}



/// POST /api/reset
async fn reset_board(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
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
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
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
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
    if let Err(res) = check_serial_connected(&state) {
        return res;
    }
    logger::log("API", &format!("🔄 Manual TIC detection request (TICTM:1) on Side {}", state.side()));
    match crate::station::start_tic_detection(&state).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "success", "message": "TIC detection started"}))),
        Err(TicStartError::NotSupported) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "TIC is not supported on this charging station model"})),
        ),
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

#[derive(serde::Deserialize, Debug, Clone)]
pub struct SetVoltagePayload {
    pub voltage: f32,
    pub ttl_seconds: Option<u64>,
}

/// POST /api/voltage
/// Injects an externally measured grid voltage (e.g. from Home Assistant, Linky TIC, Shelly).
/// Applies globally to the station (all active charge points).
async fn set_voltage(
    State(station): State<SharedStation>,
    Json(payload): Json<SetVoltagePayload>,
) -> impl IntoResponse {
    if payload.voltage < 100.0 || payload.voltage > 300.0 {
        logger::log("API", &format!("⛔ Rejecting out of bounds external voltage: {:.1}V", payload.voltage));
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "error": "Voltage must be between 100.0 and 300.0 V"
            })),
        );
    }

    let ttl = payload.ttl_seconds.unwrap_or(60);
    logger::log(
        "API",
        &format!("⚡ External grid voltage updated → {:.1}V (TTL: {}s)", payload.voltage, ttl),
    );

    // Apply to Point 1
    if let Ok(mut tel) = station.point1.telemetry.lock() {
        tel.set_external_voltage(payload.voltage, ttl);
    }

    // Apply to Point 2 if dual-point station
    if let Some(ref p2) = *station.point2.read().unwrap_or_else(|e| e.into_inner()) {
        if let Ok(mut tel) = p2.telemetry.lock() {
            tel.set_external_voltage(payload.voltage, ttl);
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "status": "success",
            "voltage": payload.voltage,
            "source": if ttl == 0 { "default" } else { "external" },
            "ttl_seconds": ttl
        })),
    )
}

/// POST /api/bluetooth
async fn set_bluetooth(
    State(station): State<SharedStation>,
    Query(query): Query<SideQuery>,
    Json(payload): Json<BluetoothPayload>,
) -> impl IntoResponse {
    let state = station.get_point(query.side);
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
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
    if let Err(res) = check_mode_allows_commands(&state) {
        return res;
    }
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
        crate::session::get_active_session_summary_for_side(target_side, &tel)
    } else {
        None
    };

    let saved_sessions = crate::session::list_saved_sessions_filtered(query.side);
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
pub struct ExportSessionQuery {
    pub format: Option<String>,
}

/// GET /api/sessions/:id/export?format=csv|txt|zip
/// Exports a session file in the requested format (csv, txt, zip). Defaults to csv.
async fn export_session_handler(
    Path(id): Path<String>,
    Query(query): Query<ExportSessionQuery>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if !crate::session::is_valid_session_id(&id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid session ID format" })),
        ));
    }

    let sessions_dir = crate::session::get_sessions_dir();
    let csv_path = sessions_dir.join(format!("{}.csv", id));
    let txt_path = sessions_dir.join(format!("{}.txt", id));

    let fmt = query.format.as_deref().unwrap_or("csv").trim().to_lowercase();
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
    match crate::session::delete_session(&id) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use crate::hardware_specs::HardwareCapabilities;
    use crate::station::{AppState, StationManager};
    use crate::telemetry::{OcppChargePointStatus, SerialStatus};

    fn make_test_station() -> (Arc<StationManager>, Arc<AppState>, mpsc::Receiver<Command>) {
        let (tx, rx) = mpsc::channel(20);
        let point1 = Arc::new(AppState::new_with_side(1, tx));
        {
            let mut info = point1.info.lock().unwrap();
            info.reference = "058001".to_string();
            info.serial_number = "TEST12345".to_string();
            info.hardware_version = "A".to_string();
            info.software_version = "1.03.25".to_string();
            info.link_version = "0.5.0-dev".to_string();
            info.week_year_production = "42W22".to_string();
            info.bluetooth_enabled = Some(true);
            info.capabilities = Some(HardwareCapabilities::from_reference("058001"));

            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
            tel.greenup_state = "A".to_string();
            tel.limit_amps = 16;
            tel.eliot_limit_amps = Some(16);
            tel.charge_authorized = true;
        }
        let station = Arc::new(StationManager::new(point1.clone(), None));
        (station, point1, rx)
    }

    fn make_dual_test_station() -> (Arc<StationManager>, Arc<AppState>, Arc<AppState>, mpsc::Receiver<Command>, mpsc::Receiver<Command>) {
        let (tx1, rx1) = mpsc::channel(20);
        let (tx2, rx2) = mpsc::channel(20);
        let point1 = Arc::new(AppState::new_with_side(1, tx1));
        let point2 = Arc::new(AppState::new_with_side(2, tx2));
        {
            let mut info1 = point1.info.lock().unwrap();
            info1.reference = "058011".to_string();
            info1.serial_number = "DUAL1".to_string();
            let mut tel1 = point1.telemetry.lock().unwrap();
            tel1.serial.status = SerialStatus::UsbConnected;
            tel1.greenup_state = "A".to_string();
            tel1.voltage = 230.0;
            tel1.power = 0.0;

            let mut info2 = point2.info.lock().unwrap();
            info2.reference = "058011".to_string();
            info2.serial_number = "DUAL2".to_string();
            let mut tel2 = point2.telemetry.lock().unwrap();
            tel2.serial.status = SerialStatus::UsbConnected;
            tel2.greenup_state = "C".to_string();
            tel2.current = 16.0;
            tel2.power = 3680.0;
            tel2.voltage = 230.0;
        }
        let station = Arc::new(StationManager::new(point1.clone(), Some(point2.clone())));
        (station, point1, point2, rx1, rx2)
    }

    #[test]
    fn test_is_valid_raw_command() {
        // Valid commands
        assert!(is_valid_raw_command("service"));
        assert!(is_valid_raw_command("0"));
        assert!(is_valid_raw_command("1"));
        assert!(is_valid_raw_command("WeekYearProduction?"));
        assert!(is_valid_raw_command("FM2:0"));
        assert!(is_valid_raw_command("TICTM:1"));
        assert!(is_valid_raw_command("12345678901234567890"));

        // Invalid commands
        assert!(!is_valid_raw_command(""));
        assert!(!is_valid_raw_command("123456789012345678901"));
        assert!(!is_valid_raw_command("FM2: 0"));
        assert!(!is_valid_raw_command("cmd\n"));
        assert!(!is_valid_raw_command("cmd\r\n"));
        assert!(!is_valid_raw_command("test;rm"));
        assert!(!is_valid_raw_command("cmd|pipe"));
        assert!(!is_valid_raw_command("$VAR"));
        assert!(!is_valid_raw_command("cmd=val"));
    }

    #[tokio::test]
    async fn test_state_and_connector_endpoints() {
        let (station, _p1, _rx) = make_test_station();

        // 1. GET /api/state (single point)
        let res_state = get_state(State(station.clone())).await.into_response();
        assert_eq!(res_state.status(), StatusCode::OK);

        // 2. GET /api/station
        let res_station = get_station(State(station.clone())).await.into_response();
        assert_eq!(res_station.status(), StatusCode::OK);

        // 3. GET /api/connectors
        let res_connectors = get_connectors(State(station.clone())).await.into_response();
        assert_eq!(res_connectors.status(), StatusCode::OK);

        // 4. GET /api/connectors/:id
        let res_c1 = get_connector_by_id(State(station.clone()), axum::extract::Path(1)).await.into_response();
        assert_eq!(res_c1.status(), StatusCode::OK);
        let res_c99 = get_connector_by_id(State(station.clone()), axum::extract::Path(99)).await.into_response();
        assert_eq!(res_c99.status(), StatusCode::NOT_FOUND);

        // 5. GET /api/connectors/:id/meter
        let res_m1 = get_connector_meter_by_id(State(station.clone()), axum::extract::Path(1)).await.into_response();
        assert_eq!(res_m1.status(), StatusCode::OK);
        let res_m99 = get_connector_meter_by_id(State(station.clone()), axum::extract::Path(99)).await.into_response();
        assert_eq!(res_m99.status(), StatusCode::NOT_FOUND);

        // 6. GET /api/connectors/:id/status
        let res_s1 = get_connector_status_by_id(State(station.clone()), axum::extract::Path(1)).await.into_response();
        assert_eq!(res_s1.status(), StatusCode::OK);
        let res_s99 = get_connector_status_by_id(State(station.clone()), axum::extract::Path(99)).await.into_response();
        assert_eq!(res_s99.status(), StatusCode::NOT_FOUND);

        // 7. GET /api/meter & GET /api/status (primary)
        let res_meter = get_meter(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_meter.status(), StatusCode::OK);
        let res_status = get_status(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_status.status(), StatusCode::OK);

        // Dual point station tests
        let (dual_station, _dp1, _dp2, _drx1, _drx2) = make_dual_test_station();
        let dual_meter_p1 = get_meter(State(dual_station.clone()), Query(SideQuery { side: Some(1) })).await.into_response();
        assert_eq!(dual_meter_p1.status(), StatusCode::OK);

        let dual_status_p2 = get_status(State(dual_station.clone()), Query(SideQuery { side: Some(2) })).await.into_response();
        assert_eq!(dual_status_p2.status(), StatusCode::OK);

        let snap_res = get_state(State(dual_station.clone())).await.into_response();
        assert_eq!(snap_res.status(), StatusCode::OK);
        let snap = dual_station.snapshot();
        assert_eq!(snap.connectors[0].status.ocpp, OcppChargePointStatus::Available);
        assert_eq!(snap.connectors[1].status.ocpp, OcppChargePointStatus::Charging);
    }

    #[tokio::test]
    async fn test_set_voltage_endpoint() {
        let (station, point1, _rx) = make_test_station();

        // 1. Out of bounds voltage (< 100V) -> 400 Bad Request
        let res = set_voltage(State(station.clone()), Json(SetVoltagePayload {
            voltage: 80.0,
            ttl_seconds: Some(60),
        })).await.into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 2. Out of bounds voltage (> 300V) -> 400 Bad Request
        let res = set_voltage(State(station.clone()), Json(SetVoltagePayload {
            voltage: 350.0,
            ttl_seconds: Some(60),
        })).await.into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 3. Valid voltage -> 200 OK
        let res = set_voltage(State(station.clone()), Json(SetVoltagePayload {
            voltage: 236.4,
            ttl_seconds: Some(120),
        })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);

        {
            let tel1 = point1.telemetry.lock().unwrap();
            assert_eq!(tel1.voltage, 236.4);
            assert_eq!(tel1.voltage_source, "external");
            assert!(tel1.voltage_ttl_remaining_s.is_some());
        }

        // 4. Reset voltage with ttl=0 -> falls back to default 230V
        let res = set_voltage(State(station.clone()), Json(SetVoltagePayload {
            voltage: 230.0,
            ttl_seconds: Some(0),
        })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);

        {
            let tel1 = point1.telemetry.lock().unwrap();
            assert_eq!(tel1.voltage, 230.0);
            assert_eq!(tel1.voltage_source, "default");
            assert_eq!(tel1.voltage_ttl_remaining_s, None);
        }
    }

    #[tokio::test]
    async fn test_refresh_tic_endpoint_guards() {
        let (station, point1, _rx) = make_test_station();

        // 1. When USB serial is disconnected -> SERVICE_UNAVAILABLE (503)
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let response = refresh_tic(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Mark USB connected
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Hardware without TIC (e.g. 058030) -> BAD_REQUEST (400)
        {
            let mut info = point1.info.lock().unwrap();
            info.capabilities = Some(HardwareCapabilities::from_reference("058030"));
        }
        let response = refresh_tic(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // 3. Hardware with TIC (058001), but state is not "A" -> BAD_REQUEST (400)
        {
            let mut info = point1.info.lock().unwrap();
            info.capabilities = Some(HardwareCapabilities::from_reference("058001"));
            let mut tel = point1.telemetry.lock().unwrap();
            tel.greenup_state = "C".to_string();
        }
        let response = refresh_tic(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // 4. Hardware with TIC and state is "A" -> OK (200)
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.greenup_state = "A".to_string();
        }
        let response = refresh_tic(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sniffer_mode_command_guards() {
        let (tx, _rx) = mpsc::channel(20);
        let point1 = Arc::new(AppState::new_with_side_and_sniffer(1, true, tx));
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
            tel.greenup_state = "A".to_string();
        }
        let station = Arc::new(StationManager::new(point1.clone(), None));

        assert!(point1.sniffer_mode());
        assert!(station.snapshot().station.sniffer_mode);

        // In Sniffer mode, all TX command APIs must return 403 Forbidden
        let res_charge = set_charge_state(
            State(station.clone()),
            Query(SideQuery { side: None }),
            Json(ChargeActionPayload { action: "enable".to_string() }),
        ).await.into_response();
        assert_eq!(res_charge.status(), StatusCode::FORBIDDEN);

        let res_current = set_current(
            State(station.clone()),
            axum::extract::Path(16),
            Query(SideQuery { side: None }),
        ).await.into_response();
        assert_eq!(res_current.status(), StatusCode::FORBIDDEN);

        let res_raw = send_raw_command(
            State(station.clone()),
            Query(SideQuery { side: None }),
            "RESET".to_string(),
        ).await.into_response();
        assert_eq!(res_raw.status(), StatusCode::FORBIDDEN);

        let res_init = init_sequence(
            State(station.clone()),
            Query(SideQuery { side: None }),
        ).await.into_response();
        assert_eq!(res_init.status(), StatusCode::FORBIDDEN);

        let res_reset = reset_board(
            State(station.clone()),
            Query(SideQuery { side: None }),
        ).await.into_response();
        assert_eq!(res_reset.status(), StatusCode::FORBIDDEN);

        let res_bt = set_bluetooth(
            State(station.clone()),
            Query(SideQuery { side: None }),
            Json(BluetoothPayload { enabled: true }),
        ).await.into_response();
        assert_eq!(res_bt.status(), StatusCode::FORBIDDEN);

        let res_tic = refresh_tic(
            State(station.clone()),
            Query(SideQuery { side: None }),
        ).await.into_response();
        assert_eq!(res_tic.status(), StatusCode::FORBIDDEN);

        // In Sniffer mode, set_voltage (software-only calculation) must still succeed (200 OK)
        let res_voltage = set_voltage(
            State(station.clone()),
            Json(SetVoltagePayload {
                voltage: 235.0,
                ttl_seconds: Some(60),
            }),
        ).await.into_response();
        assert_eq!(res_voltage.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_charge_actions_and_guards() {
        let (station, point1, mut rx) = make_test_station();

        // 1. USB disconnected -> 503
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "enable".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Reconnect USB
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Invalid action -> 400
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "invalid_action".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 3. Action enable -> sends AuthorizeType2(true), SetExternalSignal(false), SetDerogation(true)
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "enable".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::AuthorizeType2(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetExternalSignal(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDerogation(true));

        // 4. Action disable -> sends SetExternalSignal(true), SetDerogation(false)
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "disable".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::SetExternalSignal(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDerogation(false));

        // 5. Action pause -> sends SetStartButton(false)
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "pause".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::SetStartButton(false));

        // 6. Action resume -> sends SetStartButton(true)
        let res = set_charge_state(State(station.clone()), Query(SideQuery { side: None }), Json(ChargeActionPayload { action: "resume".to_string() })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::SetStartButton(true));
    }

    #[tokio::test]
    async fn test_current_limit_endpoints_and_guards() {
        let (station, point1, mut rx) = make_test_station();

        // 1. USB disconnected -> 503
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let res = set_current(State(station.clone()), axum::extract::Path(16), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Reconnect USB
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Out of bounds current (< 7 or > 32) -> 400
        let res_low = set_current(State(station.clone()), axum::extract::Path(6), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_low.status(), StatusCode::BAD_REQUEST);

        let res_high = set_current(State(station.clone()), axum::extract::Path(33), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_high.status(), StatusCode::BAD_REQUEST);

        // 3. Valid current limit -> 200 OK
        let res = set_current(State(station.clone()), axum::extract::Path(24), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, Command::SetCurrentLimit(24));
    }

    #[tokio::test]
    async fn test_bluetooth_endpoint() {
        let (station, point1, mut rx) = make_test_station();

        // 1. Disconnected -> 503
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let res = set_bluetooth(State(station.clone()), Query(SideQuery { side: None }), Json(BluetoothPayload { enabled: true })).await.into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Reconnect
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Enable Bluetooth -> 200 OK, sends SetBluetooth(true)
        let res = set_bluetooth(State(station.clone()), Query(SideQuery { side: None }), Json(BluetoothPayload { enabled: true })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::SetBluetooth(true));

        // 3. Disable Bluetooth -> 200 OK, sends SetBluetooth(false)
        let res = set_bluetooth(State(station.clone()), Query(SideQuery { side: None }), Json(BluetoothPayload { enabled: false })).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::SetBluetooth(false));
    }

    #[tokio::test]
    async fn test_init_and_reset_endpoints() {
        let (station, point1, mut rx) = make_test_station();

        // 1. Disconnected -> 503
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let res_init = init_sequence(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_init.status(), StatusCode::SERVICE_UNAVAILABLE);
        let res_reset = reset_board(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_reset.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Reconnect
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Init -> 200 OK
        let res_init = init_sequence(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_init.status(), StatusCode::OK);

        // 3. Reset -> 200 OK, sends Command::Reset
        let res_reset = reset_board(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_reset.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::Reset);
    }

    #[tokio::test]
    async fn test_raw_command_endpoint() {
        let (station, point1, mut rx) = make_test_station();

        // 1. Disconnected -> 503
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbDisconnected;
        }
        let res = send_raw_command(State(station.clone()), Query(SideQuery { side: None }), "State?".to_string()).await.into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Reconnect
        {
            let mut tel = point1.telemetry.lock().unwrap();
            tel.serial.status = SerialStatus::UsbConnected;
        }

        // 2. Invalid format -> 400
        let res_bad = send_raw_command(State(station.clone()), Query(SideQuery { side: None }), "cmd with space".to_string()).await.into_response();
        assert_eq!(res_bad.status(), StatusCode::BAD_REQUEST);

        // 3. Valid command -> 200 OK, sends Raw
        let res_ok = send_raw_command(State(station.clone()), Query(SideQuery { side: None }), "State?".to_string()).await.into_response();
        assert_eq!(res_ok.status(), StatusCode::OK);
        assert_eq!(rx.recv().await.unwrap(), Command::Raw("State?".to_string()));
    }

    #[tokio::test]
    async fn test_logs_and_level_endpoints() {
        // 1. GET /api/logs
        let res_logs = get_logs().await.into_response();
        assert_eq!(res_logs.status(), StatusCode::OK);

        // 2. GET /api/logs/level
        let res_level = get_log_level_handler().await.into_response();
        assert_eq!(res_level.status(), StatusCode::OK);

        // 3. POST /api/logs/level (valid)
        let res_set_debug = set_log_level_handler(Json(LogLevelPayload { level: "DEBUG".to_string() })).await.into_response();
        assert_eq!(res_set_debug.status(), StatusCode::OK);

        // 4. POST /api/logs/level (invalid)
        let res_set_bad = set_log_level_handler(Json(LogLevelPayload { level: "INVALID_LEVEL_123".to_string() })).await.into_response();
        assert_eq!(res_set_bad.status(), StatusCode::BAD_REQUEST);

        // 5. GET /api/logs/export
        let res_export = export_logs().await.into_response();
        assert_eq!(res_export.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sessions_endpoints() {
        let (station, _p1, _rx) = make_test_station();

        // 1. GET /api/sessions -> 200 OK
        let res_sessions = list_sessions_handler(State(station.clone()), Query(SideQuery { side: None })).await.into_response();
        assert_eq!(res_sessions.status(), StatusCode::OK);

        // 2. DELETE /api/sessions/active -> 409 Conflict (cannot delete active session)
        let res_del_active = delete_session_handler(Path("active".to_string())).await;
        assert!(res_del_active.is_err());
        assert_eq!(res_del_active.unwrap_err().0, StatusCode::CONFLICT);

        // 3. DELETE /api/sessions/non_existent -> 404 Not Found
        let res_del_unknown = delete_session_handler(Path("non_existent_session_id_404".to_string())).await;
        assert!(res_del_unknown.is_err());
        assert_eq!(res_del_unknown.unwrap_err().0, StatusCode::NOT_FOUND);

        // 4. GET /api/sessions/:id/export with invalid format -> 400 Bad Request
        let res_export_bad = export_session_handler(
            Path("some_session".to_string()),
            Query(ExportSessionQuery { format: Some("invalid_format".to_string()) }),
        ).await;
        assert!(res_export_bad.is_err());
        assert_eq!(res_export_bad.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_kiosk_mode_vs_standard_router() {
        use tower::ServiceExt;
        use axum::body::Body;

        let (station, _p1, _rx) = make_test_station();

        // 1. Standard Mode (kiosk_mode = false) with token
        let strict_config = HttpConfig {
            bind: "0.0.0.0:8080".to_string(),
            api_token: Some("secret_pass_123".to_string()),
            kiosk_mode: false,
        };
        let app_strict = build_router(station.clone(), strict_config);

        // GET /api/state without token -> 401 Unauthorized
        let res = app_strict.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/state")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // GET /api/auth/status without token -> 200 OK (public endpoint)
        let res = app_strict.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/auth/status")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // GET /api/state WITH Bearer token -> 200 OK
        let res = app_strict.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/state")
                .header("Authorization", "Bearer secret_pass_123")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 2. Kiosk Mode (kiosk_mode = true) with token
        let kiosk_config = HttpConfig {
            bind: "0.0.0.0:8080".to_string(),
            api_token: Some("secret_pass_123".to_string()),
            kiosk_mode: true,
        };
        let app_kiosk = build_router(station.clone(), kiosk_config);

        // GET /api/state without token -> 200 OK in Kiosk mode!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/state")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // GET /api/station without token -> 200 OK in Kiosk mode!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/station")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // GET /api/logs without token -> 200 OK in Kiosk mode!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/logs")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // GET /api/sessions/2026-11-02_12-00-00_point1/export without token -> reaches handler (404 Not Found, NOT 401)
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/sessions/2026-11-02_12-00-00_point1/export?format=csv")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // DELETE /api/sessions/2026-11-02_12-00-00_point1 without token -> 401 Unauthorized!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/api/sessions/2026-11-02_12-00-00_point1")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Sensitive GET /api/logs/export without token -> 401 Unauthorized!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .uri("/api/logs/export")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Sensitive POST /api/charge without token -> 401 Unauthorized!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/charge")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"action":"enable"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Sensitive POST /api/current/16 without token -> 401 Unauthorized!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/current/16")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Sensitive POST /api/charge WITH Bearer token -> 200 OK in Kiosk mode!
        let res = app_kiosk.clone().oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/charge")
                .header("Content-Type", "application/json")
                .header("Authorization", "Bearer secret_pass_123")
                .body(Body::from(r#"{"action":"enable"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
