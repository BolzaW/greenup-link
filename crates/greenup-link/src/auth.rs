use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use greenup_driver::logger;

#[derive(Debug, Clone)]
pub struct AuthState {
    pub api_token: Option<Arc<String>>,
    pub auth_ihm: bool,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            api_token: None,
            auth_ihm: true,
        }
    }
}

pub type SharedAuthState = Arc<AuthState>;

/// Constant-time byte slice comparison to prevent timing side-channel attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Standard RFC 4648 Base64 decoder (zero external crate dependencies).
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace())
        .collect();

    if clean.is_empty() {
        return Some(Vec::new());
    }

    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;

    for &b in &clean {
        if b == b'=' {
            break;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }

    Some(out)
}

/// Simple percent-decoding for query parameter values (`?token=...`).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Checks whether an incoming HTTP request carries a valid token via:
/// 1. `Authorization: Bearer <token>`
/// 2. `Authorization: Basic <base64(user:token)>` (or `:token` / `token:`)
/// 3. `X-API-Key: <token>`
/// 4. Query parameter `?token=<token>` (used by browser EventSource / SSE and log downloads)
pub fn is_request_authorized(headers: &HeaderMap, query: Option<&str>, expected_token: &str) -> bool {
    let expected_bytes = expected_token.as_bytes();

    // 1 & 2. Check Authorization header (Bearer or Basic)
    if let Some(auth_val) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            let trimmed = auth_str.trim();

            if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
                let candidate = trimmed[7..].trim();
                if constant_time_eq(candidate.as_bytes(), expected_bytes) {
                    return true;
                }
            } else if trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("basic ") {
                let b64 = trimmed[6..].trim();
                if let Some(decoded_bytes) = decode_base64(b64) {
                    if let Ok(decoded_str) = std::str::from_utf8(&decoded_bytes) {
                        if let Some((user, pass)) = decoded_str.split_once(':') {
                            if constant_time_eq(pass.as_bytes(), expected_bytes) {
                                return true;
                            }
                            if pass.is_empty() && constant_time_eq(user.as_bytes(), expected_bytes) {
                                return true;
                            }
                        } else if constant_time_eq(decoded_str.as_bytes(), expected_bytes) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    // 3. Check X-API-Key header
    if let Some(api_key_val) = headers.get("x-api-key") {
        if let Ok(key_str) = api_key_val.to_str() {
            if constant_time_eq(key_str.trim().as_bytes(), expected_bytes) {
                return true;
            }
        }
    }

    // 4. Check ?token= query parameter (for SSE / EventSource and file export)
    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == "token" {
                    let decoded = percent_decode(v);
                    if constant_time_eq(decoded.as_bytes(), expected_bytes) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Returns true if the HTTP request is permitted in open consultation mode ("kiosk", `auth_ihm = false`).
pub fn is_kiosk_permitted(method: &axum::http::Method, path: &str) -> bool {
    if method != axum::http::Method::GET {
        return false;
    }

    // Explicitly exclude sensitive GET endpoints
    if path == "/api/logs/export" {
        return false;
    }

    // Allow read-only consultation endpoints
    path == "/api/state"
        || path == "/api/station"
        || path == "/api/connectors"
        || path.starts_with("/api/connectors/")
        || path == "/api/meter"
        || path == "/api/status"
        || path == "/api/sessions"
        || (path.starts_with("/api/sessions/") && path.ends_with("/export"))
        || path == "/api/mode"
        || path == "/api/logs"
        || path == "/api/logs/stream"
        || path == "/api/logs/level"
        || path == "/api/auth/status"
        || path == "/api/driver/raw"
}

/// Axum middleware enforcing API authentication when `api_token` is configured.
/// Deliberately omits the `WWW-Authenticate: Basic` response header on 401 so that
/// web browsers do not display their native basic-auth popup over the Web GUI.
pub async fn require_auth(
    State(auth_state): State<SharedAuthState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(ref expected) = auth_state.api_token else {
        return next.run(req).await;
    };

    // If client supplied a valid token, allow the request unconditionally
    if is_request_authorized(req.headers(), req.uri().query(), expected.as_str()) {
        return next.run(req).await;
    }

    // In Kiosk / Consultation Libre mode (auth_ihm == false), permit read-only GET requests
    if !auth_state.auth_ihm && is_kiosk_permitted(req.method(), req.uri().path()) {
        return next.run(req).await;
    }

    logger::warn(
        "API",
        &format!("⚠️ 🔒 Unauthorized request rejected (401): {} {}", req.method(), req.uri().path()),
    );
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "Unauthorized: valid API token required"
        })),
    )
        .into_response()
}

/// GET /api/auth/status
/// Public endpoint allowing the Web GUI to check if authentication is enabled
/// and whether the currently stored token is valid.
pub async fn auth_status(
    State(auth_state): State<SharedAuthState>,
    req: Request,
) -> impl IntoResponse {
    match auth_state.api_token {
        None => {
            logger::debug("API", "🔐 Auth status check: authentication disabled (open access)");
            Json(json!({
                "auth_required": false,
                "authenticated": true,
                "auth_ihm": false
            }))
        }
        Some(ref expected) => {
            let valid = is_request_authorized(req.headers(), req.uri().query(), expected.as_str());
            logger::debug(
                "API",
                &format!(
                    "🔐 Auth status check: required=true, authenticated={}, auth_ihm={}",
                    valid, auth_state.auth_ihm
                ),
            );
            Json(json!({
                "auth_required": true,
                "authenticated": valid,
                "auth_ihm": auth_state.auth_ihm
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_bearer_auth() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer my_secret_token"),
        );
        assert!(is_request_authorized(&headers, None, "my_secret_token"));
        assert!(!is_request_authorized(&headers, None, "wrong_token"));
    }

    #[test]
    fn test_basic_auth_variants() {
        let mut headers = HeaderMap::new();
        // "admin:secret123" in Base64 is "YWRtaW46c2VjcmV0MTIz"
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic YWRtaW46c2VjcmV0MTIz"),
        );
        assert!(is_request_authorized(&headers, None, "secret123"));
        assert!(!is_request_authorized(&headers, None, "admin"));

        // ":secret123" in Base64 is "OnNlY3JldDEyMw=="
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic OnNlY3JldDEyMw=="),
        );
        assert!(is_request_authorized(&headers, None, "secret123"));

        // "secret123:" in Base64 is "c2VjcmV0MTIzOg=="
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic c2VjcmV0MTIzOg=="),
        );
        assert!(is_request_authorized(&headers, None, "secret123"));
    }

    #[test]
    fn test_x_api_key_and_query_param() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("tok_42"));
        assert!(is_request_authorized(&headers, None, "tok_42"));

        let empty_headers = HeaderMap::new();
        assert!(is_request_authorized(
            &empty_headers,
            Some("foo=1&token=my%20token%21"),
            "my token!"
        ));
        assert!(!is_request_authorized(
            &empty_headers,
            Some("token=wrong"),
            "my token!"
        ));
    }

    #[test]
    fn test_is_kiosk_permitted_rules() {
        use axum::http::Method;

        // Permitted GET consultation routes
        assert!(is_kiosk_permitted(&Method::GET, "/api/state"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/station"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/connectors"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/connectors/1"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/connectors/1/meter"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/connectors/1/status"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/meter"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/status"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/sessions"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/sessions/123/export"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/mode"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/logs"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/logs/stream"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/logs/level"));
        assert!(is_kiosk_permitted(&Method::GET, "/api/auth/status"));

        // Strictly forbidden in Kiosk mode without auth
        assert!(!is_kiosk_permitted(&Method::GET, "/api/logs/export"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/charge"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/current/16"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/init"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/reset"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/tic/refresh"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/bluetooth"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/command"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/voltage"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/mode"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/logs/level"));
        assert!(!is_kiosk_permitted(&Method::POST, "/api/sessions/123/export"));
        assert!(!is_kiosk_permitted(&Method::DELETE, "/api/sessions/123"));
    }
}
