use serde::{Deserialize, Serialize};

/// Real-time Linky TIC telemetry parsed from custom ATmega firmware `MacAdr:TIC:...` responses.
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomTicTelemetry {
    /// Indicates whether custom TIC firmware responses are active.
    pub active: bool,
    /// TIC mode: "AUCUN", "HISTORIQUE", "STANDARD", or None.
    pub mode: Option<String>,
    /// Tarif period: e.g. "HP" (Heures Pleines), "HC" (Heures Creuses), "TH" (Toutes Heures).
    pub ptec: Option<String>,
    /// Subscribed current in Amperes (ISOUSC).
    pub isousc: Option<u32>,
    /// Instantaneous current in Amperes (Single-phase Historique).
    pub iinst: Option<u32>,
    /// Instantaneous current phase 1 (Amperes).
    pub iinst1: Option<u32>,
    /// Instantaneous current phase 2 (Amperes).
    pub iinst2: Option<u32>,
    /// Instantaneous current phase 3 (Amperes).
    pub iinst3: Option<u32>,
    /// RMS voltage phase 1 in Volts (URMS1, Standard mode).
    pub urms1: Option<u32>,
    /// RMS voltage phase 2 in Volts (URMS2, Three-phase Standard mode).
    pub urms2: Option<u32>,
    /// RMS voltage phase 3 in Volts (URMS3, Three-phase Standard mode).
    pub urms3: Option<u32>,
    /// RMS current phase 1 in Amperes (IRMS1, Standard mode).
    pub irms1: Option<u32>,
    /// Reference power limit in kVA (PREF, Standard mode).
    pub pref: Option<u32>,
    /// Instantaneous apparent power phase 1 in VA (SINST1, equivalent to PAPP).
    pub sinst1: Option<u32>,
    /// Instantaneous apparent power phase 2 in VA (SINST2).
    pub sinst2: Option<u32>,
    /// Instantaneous apparent power phase 3 in VA (SINST3).
    pub sinst3: Option<u32>,
    /// Raw unparsed payload string received from serial link.
    pub raw: String,
    /// ISO 8601 timestamp of when the telemetry was received.
    pub timestamp: Option<String>,
}

impl CustomTicTelemetry {
    /// Parses a raw payload string received after `MacAdr:` (e.g. `TIC:HISTORIQUE;IINST:18;ISOUSC:45;PTEC:HP`).
    pub fn parse(payload: &str) -> Self {
        let clean = payload.trim();
        let mut tic = Self {
            active: true,
            raw: clean.to_string(),
            timestamp: Some(chrono::Local::now().to_rfc3339()),
            ..Default::default()
        };

        let parts: Vec<&str> = clean.split(';').collect();
        for (i, part) in parts.iter().enumerate() {
            let part = part.trim();
            if i == 0 {
                // First part is "TIC:<MODE>" (e.g. "TIC:HISTORIQUE", "TIC:STANDARD", "TIC:AUCUN")
                if let Some(mode) = part.strip_prefix("TIC:") {
                    tic.mode = Some(mode.to_string());
                } else if part == "TIC" {
                    tic.mode = Some("UNKNOWN".to_string());
                }
            } else if let Some((key, val)) = part.split_once(':') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "IINST" => {
                        let parsed = val.parse::<u32>().ok();
                        tic.iinst = parsed;
                        if tic.iinst1.is_none() {
                            tic.iinst1 = parsed;
                        }
                    }
                    "IINST1" => tic.iinst1 = val.parse::<u32>().ok(),
                    "IINST2" => tic.iinst2 = val.parse::<u32>().ok(),
                    "IINST3" => tic.iinst3 = val.parse::<u32>().ok(),
                    "ISOUSC" => tic.isousc = val.parse::<u32>().ok(),
                    "URMS1" => tic.urms1 = val.parse::<u32>().ok(),
                    "URMS2" => tic.urms2 = val.parse::<u32>().ok(),
                    "URMS3" => tic.urms3 = val.parse::<u32>().ok(),
                    "IRMS1" => tic.irms1 = val.parse::<u32>().ok(),
                    "PREF" => tic.pref = val.parse::<u32>().ok(),
                    "SINST1" => tic.sinst1 = val.parse::<u32>().ok(),
                    "SINST2" => tic.sinst2 = val.parse::<u32>().ok(),
                    "SINST3" => tic.sinst3 = val.parse::<u32>().ok(),
                    "PTEC" => tic.ptec = Some(val.to_string()),
                    _ => {}
                }
            }
        }

        tic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tic_aucun() {
        let tic = CustomTicTelemetry::parse("TIC:AUCUN");
        assert!(tic.active);
        assert_eq!(tic.mode.as_deref(), Some("AUCUN"));
        assert_eq!(tic.raw, "TIC:AUCUN");
        assert!(tic.isousc.is_none());
        assert!(tic.ptec.is_none());
    }

    #[test]
    fn parses_historique_single_phase() {
        let tic = CustomTicTelemetry::parse("TIC:HISTORIQUE;IINST:18;ISOUSC:45;PTEC:HP");
        assert!(tic.active);
        assert_eq!(tic.mode.as_deref(), Some("HISTORIQUE"));
        assert_eq!(tic.iinst, Some(18));
        assert_eq!(tic.iinst1, Some(18));
        assert_eq!(tic.isousc, Some(45));
        assert_eq!(tic.ptec.as_deref(), Some("HP"));
    }

    #[test]
    fn parses_historique_three_phase() {
        let tic = CustomTicTelemetry::parse("TIC:HISTORIQUE;IINST1:10;IINST2:12;IINST3:8;ISOUSC:30;PTEC:HC");
        assert!(tic.active);
        assert_eq!(tic.mode.as_deref(), Some("HISTORIQUE"));
        assert_eq!(tic.iinst1, Some(10));
        assert_eq!(tic.iinst2, Some(12));
        assert_eq!(tic.iinst3, Some(8));
        assert_eq!(tic.isousc, Some(30));
        assert_eq!(tic.ptec.as_deref(), Some("HC"));
    }

    #[test]
    fn parses_standard_single_phase() {
        let tic = CustomTicTelemetry::parse("TIC:STANDARD;URMS1:234;IRMS1:14;PREF:9;SINST1:3220;PTEC:HC");
        assert!(tic.active);
        assert_eq!(tic.mode.as_deref(), Some("STANDARD"));
        assert_eq!(tic.urms1, Some(234));
        assert_eq!(tic.irms1, Some(14));
        assert_eq!(tic.pref, Some(9));
        assert_eq!(tic.sinst1, Some(3220));
        assert_eq!(tic.ptec.as_deref(), Some("HC"));
    }

    #[test]
    fn parses_standard_three_phase() {
        let tic = CustomTicTelemetry::parse("TIC:STANDARD;URMS1:234;URMS2:235;URMS3:233;IRMS1:14;PREF:9;SINST1:3220;SINST2:0;SINST3:0;PTEC:HP");
        assert!(tic.active);
        assert_eq!(tic.mode.as_deref(), Some("STANDARD"));
        assert_eq!(tic.urms1, Some(234));
        assert_eq!(tic.urms2, Some(235));
        assert_eq!(tic.urms3, Some(233));
        assert_eq!(tic.irms1, Some(14));
        assert_eq!(tic.pref, Some(9));
        assert_eq!(tic.sinst1, Some(3220));
        assert_eq!(tic.sinst2, Some(0));
        assert_eq!(tic.sinst3, Some(0));
        assert_eq!(tic.ptec.as_deref(), Some("HP"));
    }
}
