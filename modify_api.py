import re

with open('crates/greenup-link/src/api.rs', 'r', encoding='utf-8') as f:
    text = f.read()

# Router changes
text = re.sub(r'\.route\("/api/charge/enable", post\(enable_charge\)\)', '.route("/api/charge", post(set_charge_state))', text)
text = re.sub(r'\.route\("/api/charge/disable", post\(disable_charge\)\)\r?\n\s*', '', text)
text = re.sub(r'\.route\("/api/charge/pause", post\(pause_charge\)\)\r?\n\s*', '', text)
text = re.sub(r'\.route\("/api/charge/resume", post\(resume_charge\)\)\r?\n\s*', '', text)
text = re.sub(r'\.route\("/api/evcc/state", post\(set_evcc_state\)\)\r?\n\s*', '', text)

# Delete evcc_state payload and function
text = re.sub(r'#\[derive\(serde::Deserialize\)\]\s*struct EvccStatePayload.*?^\}$', '', text, flags=re.DOTALL|re.MULTILINE)

# Delete enable_charge
text = re.sub(r'/// POST /api/charge/enable\s*async fn enable_charge.*?^\}', '', text, flags=re.DOTALL|re.MULTILINE)
text = re.sub(r'/// POST /api/charge/disable\s*async fn disable_charge.*?^\}', '', text, flags=re.DOTALL|re.MULTILINE)
text = re.sub(r'/// POST /api/charge/pause\s*async fn pause_charge.*?^\}', '', text, flags=re.DOTALL|re.MULTILINE)
text = re.sub(r'/// POST /api/charge/resume\s*async fn resume_charge.*?^\}', '', text, flags=re.DOTALL|re.MULTILINE)

new_api = '''
#[derive(serde::Deserialize)]
pub struct ChargeActionPayload { action: String }

/// POST /api/charge
async fn set_charge_state(State(state): State<SharedState>, Json(payload): Json<ChargeActionPayload>) -> impl IntoResponse {
    match payload.action.as_str() {
        "enable" => {
            logger::log("API", "▶ Charge enable request (T2COK + FM2:0 + DOK)");
            let _ = state.serial_tx.send(Command::AuthorizeType2(true)).await;
            let _ = state.serial_tx.send(Command::SetExternalSignal(false)).await;
            if state.serial_tx.send(Command::SetDerogation(true)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "disable" => {
            logger::log("API", "▶ Charge disable request (FM2:1 + DNOK)");
            let _ = state.serial_tx.send(Command::SetExternalSignal(true)).await;
            if state.serial_tx.send(Command::SetDerogation(false)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "pause" => {
            logger::log("API", "▶ Charge pause request (SBNOK)");
            if state.serial_tx.send(Command::SetStartButton(false)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        "resume" => {
            logger::log("API", "▶ Charge resume request (SBOK)");
            if state.serial_tx.send(Command::SetStartButton(true)).await.is_ok() {
                (StatusCode::OK, Json(json!({"status": "success"})))
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Serial error"})))
            }
        },
        _ => (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid action"})))
    }
}
'''
text += new_api

with open('crates/greenup-link/src/api.rs', 'w', encoding='utf-8') as f:
    f.write(text)
