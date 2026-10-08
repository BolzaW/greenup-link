//! Commandes émises vers la carte de puissance (TX).
//!
//! Chaque variante de [`Command`] correspond à une trame ASCII documentée dans
//! `docs/LEGRAND_SERIAL_PROTOCOL.md`. La méthode [`Command::encode`] produit la
//! trame prête à être écrite sur le lien série (terminée par `\r`).
//!
//! Cette couche est volontairement pure : aucune I/O, aucun état.

use std::fmt;

/// Terminateur de trame attendu par l'ATmega (Carriage Return, 0x0D).
pub const FRAME_TERMINATOR: &str = "\r";

/// Courant minimal accepté par la carte (en dessous, la carte passe en défaut).
pub const MIN_CURRENT_AMPS: u8 = 7;
/// Courant maximal accepté par la carte.
pub const MAX_CURRENT_AMPS: u8 = 32;

/// Modes de fonctionnement principaux de l'ATmega (`FM:X`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctioningMode {
    /// `FM:1` – Charge directe permanente (mode utilisé par Green'Up Link).
    DirectCharge = 1,
    /// `FM:2` – Pilotage par contact sec (Heures Creuses / Heures Pleines).
    RemoteControl = 2,
    /// `FM:3` – Smart meter (TIC). Non implémenté dans le firmware 18.04.
    SmartMeter = 3,
    /// `FM:4` – Programmation horaire interne.
    Programming = 4,
    /// `FM:5` – Pilotage Modbus (DLM).
    Modbus = 5,
    /// `FM:6` – Supervision OCPP (modifie fortement le comportement interne).
    Ocpp = 6,
}

impl FunctioningMode {
    /// Code numérique envoyé dans la trame `FM:X`.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Décode la valeur reçue dans une trame `FM:X`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim() {
            "1" => Some(Self::DirectCharge),
            "2" => Some(Self::RemoteControl),
            "3" => Some(Self::SmartMeter),
            "4" => Some(Self::Programming),
            "5" => Some(Self::Modbus),
            "6" => Some(Self::Ocpp),
            _ => None,
        }
    }
}

/// Erreurs de construction d'une commande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// Le courant demandé est hors de la plage [`MIN_CURRENT_AMPS`]..=[`MAX_CURRENT_AMPS`].
    CurrentOutOfRange(u8),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::CurrentOutOfRange(a) => write!(
                f,
                "courant {}A hors limites ({}..={}A)",
                a, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS
            ),
        }
    }
}

impl std::error::Error for CommandError {}

/// Toutes les commandes connues pouvant être envoyées à la carte de puissance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // --- 1. Initialisation & Système ---
    /// `RaspberryPiModeOK` – Annonce que le Pi prend le contrôle (réponse au ping `RaspberryPi?`).
    RaspberryPiModeOk,
    /// `SoftwareVersion?`
    GetSoftwareVersion,
    /// `HardwareVersion?`
    GetHardwareVersion,
    /// `SerialNumber?`
    GetSerialNumber,
    /// `Reference?`
    GetReference,
    /// `WeekYearProduction?`
    GetWeekYearProduction,
    /// `Side?` – Côté actif (bornes doubles).
    GetSide,
    /// `Reset` – Redémarre la carte de puissance.
    Reset,
    /// `Test` – Mode test usine.
    FactoryTest,
    /// `ping` – Ping basique.
    Ping,

    // --- 2. Statut & Télémesure ---
    /// `State?`
    GetState,
    /// `E?`
    GetErrors,
    /// `CC?`
    GetCurrentLimit,
    /// `CCEl?`
    GetEliotCurrentLimit,
    /// `CP?`
    GetCpVoltage,
    /// `T2C?`
    GetT2CEnabled,
    /// `SB?`
    GetSbState,
    /// `CCTIC?` – Limite de courant calculée à partir de la TIC.
    GetTicCurrentLimit,
    /// `TICTM:1` / `TICTM:0` – Active/désactive le mode test TIC.
    SetTicTestMode(bool),

    // --- 3. Pilotage de la charge ---
    /// `CC:XX` – Limite de courant prise Type 2 (utiliser [`Command::set_current_limit`] pour valider).
    SetCurrentLimit(u8),
    /// `CCS:XX` – Limite de courant prise Schuko.
    SetSchukoCurrentLimit(u8),
    /// `T2COK` / `T2CNOK` – Autorise/bloque la charge Type 2.
    AuthorizeType2(bool),
    /// `2PCOK` / `2PCNOK` – Autorise/bloque la charge prise domestique.
    AuthorizeDomestic(bool),
    /// `T2FOK` / `T2FNOK` – Force la charge Type 2.
    ForceType2(bool),
    /// `2PFOK` / `2PFNOK` – Force la charge prise domestique.
    ForceDomestic(bool),
    /// `SOK` / `SNOK` – Entre/sort du mode veille (uniquement depuis l'état A).
    SetSleep(bool),
    /// `Unlock` – Déverrouillage physique du câble.
    /// `SBOK` / `SBNOK` - Simule l'appui logiciel sur le bouton START/STOP (Pause/Reprise parfaite).
    SetStartButton(bool),
    Unlock,

    // --- 4. OCPP / RFID ---
    /// `OCPPPS?`
    GetOcppParameters,
    /// `OCPPPACOK` / `OCPPPACNOK` – Plug & Charge.
    SetPlugAndCharge(bool),
    /// `RFIDId:XXXX` – Transmet l'identifiant d'un badge.
    SendRfidId(String),
    /// `RFIDA:XXXX` – Transmet un statut d'autorisation RFID.
    SendRfidAuthorization(String),
    /// `OCPPCTO:XXX` – Timeout de connexion OCPP.
    SetOcppConnectionTimeout(u32),

    // --- 5. Bluetooth ---
    /// `BT?`
    GetBluetooth,
    /// `BTOK` / `BTNOK`
    SetBluetooth(bool),

    // --- 7. Modes de fonctionnement ---
    /// `FM?`
    GetFunctioningMode,
    /// `FM:X`
    SetFunctioningMode(FunctioningMode),
    /// `FM2?` – État de l'Éco-démarrage.
    GetEcoStart,
    /// `FM2:1` / `FM2:0` – Active/désactive l'Éco-démarrage (équivalent DIP2).
    SetEcoStart(bool),

    // --- 8. Debug ---
    /// Envoi d'une commande brute sans acquittement attendu (API de debug).
    Raw(String),
}

fn ok_nok(prefix: &str, enabled: bool) -> String {
    format!("{}{}", prefix, if enabled { "OK" } else { "NOK" })
}

impl Command {
    /// Construit une commande `CC:XX` en validant la plage de courant.
    pub fn set_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetCurrentLimit)
    }

    /// Construit une commande `CCS:XX` en validant la plage de courant.
    pub fn set_schuko_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetSchukoCurrentLimit)
    }

    fn validate_current(amps: u8) -> Result<u8, CommandError> {
        if (MIN_CURRENT_AMPS..=MAX_CURRENT_AMPS).contains(&amps) {
            Ok(amps)
        } else {
            Err(CommandError::CurrentOutOfRange(amps))
        }
    }

    /// Trame ASCII sans terminateur (utile pour les logs).
    pub fn as_frame(&self) -> String {
        match self {
            Command::RaspberryPiModeOk => "RaspberryPiModeOK".into(),
            Command::GetSoftwareVersion => "SoftwareVersion?".into(),
            Command::GetHardwareVersion => "HardwareVersion?".into(),
            Command::GetSerialNumber => "SerialNumber?".into(),
            Command::GetReference => "Reference?".into(),
            Command::GetWeekYearProduction => "WeekYearProduction?".into(),
            Command::GetSide => "Side?".into(),
            Command::Reset => "Reset".into(),
            Command::FactoryTest => "Test".into(),
            Command::Ping => "ping".into(),

            Command::GetState => "State?".into(),
            Command::GetErrors => "E?".into(),
            Command::GetCurrentLimit => "CC?".into(),
            Command::GetEliotCurrentLimit => "CCEl?".into(),
            Command::GetCpVoltage => "CP?".into(),
            Command::GetT2CEnabled => "T2C?".into(),
            Command::GetSbState => "SB?".into(),
            Command::GetTicCurrentLimit => "CCTIC?".into(),
            Command::SetTicTestMode(on) => format!("TICTM:{}", *on as u8),

            Command::SetCurrentLimit(a) => format!("CCEl:{:02}", a),
            Command::SetSchukoCurrentLimit(a) => format!("CCS:{:02}", a),
            Command::AuthorizeType2(on) => ok_nok("T2C", *on),
            Command::AuthorizeDomestic(on) => ok_nok("2PC", *on),
            Command::ForceType2(on) => ok_nok("T2F", *on),
            Command::ForceDomestic(on) => ok_nok("2PF", *on),
            Command::SetSleep(on) => ok_nok("S", *on),
            Command::SetStartButton(on) => ok_nok("SB", *on),
            Command::Unlock => "Unlock".into(),

            Command::GetOcppParameters => "OCPPPS?".into(),
            Command::SetPlugAndCharge(on) => ok_nok("OCPPPAC", *on),
            Command::SendRfidId(id) => format!("RFIDId:{}", id),
            Command::SendRfidAuthorization(s) => format!("RFIDA:{}", s),
            Command::SetOcppConnectionTimeout(t) => format!("OCPPCTO:{}", t),

            Command::GetBluetooth => "BT?".into(),
            Command::SetBluetooth(on) => ok_nok("BT", *on),

            Command::GetFunctioningMode => "FM?".into(),
            Command::SetFunctioningMode(m) => format!("FM:{}", m.code()),
            Command::GetEcoStart => "FM2?".into(),
            Command::SetEcoStart(on) => format!("FM2:{}", *on as u8),
            Command::Raw(s) => s.clone(),
        }
    }

    /// Trame complète prête à être écrite sur le lien série (avec `\r`).
    pub fn encode(&self) -> String {
        format!("{}{}", self.as_frame(), FRAME_TERMINATOR)
    }

    /// Détermine le préfixe attendu en réponse pour acquitter cette commande.
    /// Si `None`, la commande ne nécessite pas d'acquittement ou est de type "fire-and-forget" (ex: Raw).
    pub fn expected_rx_prefix(&self) -> Option<&'static str> {
        match self {
            Command::RaspberryPiModeOk => Some("Side:"),
            Command::GetSoftwareVersion => Some("SoftwareVersion:"),
            Command::GetHardwareVersion => Some("HardwareVersion:"),
            Command::GetSerialNumber => Some("SerialNumber:"),
            Command::GetReference => Some("Reference:"),
            Command::GetWeekYearProduction => Some("WeekYearProduction:"),
            Command::GetSide => Some("Side:"),
            Command::Reset => Some("State:"), // Reset renvoie l'état après le reboot
            Command::FactoryTest => None,
            Command::Ping => Some("pong"),

            Command::GetState => Some("State:"),
            Command::GetErrors => Some("E:"),
            Command::GetCurrentLimit => Some("CC:"),
            Command::GetEliotCurrentLimit => Some("CCEl:"),
            Command::GetCpVoltage => Some("CP:"),
            Command::GetT2CEnabled => Some("T2C:"),
            Command::GetSbState => Some("SB:"),
            Command::GetTicCurrentLimit => Some("CCTIC:"),
            Command::SetTicTestMode(true) => Some("TICTM:1"),
            Command::SetTicTestMode(false) => Some("TICTM:0"),

            Command::SetCurrentLimit(_) => Some("CCEl:"),
            Command::SetSchukoCurrentLimit(_) => Some("CCS:"),
            Command::AuthorizeType2(true) => Some("T2C:1"),
            Command::AuthorizeType2(false) => Some("T2C:0"),
            Command::AuthorizeDomestic(true) => Some("2PC:1"),
            Command::AuthorizeDomestic(false) => Some("2PC:0"),
            Command::ForceType2(true) => Some("T2F:1"),
            Command::ForceType2(false) => Some("T2F:0"),
            Command::ForceDomestic(true) => Some("2PF:1"),
            Command::ForceDomestic(false) => Some("2PF:0"),
            Command::SetSleep(_) => Some("Slp:"),
            Command::SetStartButton(_) => Some("SB:"),
            Command::Unlock => None, // À vérifier

            Command::GetOcppParameters => Some("OCPPPS:"),
            Command::SetPlugAndCharge(_) => Some("OCPPPAC:"),
            Command::SendRfidId(_) => Some("RFIDId:"),
            Command::SendRfidAuthorization(_) => Some("RFIDA:"),
            Command::SetOcppConnectionTimeout(_) => Some("OCPPCTO:"),

            Command::GetBluetooth => Some("BT:"),
            Command::SetBluetooth(true) => Some("BT:1"),
            Command::SetBluetooth(false) => Some("BT:0"),

            Command::GetFunctioningMode => Some("FM:"),
            Command::SetFunctioningMode(_) => Some("FM:"),
            Command::GetEcoStart => Some("FM2:"),
            Command::SetEcoStart(_) => Some("FM2:"),
            Command::Raw(_) => None,
        }
    }

    /// Séquence d'interrogation recommandée au démarrage (hors `RaspberryPiModeOK` et TIC).
    pub fn startup_queries() -> Vec<Command> {
        vec![
            Command::GetSoftwareVersion,
            Command::GetHardwareVersion,
            Command::GetSerialNumber,
            Command::GetReference,
            Command::GetWeekYearProduction,
            Command::GetState,
            Command::GetFunctioningMode,
            Command::GetCurrentLimit,
            Command::GetEliotCurrentLimit,
            Command::GetCpVoltage,
            Command::GetT2CEnabled,
            Command::GetSbState,
            Command::GetErrors,
            Command::GetBluetooth,
        ]
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_frame())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_frames() {
        assert_eq!(Command::RaspberryPiModeOk.encode(), "RaspberryPiModeOK\r");
        assert_eq!(Command::GetState.encode(), "State?\r");
        assert_eq!(Command::Unlock.encode(), "Unlock\r");
    }

    #[test]
    fn encodes_ok_nok_pairs() {
        assert_eq!(Command::SetStartButton(false).as_frame(), "SBNOK");
        assert_eq!(Command::AuthorizeType2(true).as_frame(), "T2COK");
        assert_eq!(Command::AuthorizeType2(false).as_frame(), "T2CNOK");
        assert_eq!(Command::AuthorizeDomestic(true).as_frame(), "2PCOK");
        assert_eq!(Command::ForceType2(false).as_frame(), "T2FNOK");
        assert_eq!(Command::SetSleep(true).as_frame(), "SOK");
        assert_eq!(Command::SetBluetooth(false).as_frame(), "BTNOK");
        assert_eq!(Command::SetPlugAndCharge(true).as_frame(), "OCPPPACOK");
    }

    #[test]
    fn encodes_parameterized_frames() {
        assert_eq!(Command::SetCurrentLimit(7).as_frame(), "CCEl:07");
        assert_eq!(Command::SetCurrentLimit(16).as_frame(), "CCEl:16");
        assert_eq!(Command::SetSchukoCurrentLimit(10).as_frame(), "CCS:10");
        assert_eq!(Command::SetTicTestMode(true).as_frame(), "TICTM:1");
        assert_eq!(Command::SetEcoStart(false).as_frame(), "FM2:0");
        assert_eq!(
            Command::SetFunctioningMode(FunctioningMode::DirectCharge).as_frame(),
            "FM:1"
        );
        assert_eq!(Command::SendRfidId("04A1B2".into()).as_frame(), "RFIDId:04A1B2");
        assert_eq!(Command::SetOcppConnectionTimeout(120).as_frame(), "OCPPCTO:120");
    }

    #[test]
    fn validates_current_range() {
        assert!(Command::set_current_limit(9).is_err());
        assert!(Command::set_current_limit(33).is_err());
        assert_eq!(
            Command::set_current_limit(16),
            Ok(Command::SetCurrentLimit(16))
        );
    }

    #[test]
    fn functioning_mode_roundtrip() {
        for code in 1..=6u8 {
            let m = FunctioningMode::from_code(&code.to_string()).unwrap();
            assert_eq!(m.code(), code);
        }
        assert!(FunctioningMode::from_code("7").is_none());
    }
}
