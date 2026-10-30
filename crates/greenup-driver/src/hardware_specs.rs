use serde::{Deserialize, Serialize};

/// Hardware specifications of the charging station, deduced from its reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HardwareCapabilities {
    /// Commercial name of the charging station.
    #[serde(skip)]
    pub name: String,
    /// Legrand reference (without spaces).
    #[serde(skip)]
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
    /// Indicates whether this model is equipped with a Linky TIC input (French market).
    pub has_tic: bool,
}

impl Default for HardwareCapabilities {
    /// Default charging station if the reference is unknown (Single-phase, 4.6kW, 1 T2, no Schuko, no TIC).
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
            has_tic: false,
        }
    }
}

impl HardwareCapabilities {
    /// Returns the hardware specifications from its Legrand reference (e.g. "058001").
    pub fn from_reference(reference: &str) -> Self {
        // Cleans the reference to remove potential spaces
        let clean_ref = reference.replace(" ", "");

        match clean_ref.as_str() {
            // Plastic models - Single charge point (without kit)
            "058000" => Self::new("Green’up Premium 3,7–4,6 kW T2", "058000", 1, 4.6, 20, false, 1, true),
            "058001" => Self::new("Green’up Premium 3,7–7,4 kW T2", "058001", 1, 7.4, 32, false, 1, true),
            "058002" => Self::new("Green’up Premium 11–22 kW T2", "058002", 3, 22.0, 32, false, 1, true),
            "058003" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko", "058003", 1, 4.6, 20, true, 1, true),
            "058004" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko", "058004", 1, 7.4, 32, true, 1, true),
            "058030" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko", "058030", 1, 4.6, 20, true, 1, false),
            "058035" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko", "058035", 1, 7.4, 32, true, 1, false),

            // Plastic models - Single charge point (with kit)
            "059000" => Self::new("Green’up Premium 3,7–4,6 kW T2 avec kit", "059000", 1, 4.6, 20, false, 1, false),
            "059001" => Self::new("Green’up Premium 3,7–7,4 kW T2 avec kit", "059001", 1, 7.4, 32, false, 1, false),
            "059002" => Self::new("Green’up Premium 11–22 kW T2 avec kit", "059002", 3, 22.0, 32, false, 1, false),
            "059003" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko avec kit", "059003", 1, 4.6, 20, true, 1, false),
            "059004" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko avec kit", "059004", 1, 7.4, 32, true, 1, false),
            "059005" => Self::new("Green’up Premium 3,7–4,6 kW T2 avec kit", "059005", 1, 4.6, 20, false, 1, true),
            "059006" => Self::new("Green’up Premium 3,7–7,4 kW T2 avec kit", "059006", 1, 7.4, 32, false, 1, true),
            "059007" => Self::new("Green’up Premium 11–22 kW T2 avec kit", "059007", 3, 22.0, 32, false, 1, true),
            "059008" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko avec kit", "059008", 1, 4.6, 20, true, 1, true),
            "059009" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko avec kit", "059009", 1, 7.4, 32, true, 1, true),
            "059030" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko avec kit", "059030", 1, 4.6, 20, true, 1, false),
            "059035" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko avec kit", "059035", 1, 7.4, 32, true, 1, false),
            "059070" => Self::new("Green’up Premium 3,7–4,6 kW T2 + schuko (Sans Disjoncteur)", "059070", 1, 4.6, 20, true, 1, false),
            "059071" => Self::new("Green’up Premium 3,7–7,4 kW T2 + schuko (Sans Disjoncteur)", "059071", 1, 7.4, 32, true, 1, false),

            // Metal models - Single charge point (1 pt)
            "058010" => Self::new("Green’up Premium métal 3,7–4,6 kW 1 pt T2 + schuko", "058010", 1, 4.6, 20, true, 1, true),
            "058012" => Self::new("Green’up Premium métal 3,7–7,4 kW 1 pt T2 + schuko", "058012", 1, 7.4, 32, true, 1, true),
            "058014" => Self::new("Green’up Premium métal 11–22 kW 1 pt T2 + schuko", "058014", 3, 22.0, 32, true, 1, true),
            "058041" => Self::new("Green’up Premium métal 3,7–4,6 kW 1 pt T2 + schuko EV", "058041", 1, 4.6, 20, true, 1, false),
            "058043" => Self::new("Green’up Premium métal 3,7–7,4 kW 1 pt T2 + schuko EV", "058043", 1, 7.4, 32, true, 1, false),
            "058048" => Self::new("Green’up Premium métal 11–22 kW 1 pt T2 + schuko EV", "058048", 3, 22.0, 32, true, 1, false),

            "059010" => Self::new("Green’up Premium métal 3,7–4,6 kW 1 pt T2 + schuko avec kit", "059010", 1, 4.6, 20, true, 1, false),
            "059012" => Self::new("Green’up Premium métal 3,7–7,4 kW 1 pt T2 + schuko avec kit", "059012", 1, 7.4, 32, true, 1, false),
            "059014" => Self::new("Green’up Premium métal 11–22 kW 1 pt T2 + schuko avec kit", "059014", 3, 22.0, 32, true, 1, false),
            "059041" => Self::new("Green’up Premium métal 3,7–4,6 kW 1 pt T2 + schuko EV avec kit", "059041", 1, 4.6, 20, true, 1, false),
            "059043" => Self::new("Green’up Premium métal 3,7–7,4 kW 1 pt T2 + schuko EV avec kit", "059043", 1, 7.4, 32, true, 1, false),
            "059048" => Self::new("Green’up Premium métal 11–22 kW 1 pt T2 + schuko EV avec kit", "059048", 3, 22.0, 32, true, 1, false),

            // Metal models - Dual charge point (2 pts) — Full official Legrand 12 references list
            "058011" => Self::new("Green’up Premium métal 3,7–4,6 kW 2 pts T2 + schuko", "058011", 1, 4.6, 20, true, 2, false),
            "059011" => Self::new("Green’up Premium métal 3,7–4,6 kW 2 pts T2 + schuko avec kit", "059011", 1, 4.6, 20, true, 2, false),
            "058042" => Self::new("Green’up Premium métal 3,7–4,6 kW 2 pts T2 + schuko EV", "058042", 1, 4.6, 20, true, 2, false),
            "059042" => Self::new("Green’up Premium métal 3,7–4,6 kW 2 pts T2 + schuko EV avec kit", "059042", 1, 4.6, 20, true, 2, false),

            "058013" => Self::new("Green’up Premium métal 3,7–7,4 kW 2 pts T2 + schuko", "058013", 1, 7.4, 32, true, 2, false),
            "059013" => Self::new("Green’up Premium métal 3,7–7,4 kW 2 pts T2 + schuko avec kit", "059013", 1, 7.4, 32, true, 2, false),
            "058044" => Self::new("Green’up Premium métal 3,7–7,4 kW 2 pts T2 + schuko EV", "058044", 1, 7.4, 32, true, 2, false),
            "059044" => Self::new("Green’up Premium métal 3,7–7,4 kW 2 pts T2 + schuko EV avec kit", "059044", 1, 7.4, 32, true, 2, false),

            "058015" => Self::new("Green’up Premium métal 11–22 kW 2 pts T2 + schuko", "058015", 3, 22.0, 32, true, 2, false),
            "059015" => Self::new("Green’up Premium métal 11–22 kW 2 pts T2 + schuko avec kit", "059015", 3, 22.0, 32, true, 2, false),
            "058049" => Self::new("Green’up Premium métal 11–22 kW 2 pts T2 + schuko EV", "058049", 3, 22.0, 32, true, 2, false),
            "059049" => Self::new("Green’up Premium métal 11–22 kW 2 pts T2 + schuko EV avec kit", "059049", 3, 22.0, 32, true, 2, false),

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
        has_tic: bool,
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
            has_tic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tic_compatible_references() {
        let tic_refs = [
            "058000", "058001", "058002", "058003", "058004",
            "059005", "059006", "059007", "059008", "059009",
            "058010", "058012", "058014",
        ];

        for r in tic_refs {
            let spec = HardwareCapabilities::from_reference(r);
            assert!(
                spec.has_tic,
                "Reference {} should have TIC support enabled according to Legrand EXB22062",
                r
            );
            assert!(spec.is_known, "Reference {} should be known", r);
        }
    }

    #[test]
    fn test_non_tic_references() {
        let non_tic_refs = [
            "058030", "058035", "059000", "059001", "059002",
            "059003", "059004", "059030", "059035", "059070",
            "059071", "058041", "058043", "058048", "059010",
            "059012", "059014", "059041", "059043", "059048",
            "058011", "059011", "058042", "059042", "058013",
            "059013", "058044", "059044", "058015", "059015",
            "058049", "059049", "UNKNOWN_REF",
        ];

        for r in non_tic_refs {
            let spec = HardwareCapabilities::from_reference(r);
            assert!(
                !spec.has_tic,
                "Reference {} should NOT have TIC support enabled",
                r
            );
        }
    }

    #[test]
    fn test_reference_with_spaces() {
        let spec = HardwareCapabilities::from_reference("0 580 01");
        assert!(spec.has_tic);
        assert_eq!(spec.max_current_amps, 32);
        assert_eq!(spec.phases, 1);
        assert!(!spec.has_schuko);
        assert_eq!(spec.charging_points, 1);
    }
}
