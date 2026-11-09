//! MQTT 3.1.1 client service for Green'up Link (`rumqttc`).
//!
//! Features:
//! - Autonomous async MQTT client connecting to an external broker (e.g. Mosquitto, Home Assistant).
//! - Automatic reconnection and Last Will & Testament (`<base_topic>/availability` -> `"offline"`).
//! - Shared authentication using `http.api_token` as MQTT password and configurable username.
//! - Atomic unitary scalar topics (`<base_topic>/...`) published with Report-by-Exception and periodic 60s refresh.
//! - Home Assistant MQTT Auto-Discovery payloads published to `homeassistant/<component>/greenup_<serial>/.../config`.
//! - Incoming command topics (`.../set`) for remote charging control, current limits, and external grid voltage injection.
//! - Strict sniffer mode protection (rejects all hardware TX commands when `sniffer_mode = true`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use greenup_driver::logger;
use greenup_protocol::{Command, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};

use crate::config::MqttConfig;
use crate::station::{SharedStation, StationSnapshot};

/// Parsed incoming MQTT command.
#[derive(Debug, PartialEq)]
pub enum MqttCommand {
    SetCharge {
        side: u8,
        action: String,
    },
    SetPause {
        side: u8,
        paused: bool,
    },
    SetCurrentLimit {
        side: u8,
        amps: f64,
    },
    SetVoltage {
        side: Option<u8>,
        voltage: f32,
        ttl_seconds: Option<u64>,
    },
}

/// Parses a broker string (e.g. `"127.0.0.1:1883"` or `"homeassistant.local"`) into host and port.
pub fn parse_broker_address(broker: &str) -> (String, u16) {
    let trimmed = broker.trim();
    if let Some((host, port_str)) = trimmed.split_once(':') {
        let port = port_str.parse::<u16>().unwrap_or(1883);
        (host.trim().to_string(), port)
    } else {
        (trimmed.to_string(), 1883)
    }
}

/// Resolves MQTT credentials using configured username and password (with fallback to `http.api_token`).
pub fn resolve_credentials(
    config_user: Option<&str>,
    config_pass: Option<&str>,
    api_token: Option<&str>,
) -> Option<(String, String)> {
    let pass = config_pass.or(api_token).map(str::trim).filter(|s| !s.is_empty());
    let user = config_user.map(str::trim).filter(|s| !s.is_empty());

    match (user, pass) {
        (Some(u), Some(p)) => Some((u.to_string(), p.to_string())),
        (None, Some(p)) => Some(("greenup-link".to_string(), p.to_string())),
        (Some(u), None) => Some((u.to_string(), String::new())),
        (None, None) => None,
    }
}

/// Sanitizes an identifier for safe use in MQTT topics and Home Assistant entity IDs.
pub fn sanitize_identifier(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "station".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parses an incoming MQTT command topic and payload.
pub fn parse_command_topic(
    base_topic: &str,
    topic: &str,
    payload_str: &str,
) -> Option<MqttCommand> {
    let relative = if topic.starts_with(base_topic) {
        let rest = &topic[base_topic.len()..];
        rest.trim_start_matches('/')
    } else {
        return None;
    };

    let segments: Vec<&str> = relative.split('/').collect();
    let payload = payload_str.trim();

    match segments.as_slice() {
        ["connector", id_str, "control", "charge", "set"] => {
            let side = id_str.parse::<u8>().ok()?;
            Some(MqttCommand::SetCharge {
                side,
                action: payload.to_string(),
            })
        }
        ["connector", id_str, "control", "pause", "set"] => {
            let side = id_str.parse::<u8>().ok()?;
            let paused = match payload.to_lowercase().as_str() {
                "true" | "1" | "on" | "pause" => true,
                "false" | "0" | "off" | "resume" => false,
                _ => return None,
            };
            Some(MqttCommand::SetPause { side, paused })
        }
        ["connector", id_str, "control", "setpoint_amps", "set"] => {
            let side = id_str.parse::<u8>().ok()?;
            let amps = payload.trim().parse::<f64>().ok()?;
            Some(MqttCommand::SetCurrentLimit { side, amps })
        }
        ["station", "voltage", "set"] => {
            parse_voltage_payload(payload).map(|(v, ttl)| MqttCommand::SetVoltage {
                side: None,
                voltage: v,
                ttl_seconds: ttl,
            })
        }
        ["connector", id_str, "meter", "voltage_v", "set"] => {
            let side = id_str.parse::<u8>().ok()?;
            parse_voltage_payload(payload).map(|(v, ttl)| MqttCommand::SetVoltage {
                side: Some(side),
                voltage: v,
                ttl_seconds: ttl,
            })
        }
        _ => None,
    }
}

/// Helper parsing either raw numeric float string or JSON payload `{"voltage": 235.0, "ttl_seconds": 60}`.
fn parse_voltage_payload(raw: &str) -> Option<(f32, Option<u64>)> {
    let raw = raw.trim();
    if raw.starts_with('{') {
        #[derive(serde::Deserialize)]
        struct VoltageJson {
            voltage: f32,
            ttl_seconds: Option<u64>,
        }
        if let Ok(parsed) = serde_json::from_str::<VoltageJson>(raw) {
            return Some((parsed.voltage, parsed.ttl_seconds));
        }
    }

    if let Ok(v) = raw.parse::<f32>() {
        return Some((v, None));
    }

    None
}

/// Generates all unitary scalar MQTT topics and their string values for a given station snapshot.
pub fn generate_telemetry_topics(
    base_topic: &str,
    snapshot: &StationSnapshot,
) -> Vec<(String, String)> {
    let mut topics = Vec::with_capacity(64);
    let base = base_topic.trim_end_matches('/');

    // 1. Station-level topics
    topics.push((
        format!("{}/station/model", base),
        snapshot.station.model.clone(),
    ));
    topics.push((
        format!("{}/station/serial_number", base),
        snapshot.station.serial_number.clone(),
    ));
    topics.push((
        format!("{}/station/firmware_version", base),
        snapshot.station.firmware_version.clone(),
    ));
    topics.push((
        format!("{}/station/hardware_version", base),
        snapshot.station.hardware_version.clone(),
    ));
    topics.push((
        format!("{}/station/link_version", base),
        snapshot.station.link_version.clone(),
    ));
    topics.push((
        format!("{}/station/mac_address", base),
        snapshot.station.mac_address.clone().unwrap_or_default(),
    ));
    topics.push((
        format!("{}/station/is_dual", base),
        snapshot.station.is_dual.to_string(),
    ));
    topics.push((
        format!("{}/station/available_connectors", base),
        snapshot
            .station
            .available_connectors
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(","),
    ));

    let serial_status_str = match snapshot.station.serial_connection.status {
        greenup_driver::SerialStatus::UsbConnected => "usb_connected",
        greenup_driver::SerialStatus::UsbDisconnected => "usb_disconnected",
        greenup_driver::SerialStatus::UsbConnecting => "usb_connecting",
        greenup_driver::SerialStatus::UsbNoResponse => "usb_no_response",
    };
    topics.push((
        format!("{}/station/serial/status", base),
        serial_status_str.to_string(),
    ));
    topics.push((
        format!("{}/station/serial/port", base),
        snapshot.station.serial_connection.port.clone(),
    ));
    topics.push((
        format!("{}/station/serial/sniffer_mode", base),
        snapshot.station.serial_connection.sniffer_mode.to_string(),
    ));

    let total_power_w: f32 = snapshot.connectors.iter().map(|c| c.meter.power_w).sum();
    topics.push((
        format!("{}/station/total_active_power_w", base),
        format!("{:.1}", total_power_w),
    ));

    let station_voltage = snapshot
        .connectors
        .first()
        .map(|c| c.meter.voltage_v)
        .unwrap_or(230.0);
    topics.push((
        format!("{}/station/voltage_v", base),
        format!("{:.1}", station_voltage),
    ));

    let voltage_source = snapshot
        .connectors
        .first()
        .map(|c| c.meter.voltage_source.as_str())
        .unwrap_or("default");
    topics.push((
        format!("{}/station/voltage_source", base),
        voltage_source.to_string(),
    ));

    // 2. Connector-level topics
    for conn in &snapshot.connectors {
        let prefix = format!("{}/connector/{}", base, conn.id);

        // Status
        topics.push((
            format!("{}/status/state", prefix),
            conn.status.iec.clone(),
        ));
        let ocpp_str = match conn.status.ocpp {
            crate::telemetry::OcppChargePointStatus::Available => "Available",
            crate::telemetry::OcppChargePointStatus::Preparing => "Preparing",
            crate::telemetry::OcppChargePointStatus::Charging => "Charging",
            crate::telemetry::OcppChargePointStatus::SuspendedEVSE => "SuspendedEVSE",
            crate::telemetry::OcppChargePointStatus::SuspendedEV => "SuspendedEV",
            crate::telemetry::OcppChargePointStatus::Faulted => "Faulted",
            crate::telemetry::OcppChargePointStatus::Unavailable => "Unavailable",
        };
        topics.push((
            format!("{}/status/ocpp_status", prefix),
            ocpp_str.to_string(),
        ));
        topics.push((
            format!("{}/status/raw_code", prefix),
            conn.status.raw_code.clone(),
        ));
        topics.push((
            format!("{}/status/vehicle_connected", prefix),
            conn.status.vehicle_connected.to_string(),
        ));
        topics.push((
            format!("{}/status/charging", prefix),
            conn.status.charging.to_string(),
        ));
        topics.push((
            format!("{}/status/paused", prefix),
            conn.status.paused.to_string(),
        ));
        topics.push((
            format!("{}/status/charge_complete", prefix),
            conn.status.charge_complete.to_string(),
        ));
        topics.push((
            format!("{}/status/error_code", prefix),
            conn.status.error_code.clone(),
        ));

        // Meter
        topics.push((
            format!("{}/meter/active_power_w", prefix),
            format!("{:.1}", conn.meter.power_w),
        ));
        topics.push((
            format!("{}/meter/energy_total_kwh", prefix),
            format!("{:.3}", conn.meter.energy_kwh),
        ));
        topics.push((
            format!("{}/meter/energy_total_wh", prefix),
            format!("{:.1}", conn.meter.energy_total_wh),
        ));
        topics.push((
            format!("{}/meter/voltage_v", prefix),
            format!("{:.1}", conn.meter.voltage_v),
        ));
        topics.push((
            format!("{}/meter/current_amps", prefix),
            format!("{:.2}", conn.meter.current_a),
        ));
        topics.push((
            format!("{}/meter/frequency_hz", prefix),
            format!("{:.1}", conn.meter.frequency_hz),
        ));

        // Control
        let charge_payload = if conn.control.charge_authorized {
            "ON"
        } else {
            "OFF"
        };
        topics.push((
            format!("{}/control/charge", prefix),
            charge_payload.to_string(),
        ));
        topics.push((
            format!("{}/control/pause", prefix),
            conn.status.paused.to_string(),
        ));
        topics.push((
            format!("{}/control/setpoint_amps", prefix),
            conn.control.setpoint_amps.to_string(),
        ));
        topics.push((
            format!("{}/control/applied_limit_amps", prefix),
            conn.control.applied_limit_amps.to_string(),
        ));
        topics.push((
            format!("{}/control/min_allowed_amps", prefix),
            conn.control.min_allowed_amps.to_string(),
        ));
        topics.push((
            format!("{}/control/max_hardware_amps", prefix),
            conn.control.max_hardware_amps.to_string(),
        ));

        // Grid (Linky TIC)
        topics.push((
            format!("{}/grid/tic_connected", prefix),
            conn.grid.tic_connected.to_string(),
        ));
        topics.push((
            format!("{}/grid/tic_mode", prefix),
            conn.grid.tic_mode.clone(),
        ));

        // Session
        let (active, duration, energy_wh) = match crate::session::get_active_session_id_for_side(conn.id) {
            Some(_) => {
                let is_act = crate::session::is_active_for_side(conn.id);
                // Try to get live summary if possible
                let mut dummy_tel = crate::telemetry::Telemetry::default();
                dummy_tel.side = conn.id;
                if let Some(summary) = crate::session::get_active_session_summary_for_side(conn.id, &dummy_tel) {
                    (true, summary.elapsed_seconds, (summary.total_energy_kwh * 1000.0) as f64)
                } else {
                    (is_act, 0, 0.0)
                }
            }
            None => (false, 0, 0.0),
        };

        topics.push((
            format!("{}/session/active", prefix),
            active.to_string(),
        ));
        topics.push((
            format!("{}/session/duration_seconds", prefix),
            duration.to_string(),
        ));
        topics.push((
            format!("{}/session/energy_wh", prefix),
            format!("{:.1}", energy_wh),
        ));
    }

    topics
}

/// Helper creating a Home Assistant discovery entity config.
pub fn generate_ha_discovery_payloads(
    base_topic: &str,
    snapshot: &StationSnapshot,
) -> Vec<(String, String)> {
    let mut configs = Vec::with_capacity(32);
    let base = base_topic.trim_end_matches('/');

    let serial_raw = if !snapshot.station.serial_number.is_empty() {
        &snapshot.station.serial_number
    } else if !snapshot.station.model.is_empty() {
        &snapshot.station.model
    } else {
        "station"
    };
    let clean_serial = sanitize_identifier(serial_raw);
    let model = if snapshot.station.model.is_empty() {
        "Legrand Green'up"
    } else {
        &snapshot.station.model
    };
    let link_ver = &snapshot.station.link_version;
    let fw_ver = &snapshot.station.firmware_version;
    let hw_ver = &snapshot.station.hardware_version;

    let device_obj = serde_json::json!({
        "identifiers": [format!("greenup_{}", clean_serial)],
        "name": format!("Legrand Green'up ({})", model),
        "manufacturer": "Legrand",
        "model": model,
        "sw_version": format!("{} (FW {})", link_ver, fw_ver),
        "hw_version": hw_ver,
    });

    let availability_topic = format!("{}/availability", base);

    // Common station entities
    // 1. Station Total Active Power
    let topic_power = format!("homeassistant/sensor/greenup_{}/station_total_power/config", clean_serial);
    let payload_power = serde_json::json!({
        "name": "Station Total Power",
        "unique_id": format!("{}_station_total_power", clean_serial),
        "state_topic": format!("{}/station/total_active_power_w", base),
        "unit_of_measurement": "W",
        "device_class": "power",
        "state_class": "measurement",
        "availability_topic": availability_topic,
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": device_obj,
    });
    configs.push((topic_power, payload_power.to_string()));

    // 2. Station Grid Voltage Injection Number
    let topic_voltage = format!("homeassistant/number/greenup_{}/station_voltage/config", clean_serial);
    let payload_voltage = serde_json::json!({
        "name": "Grid Voltage Injection",
        "unique_id": format!("{}_station_voltage", clean_serial),
        "state_topic": format!("{}/station/voltage_v", base),
        "command_topic": format!("{}/station/voltage/set", base),
        "min": 100.0,
        "max": 300.0,
        "step": 0.5,
        "unit_of_measurement": "V",
        "mode": "box",
        "icon": "mdi:flash-triangle",
        "availability_topic": availability_topic,
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": device_obj,
    });
    configs.push((topic_voltage, payload_voltage.to_string()));

    // 3. Station Sniffer Mode Binary Sensor
    let topic_sniffer = format!("homeassistant/binary_sensor/greenup_{}/station_sniffer_mode/config", clean_serial);
    let payload_sniffer = serde_json::json!({
        "name": "Station Sniffer Mode",
        "unique_id": format!("{}_station_sniffer_mode", clean_serial),
        "state_topic": format!("{}/station/serial/sniffer_mode", base),
        "payload_on": "true",
        "payload_off": "false",
        "icon": "mdi:ear-hearing",
        "availability_topic": availability_topic,
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": device_obj,
    });
    configs.push((topic_sniffer, payload_sniffer.to_string()));

    // Entities per Connector
    for conn in &snapshot.connectors {
        let id = conn.id;
        let prefix = format!("{}/connector/{}", base, id);
        let point_label = if snapshot.station.is_dual {
            format!("Charge Point {}", id)
        } else {
            "Charge Point".to_string()
        };

        // Active Power Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_power/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Power", point_label),
            "unique_id": format!("{}_point_{}_power", clean_serial, id),
            "state_topic": format!("{}/meter/active_power_w", prefix),
            "unit_of_measurement": "W",
            "device_class": "power",
            "state_class": "measurement",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Total Energy Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_total_energy/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Total Energy", point_label),
            "unique_id": format!("{}_point_{}_total_energy", clean_serial, id),
            "state_topic": format!("{}/meter/energy_total_kwh", prefix),
            "unit_of_measurement": "kWh",
            "device_class": "energy",
            "state_class": "total_increasing",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Current Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_current/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Current", point_label),
            "unique_id": format!("{}_point_{}_current", clean_serial, id),
            "state_topic": format!("{}/meter/current_amps", prefix),
            "unit_of_measurement": "A",
            "device_class": "current",
            "state_class": "measurement",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Voltage Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_voltage/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Voltage", point_label),
            "unique_id": format!("{}_point_{}_voltage", clean_serial, id),
            "state_topic": format!("{}/meter/voltage_v", prefix),
            "unit_of_measurement": "V",
            "device_class": "voltage",
            "state_class": "measurement",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // IEC State Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_state/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} IEC State", point_label),
            "unique_id": format!("{}_point_{}_state", clean_serial, id),
            "state_topic": format!("{}/status/state", prefix),
            "icon": "mdi:ev-station",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // OCPP Status Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_ocpp/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} OCPP Status", point_label),
            "unique_id": format!("{}_point_{}_ocpp", clean_serial, id),
            "state_topic": format!("{}/status/ocpp_status", prefix),
            "icon": "mdi:ev-station",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Applied Limit Sensor
        let t = format!("homeassistant/sensor/greenup_{}/point_{}_applied_limit/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Applied Limit", point_label),
            "unique_id": format!("{}_point_{}_applied_limit", clean_serial, id),
            "state_topic": format!("{}/control/applied_limit_amps", prefix),
            "unit_of_measurement": "A",
            "device_class": "current",
            "icon": "mdi:speedometer",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Vehicle Connected Binary Sensor
        let t = format!("homeassistant/binary_sensor/greenup_{}/point_{}_connected/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Vehicle Connected", point_label),
            "unique_id": format!("{}_point_{}_connected", clean_serial, id),
            "state_topic": format!("{}/status/vehicle_connected", prefix),
            "payload_on": "true",
            "payload_off": "false",
            "device_class": "plug",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Charging Binary Sensor
        let t = format!("homeassistant/binary_sensor/greenup_{}/point_{}_charging/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Charging", point_label),
            "unique_id": format!("{}_point_{}_charging", clean_serial, id),
            "state_topic": format!("{}/status/charging", prefix),
            "payload_on": "true",
            "payload_off": "false",
            "device_class": "battery_charging",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Session Active Binary Sensor
        let t = format!("homeassistant/binary_sensor/greenup_{}/point_{}_session_active/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Session Active", point_label),
            "unique_id": format!("{}_point_{}_session_active", clean_serial, id),
            "state_topic": format!("{}/session/active", prefix),
            "payload_on": "true",
            "payload_off": "false",
            "icon": "mdi:progress-clock",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Charge Enable Switch
        let t = format!("homeassistant/switch/greenup_{}/point_{}_charge/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Charge", point_label),
            "unique_id": format!("{}_point_{}_charge", clean_serial, id),
            "state_topic": format!("{}/control/charge", prefix),
            "command_topic": format!("{}/control/charge/set", prefix),
            "payload_on": "ON",
            "payload_off": "OFF",
            "icon": "mdi:ev-plug-type2",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Pause Switch
        let t = format!("homeassistant/switch/greenup_{}/point_{}_pause/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Pause", point_label),
            "unique_id": format!("{}_point_{}_pause", clean_serial, id),
            "state_topic": format!("{}/control/pause", prefix),
            "command_topic": format!("{}/control/pause/set", prefix),
            "payload_on": "true",
            "payload_off": "false",
            "icon": "mdi:pause-circle",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));

        // Current Setpoint Number
        let max_amps = conn.control.max_hardware_amps.max(32);
        let t = format!("homeassistant/number/greenup_{}/point_{}_setpoint/config", clean_serial, id);
        let p = serde_json::json!({
            "name": format!("{} Current Setpoint", point_label),
            "unique_id": format!("{}_point_{}_setpoint", clean_serial, id),
            "state_topic": format!("{}/control/setpoint_amps", prefix),
            "command_topic": format!("{}/control/setpoint_amps/set", prefix),
            "min": 7,
            "max": max_amps,
            "step": 1,
            "unit_of_measurement": "A",
            "mode": "slider",
            "icon": "mdi:current-ac",
            "availability_topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline",
            "device": device_obj,
        });
        configs.push((t, p.to_string()));
    }

    configs
}

/// Helper generating all Home Assistant discovery config topic paths for a given serial identifier,
/// used to unregister / purge stale discovery topics from the broker.
pub fn generate_ha_discovery_topics_to_clear(clean_serial: &str, connector_ids: &[u8]) -> Vec<String> {
    let mut topics = Vec::with_capacity(32);
    topics.push(format!("homeassistant/sensor/greenup_{}/station_total_power/config", clean_serial));
    topics.push(format!("homeassistant/number/greenup_{}/station_voltage/config", clean_serial));
    topics.push(format!("homeassistant/binary_sensor/greenup_{}/station_sniffer_mode/config", clean_serial));

    for &id in connector_ids {
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_power/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_total_energy/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_current/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_voltage/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_state/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_ocpp/config", clean_serial, id));
        topics.push(format!("homeassistant/sensor/greenup_{}/point_{}_applied_limit/config", clean_serial, id));
        topics.push(format!("homeassistant/binary_sensor/greenup_{}/point_{}_connected/config", clean_serial, id));
        topics.push(format!("homeassistant/binary_sensor/greenup_{}/point_{}_charging/config", clean_serial, id));
        topics.push(format!("homeassistant/binary_sensor/greenup_{}/point_{}_session_active/config", clean_serial, id));
        topics.push(format!("homeassistant/switch/greenup_{}/point_{}_charge/config", clean_serial, id));
        topics.push(format!("homeassistant/switch/greenup_{}/point_{}_pause/config", clean_serial, id));
        topics.push(format!("homeassistant/number/greenup_{}/point_{}_setpoint/config", clean_serial, id));
    }
    topics
}

/// Executes an incoming MQTT command on the appropriate charge point or station.
async fn handle_mqtt_command(station: &SharedStation, command: MqttCommand) {
    match command {
        MqttCommand::SetCharge { side, action } => {
            let point = station.get_point(Some(side));
            if point.sniffer_mode() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT charge action on Side {}: sniffer mode active",
                        side
                    ),
                );
                return;
            }
            if !point.is_serial_connected() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT charge action on Side {}: USB serial link disconnected",
                        side
                    ),
                );
                return;
            }

            match action.to_lowercase().as_str() {
                "enable" | "resume" | "on" | "1" | "true" => {
                    logger::log("MQTT", &format!("⚡ Point {}: Charge enable requested via MQTT", side));
                    let _ = point.serial_tx.send(Command::AuthorizeType2(true)).await;
                    let _ = point.serial_tx.send(Command::SetExternalSignal(false)).await;
                    let _ = point.serial_tx.send(Command::SetDerogation(true)).await;
                }
                "disable" | "off" | "0" | "false" => {
                    logger::log("MQTT", &format!("🛑 Point {}: Charge disable requested via MQTT", side));
                    let _ = point.serial_tx.send(Command::SetExternalSignal(true)).await;
                    let _ = point.serial_tx.send(Command::SetDerogation(false)).await;
                }
                "pause" => {
                    logger::log("MQTT", &format!("⏸ Point {}: Charge pause requested via MQTT", side));
                    let _ = point.serial_tx.send(Command::SetStartButton(false)).await;
                }
                other => {
                    logger::warn("MQTT", &format!("⚠️ Point {}: Unknown charge action '{}' via MQTT", side, other));
                }
            }
        }
        MqttCommand::SetPause { side, paused } => {
            let point = station.get_point(Some(side));
            if point.sniffer_mode() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT pause action on Side {}: sniffer mode active",
                        side
                    ),
                );
                return;
            }
            if !point.is_serial_connected() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT pause action on Side {}: USB serial link disconnected",
                        side
                    ),
                );
                return;
            }

            if paused {
                logger::log("MQTT", &format!("⏸ Point {}: Pause requested via MQTT", side));
                let _ = point.serial_tx.send(Command::SetStartButton(false)).await;
            } else {
                logger::log("MQTT", &format!("▶ Point {}: Resume requested via MQTT", side));
                let _ = point.serial_tx.send(Command::SetStartButton(true)).await;
            }
        }
        MqttCommand::SetCurrentLimit { side, amps } => {
            let point = station.get_point(Some(side));
            if point.sniffer_mode() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT current limit on Side {}: sniffer mode active",
                        side
                    ),
                );
                return;
            }
            if !point.is_serial_connected() {
                logger::warn(
                    "MQTT",
                    &format!(
                        "⛔ Rejecting MQTT current limit on Side {}: USB serial link disconnected",
                        side
                    ),
                );
                return;
            }

            let rounded = amps.round();
            let validated_amps = if rounded.is_finite() && (0.0..=255.0).contains(&rounded) {
                let a = rounded as u8;
                Command::set_current_limit(a).ok().map(|_| a)
            } else {
                None
            };

            let validated_amps = match validated_amps {
                Some(a) => a,
                None => {
                    logger::warn(
                        "MQTT",
                        &format!(
                            "⛔ Rejecting out of bounds current: {:.2}A (allowed {}-{}A)",
                            amps, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS
                        ),
                    );
                    return;
                }
            };

            if (amps - validated_amps as f64).abs() > 0.01 {
                logger::debug(
                    "MQTT",
                    &format!("⚡ MQTT current limit {:.2}A rounded to {}A on Side {}", amps, validated_amps, side),
                );
            }

            point.sync_from_driver();
            let current_greenup_state = point
                .telemetry
                .lock()
                .map(|t| t.greenup_state.clone())
                .unwrap_or_else(|_| "Unknown".to_string());

            if current_greenup_state == "A" {
                logger::debug(
                    "MQTT",
                    &format!(
                        "⚡ Current limit modification in State A on Side {} → {}A (synchronizing CCEl:{:02} and CC:{:02})",
                        side, validated_amps, validated_amps, validated_amps
                    ),
                );
                let _ = point.serial_tx.send(Command::SetCurrentLimit(validated_amps)).await;
                let _ = point.serial_tx.send(Command::SetDirectCurrentLimit(validated_amps)).await;
            } else {
                logger::debug(
                    "MQTT",
                    &format!(
                        "⚡ Current limit modification on Side {} → {}A (sending CCEl:{:02})",
                        side, validated_amps, validated_amps
                    ),
                );
                let _ = point.serial_tx.send(Command::SetCurrentLimit(validated_amps)).await;
            }
        }
        MqttCommand::SetVoltage { side, voltage, ttl_seconds } => {
            if voltage < 100.0 || voltage > 300.0 {
                logger::warn(
                    "MQTT",
                    &format!("⛔ Rejecting out of bounds external voltage via MQTT: {:.1}V", voltage),
                );
                return;
            }

            let ttl = ttl_seconds.unwrap_or(60);
            logger::debug(
                "MQTT",
                &format!("⚡ External grid voltage updated via MQTT → {:.1}V (TTL: {}s)", voltage, ttl),
            );

            // Apply to Point 1 if target is side 1 or global
            if side.is_none() || side == Some(1) {
                if let Ok(mut tel) = station.point1.telemetry.lock() {
                    tel.set_external_voltage(voltage, ttl);
                }
            }

            // Apply to Point 2 if dual station and target is side 2 or global
            if side.is_none() || side == Some(2) {
                if let Some(ref p2) = *station.point2.read().unwrap_or_else(|e| e.into_inner()) {
                    if let Ok(mut tel) = p2.telemetry.lock() {
                        tel.set_external_voltage(voltage, ttl);
                    }
                }
            }
        }
    }
}

/// Spawns the autonomous background MQTT service.
pub fn spawn_mqtt_service(
    station: SharedStation,
    config: MqttConfig,
    api_token: Option<String>,
) {
    tokio::spawn(async move {
        run_mqtt_loop(station, config, api_token).await;
    });
}

/// Main MQTT client loop.
async fn run_mqtt_loop(
    station: SharedStation,
    config: MqttConfig,
    api_token: Option<String>,
) {
    let (host, port) = parse_broker_address(&config.broker);
    let base_topic = config.base_topic.trim_matches('/').to_string();
    let credentials = resolve_credentials(
        config.username.as_deref(),
        config.password.as_deref(),
        api_token.as_deref(),
    );

    let client_id = format!("greenup-link-{}", std::process::id());
    let mut mqttoptions = MqttOptions::new(&client_id, &host, port);
    mqttoptions.set_keep_alive(Duration::from_secs(15));
    mqttoptions.set_clean_session(true);

    let availability_topic = format!("{}/availability", base_topic);
    let will = LastWill::new(
        availability_topic.clone(),
        "offline",
        QoS::AtLeastOnce,
        true,
    );
    mqttoptions.set_last_will(will);

    if let Some((ref user, ref pass)) = credentials {
        mqttoptions.set_credentials(user, pass);
        logger::log(
            "MQTT",
            &format!(
                "🔑 Authenticating to MQTT broker '{}:{}' as user '{}'",
                host, port, user
            ),
        );
    } else {
        logger::log(
            "MQTT",
            &format!("Connecting to MQTT broker '{}:{}' (anonymous)", host, port),
        );
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 256);

    let is_connected = Arc::new(AtomicBool::new(false));
    let is_connected_el = is_connected.clone();
    let client_el = client.clone();
    let base_topic_el = base_topic.clone();
    let availability_topic_el = availability_topic.clone();
    let station_el = station.clone();
    let host_el = host.clone();
    let (reconnect_tx, mut reconnect_rx) = tokio::sync::mpsc::channel::<()>(4);

    // Dedicated asynchronous task running the MQTT event loop (network I/O, incoming packets & keepalives).
    // Decoupled from the telemetry publisher task so outbound publishes never deadlock internal channels.
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_ack))) => {
                    let was_connected = is_connected_el.swap(true, Ordering::SeqCst);
                    if !was_connected {
                        logger::log(
                            "MQTT",
                            &format!("✅ Connected to MQTT broker '{}:{}'", host_el, port),
                        );
                    }

                    // Publish availability: online
                    let _ = client_el
                        .publish(&availability_topic_el, QoS::AtLeastOnce, true, "online")
                        .await;

                    // Subscribe to incoming command topics
                    let cmd_patterns = [
                        format!("{}/connector/+/control/charge/set", base_topic_el),
                        format!("{}/connector/+/control/pause/set", base_topic_el),
                        format!("{}/connector/+/control/setpoint_amps/set", base_topic_el),
                        format!("{}/station/voltage/set", base_topic_el),
                        format!("{}/connector/+/meter/voltage_v/set", base_topic_el),
                    ];
                    for pattern in &cmd_patterns {
                        if let Err(e) = client_el.subscribe(pattern, QoS::AtLeastOnce).await {
                            logger::warn("MQTT", &format!("Failed to subscribe to '{}': {}", pattern, e));
                        }
                    }

                    let _ = reconnect_tx.try_send(());
                }
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    let topic = &publish.topic;
                    let payload_str = String::from_utf8_lossy(&publish.payload);
                    logger::debug("MQTT", &format!("📩 Incoming message on '{}': {}", topic, payload_str.trim()));

                    if let Some(cmd) = parse_command_topic(&base_topic_el, topic, &payload_str) {
                        handle_mqtt_command(&station_el, cmd).await;
                    }
                }
                Ok(Event::Incoming(Packet::Disconnect)) => {
                    is_connected_el.store(false, Ordering::SeqCst);
                    logger::warn("MQTT", "⚠️ Disconnected from MQTT broker");
                }
                Err(e) => {
                    let was_connected = is_connected_el.swap(false, Ordering::SeqCst);
                    if was_connected {
                        logger::warn("MQTT", &format!("⚠️ MQTT connection error: {}. Reconnecting...", e));
                    }
                    sleep(Duration::from_secs(2)).await;
                }
                _ => {}
            }
        }
    });

    let mut last_published_topics: HashMap<String, String> = HashMap::new();
    let mut last_periodic_refresh = Instant::now();
    let mut last_ha_discovery_serial: Option<String> = None;

    // Periodic telemetry and Home Assistant discovery evaluation ticker
    let mut poll_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        poll_interval.tick().await;

        if reconnect_rx.try_recv().is_ok() {
            last_published_topics.clear();
            last_ha_discovery_serial = None;
            last_periodic_refresh = Instant::now();
        }

        if !is_connected.load(Ordering::SeqCst) {
            continue;
        }

        let snapshot = station.snapshot();

        // 1. Home Assistant Auto-Discovery
        if config.ha_discovery {
            let station_id_raw = if !snapshot.station.serial_number.is_empty() {
                snapshot.station.serial_number.as_str()
            } else if !snapshot.station.model.is_empty() {
                snapshot.station.model.as_str()
            } else {
                ""
            };

            // Only publish discovery once hardware metadata has been identified from ATmega
            if !station_id_raw.is_empty() {
                let clean_serial = sanitize_identifier(station_id_raw);

                if last_ha_discovery_serial.as_deref() != Some(&clean_serial) {
                    let conn_ids = [1, 2];

                    // Purge legacy generic / fallback discovery topics retained on the broker
                    for dummy in &["greenup_station", "station", "default_serial"] {
                        for topic in generate_ha_discovery_topics_to_clear(dummy, &conn_ids) {
                            let _ = client.publish(topic, QoS::AtLeastOnce, true, Vec::new()).await;
                        }
                    }

                    // Also purge previous clean serial if it changed
                    if let Some(ref old_serial) = last_ha_discovery_serial {
                        for topic in generate_ha_discovery_topics_to_clear(old_serial, &conn_ids) {
                            let _ = client.publish(topic, QoS::AtLeastOnce, true, Vec::new()).await;
                        }
                    }

                    logger::log(
                        "MQTT",
                        &format!(
                            "📢 Publishing Home Assistant MQTT Discovery entities for station '{}'",
                            clean_serial
                        ),
                    );
                    let discovery_payloads = generate_ha_discovery_payloads(&base_topic, &snapshot);
                    for (topic, payload) in discovery_payloads {
                        if let Err(e) = client
                            .publish(&topic, QoS::AtLeastOnce, true, payload.into_bytes())
                            .await
                        {
                            logger::warn("MQTT", &format!("Failed to publish discovery topic '{}': {}", topic, e));
                        }
                    }
                    last_ha_discovery_serial = Some(clean_serial);
                }
            }
        }

        // 2. Periodic telemetry evaluation (Report-by-Exception + 60s full refresh)
        let now = Instant::now();
        let force_refresh = now.duration_since(last_periodic_refresh) >= Duration::from_secs(60);
        if force_refresh {
            last_periodic_refresh = now;
        }

        let topics = generate_telemetry_topics(&base_topic, &snapshot);
        for (topic, value) in topics {
            let should_publish = force_refresh || match last_published_topics.get(&topic) {
                Some(prev) => prev != &value,
                None => true,
            };

            if should_publish {
                let _ = client
                    .publish(&topic, QoS::AtLeastOnce, true, value.clone().into_bytes())
                    .await;
                last_published_topics.insert(topic, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::*;

    #[test]
    fn test_parse_broker_address() {
        assert_eq!(parse_broker_address("127.0.0.1:1883"), ("127.0.0.1".to_string(), 1883));
        assert_eq!(parse_broker_address("mqtt.local"), ("mqtt.local".to_string(), 1883));
        assert_eq!(parse_broker_address("192.168.1.100:8883"), ("192.168.1.100".to_string(), 8883));
        assert_eq!(parse_broker_address("  broker.lan:1884  "), ("broker.lan".to_string(), 1884));
    }

    #[test]
    fn test_resolve_credentials() {
        assert_eq!(
            resolve_credentials(Some("admin"), Some("secret123"), None),
            Some(("admin".to_string(), "secret123".to_string()))
        );
        assert_eq!(
            resolve_credentials(Some("admin"), None, Some("fallback")),
            Some(("admin".to_string(), "fallback".to_string()))
        );
        assert_eq!(
            resolve_credentials(None, Some("secret123"), None),
            Some(("greenup-link".to_string(), "secret123".to_string()))
        );
        assert_eq!(
            resolve_credentials(Some("admin"), None, None),
            Some(("admin".to_string(), String::new()))
        );
        assert_eq!(resolve_credentials(None, None, None), None);
        assert_eq!(resolve_credentials(Some(""), Some(""), Some("")), None);
    }

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("0123456789"), "0123456789");
        assert_eq!(sanitize_identifier("059001/A"), "059001_A");
        assert_eq!(sanitize_identifier("   "), "station");
    }

    #[test]
    fn test_parse_command_topic() {
        let base = "greenup";

        // Charge control
        let c1 = parse_command_topic(base, "greenup/connector/1/control/charge/set", "enable");
        assert_eq!(c1, Some(MqttCommand::SetCharge { side: 1, action: "enable".to_string() }));

        let c2 = parse_command_topic(base, "greenup/connector/2/control/charge/set", "OFF");
        assert_eq!(c2, Some(MqttCommand::SetCharge { side: 2, action: "OFF".to_string() }));

        // Pause control
        let p1 = parse_command_topic(base, "greenup/connector/1/control/pause/set", "true");
        assert_eq!(p1, Some(MqttCommand::SetPause { side: 1, paused: true }));

        let p2 = parse_command_topic(base, "greenup/connector/1/control/pause/set", "0");
        assert_eq!(p2, Some(MqttCommand::SetPause { side: 1, paused: false }));

        // Setpoint current limit (integer and decimal)
        let s1 = parse_command_topic(base, "greenup/connector/1/control/setpoint_amps/set", "16");
        assert_eq!(s1, Some(MqttCommand::SetCurrentLimit { side: 1, amps: 16.0 }));

        let s2 = parse_command_topic(base, "greenup/connector/1/control/setpoint_amps/set", "16.7");
        assert_eq!(s2, Some(MqttCommand::SetCurrentLimit { side: 1, amps: 16.7 }));

        // Station voltage
        let v1 = parse_command_topic(base, "greenup/station/voltage/set", "234.5");
        assert_eq!(v1, Some(MqttCommand::SetVoltage { side: None, voltage: 234.5, ttl_seconds: None }));

        let v2 = parse_command_topic(base, "greenup/station/voltage/set", r#"{"voltage": 238.0, "ttl_seconds": 120}"#);
        assert_eq!(v2, Some(MqttCommand::SetVoltage { side: None, voltage: 238.0, ttl_seconds: Some(120) }));

        // Connector voltage
        let v3 = parse_command_topic(base, "greenup/connector/2/meter/voltage_v/set", "230.0");
        assert_eq!(v3, Some(MqttCommand::SetVoltage { side: Some(2), voltage: 230.0, ttl_seconds: None }));

        // Unrelated topic
        let u1 = parse_command_topic(base, "other/topic", "value");
        assert_eq!(u1, None);
    }

    #[test]
    fn test_generate_telemetry_topics() {
        let snap = StationSnapshot {
            station: StationData {
                vendor: "Legrand".to_string(),
                model: "059001".to_string(),
                serial_number: "TEST1234".to_string(),
                hardware_version: "V1.0".to_string(),
                firmware_version: "1.19.02".to_string(),
                link_version: "0.5.0".to_string(),
                serial_connection: SerialConnection {
                    status: SerialStatus::UsbConnected,
                    port: "/dev/ttyUSB0".to_string(),
                    sniffer_mode: false,
                    message: "OK".to_string(),
                    last_rx_secs_ago: Some(1),
                },
                is_dual: false,
                available_connectors: vec![1],
                production_date: Some("2021-W12".to_string()),
                bluetooth_enabled: Some(true),
                mac_address: Some("AA:BB:CC:DD:EE:FF".to_string()),
                capabilities: None,
            },
            connectors: vec![
                ConnectorData {
                    id: 1,
                    status: ConnectorStatus {
                        ocpp: OcppChargePointStatus::Available,
                        iec: "A".to_string(),
                        raw_code: "A".to_string(),
                        vehicle_connected: false,
                        charging: false,
                        paused: false,
                        charge_complete: false,
                        error_code: String::new(),
                        error_description: None,
                    },
                    control: ConnectorControl {
                        setpoint_amps: 16,
                        applied_limit_amps: 16,
                        min_allowed_amps: 7,
                        max_hardware_amps: 32,
                        charge_authorized: true,
                        pilot_voltage: None,
                    },
                    meter: ConnectorMeter {
                        power_w: 0.0,
                        energy_kwh: 12.345,
                        energy_total_wh: 12345.0,
                        current_a: 0.0,
                        voltage_v: 230.0,
                        voltage_source: "default".to_string(),
                        voltage_ttl_remaining_s: None,
                        frequency_hz: 50.0,
                    },
                    grid: ConnectorGrid {
                        tic_connected: false,
                        tic_mode: "none".to_string(),
                        tic_baud_rate: None,
                    },
                },
            ],
        };

        let topics = generate_telemetry_topics("greenup", &snap);
        let topic_map: HashMap<String, String> = topics.into_iter().collect();

        assert_eq!(topic_map.get("greenup/station/model").map(String::as_str), Some("059001"));
        assert_eq!(topic_map.get("greenup/station/serial_number").map(String::as_str), Some("TEST1234"));
        assert_eq!(topic_map.get("greenup/station/serial/status").map(String::as_str), Some("usb_connected"));
        assert_eq!(topic_map.get("greenup/station/serial/sniffer_mode").map(String::as_str), Some("false"));
        assert_eq!(topic_map.get("greenup/station/voltage_v").map(String::as_str), Some("230.0"));

        assert_eq!(topic_map.get("greenup/connector/1/status/state").map(String::as_str), Some("A"));
        assert_eq!(topic_map.get("greenup/connector/1/status/ocpp_status").map(String::as_str), Some("Available"));
        assert_eq!(topic_map.get("greenup/connector/1/meter/energy_total_kwh").map(String::as_str), Some("12.345"));
        assert_eq!(topic_map.get("greenup/connector/1/control/setpoint_amps").map(String::as_str), Some("16"));
        assert_eq!(topic_map.get("greenup/connector/1/control/applied_limit_amps").map(String::as_str), Some("16"));
        assert_eq!(topic_map.get("greenup/connector/1/control/charge").map(String::as_str), Some("ON"));
    }

    #[test]
    fn test_generate_ha_discovery_payloads() {
        let snap = StationSnapshot {
            station: StationData {
                vendor: "Legrand".to_string(),
                model: "059001".to_string(),
                serial_number: "059001A123".to_string(),
                hardware_version: "V1.0".to_string(),
                firmware_version: "1.19.02".to_string(),
                link_version: "0.5.0".to_string(),
                serial_connection: SerialConnection::default(),
                is_dual: false,
                available_connectors: vec![1],
                production_date: None,
                bluetooth_enabled: None,
                mac_address: None,
                capabilities: None,
            },
            connectors: vec![
                ConnectorData {
                    id: 1,
                    status: ConnectorStatus {
                        ocpp: OcppChargePointStatus::Available,
                        iec: "A".to_string(),
                        raw_code: "A".to_string(),
                        vehicle_connected: false,
                        charging: false,
                        paused: false,
                        charge_complete: false,
                        error_code: String::new(),
                        error_description: None,
                    },
                    control: ConnectorControl {
                        setpoint_amps: 16,
                        applied_limit_amps: 16,
                        min_allowed_amps: 7,
                        max_hardware_amps: 32,
                        charge_authorized: true,
                        pilot_voltage: None,
                    },
                    meter: ConnectorMeter {
                        power_w: 0.0,
                        energy_kwh: 0.0,
                        energy_total_wh: 0.0,
                        current_a: 0.0,
                        voltage_v: 230.0,
                        voltage_source: "default".to_string(),
                        voltage_ttl_remaining_s: None,
                        frequency_hz: 50.0,
                    },
                    grid: ConnectorGrid {
                        tic_connected: false,
                        tic_mode: "none".to_string(),
                        tic_baud_rate: None,
                    },
                },
            ],
        };

        let configs = generate_ha_discovery_payloads("greenup", &snap);
        assert!(!configs.is_empty(), "Should generate HA discovery entities");

        // Verify power sensor config
        let power_cfg = configs.iter().find(|(topic, _)| topic.contains("point_1_power")).expect("Point 1 power config missing");
        assert!(power_cfg.0.starts_with("homeassistant/sensor/greenup_059001A123/"));
        assert!(power_cfg.1.contains("\"device_class\":\"power\""));
        assert!(power_cfg.1.contains("\"unit_of_measurement\":\"W\""));

        // Verify charge switch config
        let switch_cfg = configs.iter().find(|(topic, _)| topic.contains("point_1_charge")).expect("Point 1 charge switch missing");
        assert!(switch_cfg.0.starts_with("homeassistant/switch/greenup_059001A123/"));
        assert!(switch_cfg.1.contains("\"command_topic\":\"greenup/connector/1/control/charge/set\""));

        // Verify current setpoint number config
        let number_cfg = configs.iter().find(|(topic, _)| topic.contains("point_1_setpoint")).expect("Point 1 setpoint number missing");
        assert!(number_cfg.0.starts_with("homeassistant/number/greenup_059001A123/"));
        assert!(number_cfg.1.contains("\"min\":7"));
        assert!(number_cfg.1.contains("\"max\":32"));
    }

    #[test]
    fn test_generate_ha_discovery_payloads_fallback_to_model() {
        let snap = StationSnapshot {
            station: StationData {
                vendor: "Legrand".to_string(),
                model: "058001".to_string(),
                serial_number: String::new(),
                hardware_version: "V1.0".to_string(),
                firmware_version: "1.17.77".to_string(),
                link_version: "0.5.0".to_string(),
                serial_connection: SerialConnection::default(),
                is_dual: false,
                available_connectors: vec![1],
                production_date: None,
                bluetooth_enabled: None,
                mac_address: None,
                capabilities: None,
            },
            connectors: vec![
                ConnectorData {
                    id: 1,
                    status: ConnectorStatus {
                        ocpp: OcppChargePointStatus::Available,
                        iec: "A".to_string(),
                        raw_code: "A".to_string(),
                        vehicle_connected: false,
                        charging: false,
                        paused: false,
                        charge_complete: false,
                        error_code: String::new(),
                        error_description: None,
                    },
                    control: ConnectorControl {
                        setpoint_amps: 16,
                        applied_limit_amps: 16,
                        min_allowed_amps: 7,
                        max_hardware_amps: 32,
                        charge_authorized: true,
                        pilot_voltage: None,
                    },
                    meter: ConnectorMeter {
                        power_w: 0.0,
                        energy_kwh: 0.0,
                        energy_total_wh: 0.0,
                        current_a: 0.0,
                        voltage_v: 230.0,
                        voltage_source: "default".to_string(),
                        voltage_ttl_remaining_s: None,
                        frequency_hz: 50.0,
                    },
                    grid: ConnectorGrid {
                        tic_connected: false,
                        tic_mode: "none".to_string(),
                        tic_baud_rate: None,
                    },
                },
            ],
        };

        let configs = generate_ha_discovery_payloads("greenup", &snap);
        assert!(!configs.is_empty(), "Should generate HA discovery entities");

        let power_cfg = configs.iter().find(|(topic, _)| topic.contains("point_1_power")).expect("Point 1 power config missing");
        assert!(power_cfg.0.starts_with("homeassistant/sensor/greenup_058001/"));
        assert!(power_cfg.1.contains("\"identifiers\":[\"greenup_058001\"]"));
        assert!(power_cfg.1.contains("\"model\":\"058001\""));
    }
}
