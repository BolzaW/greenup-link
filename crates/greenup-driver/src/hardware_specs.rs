use serde::{Deserialize, Serialize};

/// Hardware specifications of the charging station, deduced from its reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    /// Commercial name of the charging station.
    pub name: String,
    /// Legrand reference (without spaces).
    pub reference: String,
    /// Number of phases (1 = Single-phase, 3 = Three-phase).
    pub phases: u8,
    /// Theoretical maximum power in kW (e.g. 4.6, 7.4, 22.0).
    pub max_power_kw: f32,
    /// Maximum current limit per phase in Amperes (e.g. 20, 32).
    pub max_current_amps: u8,
    /// Presence of a domestic socket (Schuko).
    pub has_schuko: bool,
    /// Number of charging points (1 or 2).
    pub charging_points: u8,
    /// Indicates if this reference is officially recognized (false if fallback).
    pub is_known: bool,
}

impl Default for HardwareCapabilities {
    /// Default charging station if the reference is unknown (Single-phase, 4.6kW, 1 T2, no Schuko).
    fn default() -> Self {
        Self {
            name: "Unknown Model (Default Fallback)".to_string(),
            reference: "UNKNOWN".to_string(),
            phases: 1,
            max_power_kw: 4.6,
            max_current_amps: 20,
            has_schuko: false,
            charging_points: 1,
            is_known: false,
        }
    }
}

impl HardwareCapabilities {
    /// Returns the hardware specifications from its Legrand reference (e.g. "058001").
    pub fn from_reference(reference: &str) -> Self {
        // Cleans the reference to remove potential spaces
        let clean_ref = reference.replace(" ", "");

        match clean_ref.as_str() {
            "058000" => Self::new("Green’up Premium 3,7–4,6 kW T2", "058000", 1, 4.6, 20, false, 1),
            "058001" => Self::new("Green’up Premium 3,7–7,4 kW T2", "058001", 1, 7.4, 32, false, 1),
            "058002" => Self::new("Green’up Premium 11–22 kW T2", "058002", 3, 22.0, 32, false, 1),
            
            "058003" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko", "058003", 1, 4.6, 20, true, 1),
            "058004" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko", "058004", 1, 7.4, 32, true, 1),
            
            "058010" => Self::new("Green’up Premium metal 3,7–4,6 kW 1 pt T2 + schuko", "058010", 1, 4.6, 20, true, 1),
            "058011" => Self::new("Green’up Premium metal 3,7–4,6 kW 2 pts T2 + schuko", "058011", 1, 4.6, 20, true, 2),
            "058012" => Self::new("Green’up Premium metal 3,7–7,4 kW 1 pt T2 + schuko", "058012", 1, 7.4, 32, true, 1),
            "058013" => Self::new("Green’up Premium metal 3,7–7,4 kW 2 pts T2 + schuko", "058013", 1, 7.4, 32, true, 2),
            "058014" => Self::new("Green’up Premium metal 11–22 kW 1 pt T2 + schuko", "058014", 3, 22.0, 32, true, 1),
            "058015" => Self::new("Green’up Premium metal 11–22 kW 2 pts T2 + schuko", "058015", 3, 22.0, 32, true, 2),
            
            "058030" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko ", "058030", 1, 4.6, 20, true, 1),
            "058035" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko ", "058035", 1, 7.4, 32, true, 1),
            
            "058041" => Self::new("Green’up Premium metal 3,7–4,6 kW 1 pt T2 + schuko ", "058041", 1, 4.6, 20, true, 1),
            "058042" => Self::new("Green’up Premium metal 3,7–4,6 kW 2 pts T2 + schuko ", "058042", 1, 4.6, 20, true, 2),
            "058043" => Self::new("Green’up Premium metal 3,7–7,4 kW 1 pt T2 + schuko ", "058043", 1, 7.4, 32, true, 1),
            "058048" => Self::new("Green’up Premium metal 11–22 kW 1 pt T2 + schuko ", "058048", 3, 22.0, 32, true, 1),
            "058049" => Self::new("Green’up Premium metal 11–22 kW 2 pts T2 + schuko ", "058049", 3, 22.0, 32, true, 2),
            
            "059005" => Self::new("Green’up Premium 3,7–4,6 kW T2 avec kit", "059005", 1, 4.6, 20, false, 1),
            "059006" => Self::new("Green’up Premium 3,7–7,4 kW T2 avec kit", "059006", 1, 7.4, 32, false, 1),
            "059007" => Self::new("Green’up Premium 11–22 kW T2 avec kit", "059007", 3, 22.0, 32, false, 1),
            
            "059008" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko avec kit", "059008", 1, 4.6, 20, true, 1),
            "059009" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko avec kit", "059009", 1, 7.4, 32, true, 1),
            
            "059070" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko (Sans Disjoncteur)", "059070", 1, 4.6, 20, true, 1),
            "059071" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko (Sans Disjoncteur)", "059071", 1, 7.4, 32, true, 1),

            _ => {
                let mut fallback = Self::default();
                // We still keep the reference sent by the charging station
                // (but with a complicated static lifetime, we just use the flag)
                fallback.is_known = false;
                fallback
            }
        }
    }

    fn new(
        name: &str,
        reference: &str,
        phases: u8,
        max_power_kw: f32,
        max_current_amps: u8,
        has_schuko: bool,
        charging_points: u8,
    ) -> Self {
        Self {
            name: name.to_string(),
            reference: reference.to_string(),
            phases,
            max_power_kw,
            max_current_amps,
            has_schuko,
            charging_points,
            is_known: true,
        }
    }
}
