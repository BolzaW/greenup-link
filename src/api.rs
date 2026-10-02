use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use crate::logger;
use crate::state::SharedState;

pub fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(serve_ui))
        .route("/api/info", get(get_info))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/current/:amps", post(set_current))
        .route("/api/charge/start", post(start_charge))
        .route("/api/charge/stop", post(stop_charge))
        .route("/api/command", post(send_raw_command))
        .route("/api/tic/refresh", post(refresh_tic))
        .route("/api/bluetooth", post(set_bluetooth))
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

/// GET /api/info
async fn get_info(State(state): State<SharedState>) -> impl IntoResponse {
    let info = state.info.lock().unwrap().clone();
    Json(info)
}

/// GET /api/telemetry
async fn get_telemetry(State(state): State<SharedState>) -> impl IntoResponse {
    let telemetry = state.telemetry.lock().unwrap().clone();
    Json(telemetry)
}

/// POST /api/current/:amps
async fn set_current(State(state): State<SharedState>, Path(amps): Path<u32>) -> impl IntoResponse {
    if amps < 10 || amps > 32 {
        logger::log("API", &format!("⛔ Rejet courant hors limites: {}A", amps));
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Le courant doit être obligatoirement compris entre 10 et 32 Ampères"})),
        );
    }

    let cmd = format!("CC:{:02}\r", amps);
    logger::log("API", &format!("⚡ Modification limite courant → {}A", amps));
    
    if state.serial_tx.send(cmd).await.is_ok() {
        (
            StatusCode::OK,
            Json(json!({
                "status": "success", 
                "message": format!("Limite de courant définie sur {}A", amps)
            })),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Erreur interne: Impossible de communiquer avec le thread série"})),
        )
    }
}

/// POST /api/charge/start
async fn start_charge(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "▶ Demande de démarrage de charge (T2COK)");
    if state.serial_tx.send("T2COK\r".to_string()).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Charge autorisée (Type 2)"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Erreur de communication série"})))
    }
}

/// POST /api/charge/stop
async fn stop_charge(State(state): State<SharedState>) -> impl IntoResponse {
    logger::log("API", "⏹ Demande d'arrêt de charge (T2CNOK)");
    if state.serial_tx.send("T2CNOK\r".to_string()).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Charge stoppée (Type 2)"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Erreur de communication série"})))
    }
}

/// POST /api/command
async fn send_raw_command(State(state): State<SharedState>, body: String) -> impl IntoResponse {
    let cmd = format!("{}\r", body.trim());
    logger::log("API", &format!("🔧 Commande brute: {}", body.trim()));
    if state.serial_tx.send(cmd).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Commande envoyée"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Erreur de communication série"})))
    }
}

/// POST /api/tic/refresh
async fn refresh_tic(State(state): State<SharedState>) -> impl IntoResponse {
    let state_val = {
        let tel = state.telemetry.lock().unwrap();
        tel.state.clone()
    };

    if state_val != "A" {
        logger::log("API", &format!("⛔ Rejet détection TIC car borne non libre (State: {})", state_val));
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "La détection TIC ne peut se faire que lorsque la borne est libre (Statut A)"})),
        );
    }

    logger::log("API", "🔄 Demande manuelle de détection TIC (TICTM:1)");
    
    // Reset du compteur et state
    if let Ok(mut count) = state.tic_test_zero_count.lock() {
        *count = 0;
    }
    if let Ok(mut tel) = state.telemetry.lock() {
        tel.tic_mode = "detecting".to_string(); // Indicateur pour l'IHM
    }

    if state.serial_tx.send("TICTM:1\r".to_string()).await.is_ok() {
        (StatusCode::OK, Json(json!({"status": "success", "message": "Détection TIC lancée"})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Erreur de communication série"})))
    }
}

/// POST /api/bluetooth
async fn set_bluetooth(State(state): State<SharedState>, Json(payload): Json<BluetoothPayload>) -> impl IntoResponse {
    let cmd = if payload.enabled { "BTOK\r" } else { "BTNOK\r" };
    let log_msg = if payload.enabled { "🔵 Activation" } else { "⚪ Désactivation" };
    
    logger::log("API", &format!("{} Bluetooth demandée", log_msg));
    
    if state.serial_tx.send(cmd.to_string()).await.is_ok() {
        (
            StatusCode::OK,
            Json(json!({"status": "success", "message": format!("Bluetooth {} avec succès", if payload.enabled { "activé" } else { "désactivé" })}))
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Erreur de communication série"}))
        )
    }
}
