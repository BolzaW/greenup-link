mod api;
mod auth;
mod config;

use std::sync::{atomic::AtomicU8, Arc, Mutex};
use tokio::sync::mpsc;
use greenup_driver::state::{AppState, BoardInfo, StationManager, Telemetry};

fn discover_candidate_ports(configured_port: Option<&str>, configured_port2: Option<&str>) -> (String, String) {
    let mut available: Vec<String> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .filter(|name| {
            name.contains("ttyUSB")
                || name.contains("ttyACM")
                || name.starts_with("COM")
                || name.contains("cu.usbserial")
        })
        .collect();

    available.sort();

    let port1 = match configured_port {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => available.first().cloned().unwrap_or_else(|| "/dev/ttyUSB0".to_string()),
    };

    let port2 = match configured_port2 {
        Some(p2) if !p2.trim().is_empty() => p2.trim().to_string(),
        _ => available
            .iter()
            .find(|&p| p != &port1)
            .cloned()
            .unwrap_or_else(|| {
                if port1.ends_with('0') {
                    format!("{}1", &port1[..port1.len() - 1])
                } else if port1 == "/dev/ttyUSB0" {
                    "/dev/ttyUSB1".to_string()
                } else {
                    "/dev/ttyUSB1".to_string()
                }
            }),
    };

    (port1, port2)
}

fn spawn_serial_worker(
    side: u8,
    port_name: String,
    baud_rate: u32,
    watchdog_timeout_secs: u64,
    link_version: &str,
) -> (Arc<AppState>, tokio::sync::mpsc::Sender<greenup_protocol::Command>) {
    let (tx, rx) = mpsc::channel::<greenup_protocol::Command>(32);
    let mut initial_info = BoardInfo::default();
    initial_info.link_version = link_version.to_string();
    initial_info.side = Some(side);

    let mut tel = Telemetry::default();
    tel.side = side;

    let app_state = Arc::new(AppState {
        side: AtomicU8::new(side),
        telemetry: Mutex::new(tel),
        info: Mutex::new(initial_info),
        serial_tx: tx.clone(),
        tic_detection: Mutex::new(greenup_driver::state::TicDetectionState::default()),
        tokio_handle: tokio::runtime::Handle::current(),
    });

    let state_for_serial = app_state.clone();
    let serial_options = greenup_driver::SerialOptions {
        port: port_name,
        baud_rate,
        timeout_ms: 100,
        watchdog_timeout_secs,
    };
    tokio::task::spawn_blocking(move || {
        greenup_driver::run_serial_loop_with_options(state_for_serial, rx, serial_options);
    });

    (app_state, tx)
}

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
        greenup_driver::logger::log(
            "SYS",
            "🔓 API authentication: DISABLED (open LAN mode — set api_token in greenup.toml to secure)",
        );
    }

    let (port1_name, port2_candidate) = discover_candidate_ports(
        app_config.serial.port.as_deref(),
        app_config.serial.port2.as_deref(),
    );

    greenup_driver::logger::log(
        "SYS",
        &format!("🔌 Serial port resolved: Point 1 = {} (Point 2 candidate = {})", port1_name, port2_candidate),
    );

    let (app_state_1, _tx1) = spawn_serial_worker(
        1,
        port1_name,
        app_config.serial.baud_rate,
        app_config.serial.watchdog_timeout_secs,
        pkg_version,
    );

    let (station, should_watch_auto) = match app_config.serial.dual_point {
        config::DualPointMode::True => {
            greenup_driver::logger::log(
                "SYS",
                &format!("⚡ Dual charge point mode: FORCED (Point 1: active, Point 2: {})", port2_candidate),
            );
            let (app_state_2, _tx2) = spawn_serial_worker(
                2,
                port2_candidate.clone(),
                app_config.serial.baud_rate,
                app_config.serial.watchdog_timeout_secs,
                pkg_version,
            );
            greenup_driver::logger::set_dual_station(true);
            (Arc::new(StationManager::new(app_state_1, Some(app_state_2))), false)
        }
        config::DualPointMode::False => {
            greenup_driver::logger::log("SYS", "⚡ Single charge point mode: FORCED");
            greenup_driver::logger::set_dual_station(false);
            (Arc::new(StationManager::new(app_state_1, None)), false)
        }
        config::DualPointMode::Auto => {
            greenup_driver::logger::log("SYS", "⚡ Dual charge point mode: AUTO (detecting from board reference)");
            greenup_driver::logger::set_dual_station(false);
            (Arc::new(StationManager::new(app_state_1, None)), true)
        }
    };

    if should_watch_auto {
        let station_clone = station.clone();
        let port2_target = port2_candidate;
        let baud = app_config.serial.baud_rate;
        let wd = app_config.serial.watchdog_timeout_secs;
        let ver = pkg_version.to_string();

        tokio::spawn(async move {
            for _ in 0..60 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if station_clone.is_dual() {
                    return;
                }
                let (has_ref, is_2pt, ref_str) = {
                    let info = station_clone.point1.info.lock().unwrap_or_else(|e| e.into_inner());
                    let is_2pt = info.capabilities.as_ref().map(|c| c.charging_points == 2).unwrap_or(false);
                    (!info.reference.is_empty(), is_2pt, info.reference.clone())
                };

                if has_ref {
                    if is_2pt {
                        greenup_driver::logger::log(
                            "SYS",
                            &format!(
                                "⚡ Station reference '{}' identified as Dual Charge Point: automatically starting Point 2 on {}",
                                ref_str, port2_target
                            ),
                        );
                        let (app_state_2, _tx2) = spawn_serial_worker(2, port2_target, baud, wd, &ver);
                        station_clone.attach_point2(app_state_2);
                        greenup_driver::logger::set_dual_station(true);
                    } else {
                        greenup_driver::logger::log(
                            "SYS",
                            &format!("ℹ️ Station reference '{}' confirmed Single Charge Point", ref_str),
                        );
                    }
                    return;
                }
            }
        });
    }

    // Start the Axum HTTP server
    let router = api::build_router(station, app_config.http.api_token);

    let addr = &app_config.http.bind;
    greenup_driver::logger::log("SYS", &format!("🌍 Web server started: http://{}", addr));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    axum::serve(listener, router).await.expect("Fatal web server error");
}
