use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use crate::flash;
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
        // ⚠️ Routes expérimentales — Flash firmware
        .route("/api/flash/status", get(flash_status))
        .route("/api/flash/firmwares", get(flash_list_firmwares))
        .route("/api/flash/prepare", post(flash_prepare))
        .route("/api/flash/execute", post(flash_execute))
        .route("/api/flash/upload", post(flash_upload))
        .route("/api/flash/reset", post(flash_reset))
        .with_state(state)
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

// ===========================================================================
// ⚠️  FLASH FIRMWARE — API EXPÉRIMENTALE
// ===========================================================================

/// GET /api/flash/status — État courant du processus de flash
async fn flash_status(State(state): State<SharedState>) -> impl IntoResponse {
    let flash_state = state.flash_state.lock().unwrap().clone();
    Json(json!({
        "experimental": true,
        "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
        "flash": flash_state,
    }))
}

/// GET /api/flash/firmwares — Liste les firmwares .hex disponibles
async fn flash_list_firmwares(State(state): State<SharedState>) -> impl IntoResponse {
    let firmwares = flash::list_available_firmwares();
    let current_version = {
        let info = state.info.lock().unwrap();
        flash::FirmwareVersion::from_board_string(&info.software_version)
    };

    Json(json!({
        "experimental": true,
        "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
        "current_version": current_version,
        "firmwares": firmwares,
    }))
}

/// Payload JSON pour la préparation du flash
#[derive(serde::Deserialize)]
struct FlashPrepareRequest {
    /// Chemin vers le fichier .hex à flasher
    hex_file: String,
    /// Port série cible (optionnel, auto-détecté si absent)
    target_port: Option<String>,
    /// L'utilisateur doit explicitement accepter le risque
    i_accept_the_risk: bool,
}

/// POST /api/flash/prepare — Prépare le flash (validation, détection downgrade)
async fn flash_prepare(
    State(state): State<SharedState>,
    Json(payload): Json<FlashPrepareRequest>,
) -> impl IntoResponse {
    // L'utilisateur DOIT explicitement accepter le risque
    if !payload.i_accept_the_risk {
        logger::log("FLASH", "⛔ Rejet : l'utilisateur n'a pas accepté le risque");
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Vous devez explicitement accepter le risque en passant i_accept_the_risk: true",
                "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
            })),
        );
    }

    // Détecter le port série si non spécifié
    let target_port = match payload.target_port {
        Some(p) => p,
        None => match flash::detect_serial_port() {
            Some(p) => p,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Aucun port série détecté (/dev/ttyUSB0 ou /dev/ttyUSB1)"})),
                );
            }
        },
    };

    match flash::prepare_flash(&state, &payload.hex_file, &target_port) {
        Ok(flash_state) => (
            StatusCode::OK,
            Json(json!({
                "experimental": true,
                "status": "ready",
                "message": "Flash préparé. Appelez POST /api/flash/execute pour lancer le flash.",
                "flash": flash_state,
                "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": e,
                "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
            })),
        ),
    }
}

/// POST /api/flash/execute — Lance le flash (bloquant, exécuté dans un thread dédié)
async fn flash_execute(State(state): State<SharedState>) -> impl IntoResponse {
    // Vérifier qu'un flash a été préparé
    {
        let fs = state.flash_state.lock().unwrap();
        if fs.status != flash::FlashStatus::Ready {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Aucun flash préparé. Appelez POST /api/flash/prepare d'abord.",
                    "current_status": format!("{:?}", fs.status),
                })),
            );
        }
    }

    logger::log("FLASH", "🚀 Lancement du flash dans un thread dédié...");

    // Le flash est bloquant (avrdude), on le lance dans un thread dédié
    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || {
        let _ = flash::execute_flash(&state_clone);
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "experimental": true,
            "status": "started",
            "message": "Flash lancé en arrière-plan. Suivez la progression via GET /api/flash/status",
            "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
        })),
    )
}

/// POST /api/flash/upload — Upload un fichier .hex via multipart/form-data
async fn flash_upload(
    State(_state): State<SharedState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name != "firmware" {
            continue;
        }

        let filename = field
            .file_name()
            .unwrap_or("uploaded_firmware.hex")
            .to_string();

        // Vérifier l'extension
        if !filename.ends_with(".hex") {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Le fichier doit avoir l'extension .hex"})),
            );
        }

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Erreur de lecture du fichier: {}", e)})),
                );
            }
        };

        // Sauvegarder le fichier
        let upload_dir = "/home/pi/Desktop/Java/Upload/Firmware";
        let _ = std::fs::create_dir_all(upload_dir);
        let dest_path = format!("{}/{}", upload_dir, filename);

        match std::fs::write(&dest_path, &data) {
            Ok(_) => {
                logger::log(
                    "FLASH",
                    &format!("📁 Firmware uploadé : {} ({} octets)", dest_path, data.len()),
                );

                // Valider le fichier uploadé
                let validation = flash::validate_hex_file(&dest_path);

                return (
                    StatusCode::OK,
                    Json(json!({
                        "experimental": true,
                        "status": "uploaded",
                        "filename": filename,
                        "path": dest_path,
                        "size": data.len(),
                        "validation": validation,
                        "version": flash::FirmwareVersion::from_filename(&filename),
                        "next_step": "Appelez POST /api/flash/prepare avec ce chemin pour préparer le flash.",
                        "disclaimer": flash::EXPERIMENTAL_DISCLAIMER,
                    })),
                );
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Impossible d'écrire le fichier: {}", e)})),
                );
            }
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "Aucun champ 'firmware' trouvé dans le multipart. Envoyez le .hex dans un champ nommé 'firmware'."})),
    )
}

/// POST /api/flash/reset — Réinitialise l'état du flash
async fn flash_reset(State(state): State<SharedState>) -> impl IntoResponse {
    flash::reset_flash_state(&state);
    logger::log("FLASH", "🔄 État du flash réinitialisé");
    (
        StatusCode::OK,
        Json(json!({"status": "reset", "message": "État du flash réinitialisé"})),
    )
}
