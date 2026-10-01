// ============================================================================
// flash.rs — Module EXPÉRIMENTAL de flash firmware pour carte de puissance
// ============================================================================
//
// ⚠️  AVERTISSEMENT — FONCTIONNALITÉ EXPÉRIMENTALE ⚠️
//
//  Ce module permet de flasher un firmware custom sur la carte de puissance
//  (ATmega2560) de la borne Legrand Green'Up Premium.
//
//  AUCUNE vérification de version n'est imposée : il est possible de flasher
//  une version plus ancienne, identique, ou un firmware entièrement custom.
//
//  RISQUES :
//   - Un firmware incompatible peut rendre la borne inopérante.
//   - Une interruption pendant le flash peut corrompre le bootloader.
//   - Legrand ne fournira aucun support pour une borne modifiée.
//
//  Utilisez cette fonctionnalité à vos risques et périls.
//
// ============================================================================

use crate::logger;
use crate::state::SharedState;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Répertoire de stockage temporaire des firmwares uploadés
const FIRMWARE_UPLOAD_DIR: &str = "/home/pi/Desktop/Java/Upload/Firmware";

/// Chemin vers la configuration avrdude
const AVRDUDE_CONF: &str = "/etc/avrdude.conf";

/// Baudrate pour la communication avec le bootloader
const AVRDUDE_BAUD: u32 = 115200;

/// Disclaimer affiché avant chaque opération de flash
pub const EXPERIMENTAL_DISCLAIMER: &str = "\
⚠️  FONCTIONNALITÉ EXPÉRIMENTALE — UTILISATION À VOS RISQUES ET PÉRILS ⚠️\n\
\n\
Ce module permet de flasher un firmware arbitraire sur la carte de puissance\n\
(ATmega2560) sans vérification de version. Cela signifie que :\n\
\n\
  • Vous pouvez flasher une version PLUS ANCIENNE que celle installée.\n\
  • Vous pouvez flasher un firmware CUSTOM non officiel.\n\
  • Un firmware incompatible peut RENDRE LA BORNE INOPÉRANTE.\n\
  • Une interruption pendant le flash peut CORROMPRE le bootloader.\n\
  • Legrand ne fournira AUCUN SUPPORT pour une borne modifiée.\n\
\n\
En utilisant cette fonctionnalité, vous acceptez l'entière responsabilité\n\
de toute conséquence sur votre matériel.";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Représente la version parsée depuis le nom du fichier firmware
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl FirmwareVersion {
    /// Parse une version depuis un nom de fichier au format "FirmwareBoardA-V01;18;10.hex"
    /// Retourne None si le format n'est pas reconnu (firmware custom)
    pub fn from_filename(filename: &str) -> Option<Self> {
        // Extrait la partie après "FirmwareBoardA-V" et avant ".hex"
        let name = filename
            .strip_prefix("FirmwareBoardA-V")
            .or_else(|| filename.strip_prefix("FirmwareBoardA-v"))?;
        let name = name.strip_suffix(".hex")?;

        let parts: Vec<&str> = name.split(';').collect();
        if parts.len() != 3 {
            return None;
        }

        Some(FirmwareVersion {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    /// Parse une version depuis la chaîne retournée par la carte (ex: "V01.18.10")
    pub fn from_board_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix('V').or_else(|| s.strip_prefix('v')).unwrap_or(s);
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(FirmwareVersion {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V{:02}.{:02}.{:02}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for FirmwareVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FirmwareVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

/// État courant du processus de flash
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlashStatus {
    /// Aucun flash en cours
    Idle,
    /// Fichier uploadé, en attente de confirmation
    Ready,
    /// Envoi de la commande de reset vers le bootloader
    EnteringBootloader,
    /// avrdude est en train de flasher
    Flashing,
    /// Flash terminé avec succès
    Success,
    /// Flash échoué
    Failed,
}

impl Default for FlashStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Informations sur le flash en cours ou le dernier flash effectué
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlashState {
    pub status: FlashStatus,
    /// Chemin vers le fichier .hex à flasher
    pub hex_file: Option<String>,
    /// Version du firmware à flasher (si parseable depuis le nom)
    pub firmware_version: Option<FirmwareVersion>,
    /// Version actuellement installée sur la carte
    pub current_version: Option<FirmwareVersion>,
    /// Indique si c'est un downgrade (version inférieure à l'actuelle)
    pub is_downgrade: bool,
    /// Port série ciblé
    pub target_port: String,
    /// Message de progression ou d'erreur
    pub message: String,
    /// Sortie complète d'avrdude (pour diagnostic)
    pub avrdude_output: String,
    /// Horodatage du dernier événement
    pub last_updated: String,
}

// ---------------------------------------------------------------------------
// Parsing Intel HEX — Validation du fichier avant flash
// ---------------------------------------------------------------------------

/// Résultat de la validation d'un fichier Intel HEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexValidation {
    pub valid: bool,
    pub total_lines: usize,
    pub data_records: usize,
    pub total_bytes: usize,
    pub has_eof: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Valide un fichier Intel HEX sans le parser intégralement.
/// Vérifie :
///  - Le format de chaque ligne (`:LLAAAATT...CC`)
///  - Le checksum de chaque enregistrement
///  - La présence d'un enregistrement EOF
///
/// Ne vérifie PAS :
///  - La version du firmware
///  - La compatibilité avec le hardware cible
pub fn validate_hex_file(path: &str) -> HexValidation {
    let mut result = HexValidation {
        valid: true,
        total_lines: 0,
        data_records: 0,
        total_bytes: 0,
        has_eof: false,
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            result.valid = false;
            result.errors.push(format!("Impossible de lire le fichier: {}", e));
            return result;
        }
    };

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        result.total_lines += 1;

        // Chaque ligne doit commencer par ':'
        if !line.starts_with(':') {
            result.valid = false;
            result
                .errors
                .push(format!("Ligne {} : ne commence pas par ':'", line_num + 1));
            continue;
        }

        let hex_data = &line[1..];

        // Longueur minimale : LL(2) + AAAA(4) + TT(2) + CC(2) = 10 caractères hex
        if hex_data.len() < 10 {
            result.valid = false;
            result
                .errors
                .push(format!("Ligne {} : trop courte", line_num + 1));
            continue;
        }

        // Vérifier que tout est bien du hex
        if !hex_data.chars().all(|c| c.is_ascii_hexdigit()) {
            result.valid = false;
            result.errors.push(format!(
                "Ligne {} : contient des caractères non-hexadécimaux",
                line_num + 1
            ));
            continue;
        }

        // Parser les champs
        let byte_count = match u8::from_str_radix(&hex_data[0..2], 16) {
            Ok(v) => v,
            Err(_) => {
                result.valid = false;
                result.errors.push(format!(
                    "Ligne {} : byte count invalide",
                    line_num + 1
                ));
                continue;
            }
        };

        let record_type = match u8::from_str_radix(&hex_data[6..8], 16) {
            Ok(v) => v,
            Err(_) => {
                result.valid = false;
                result.errors.push(format!(
                    "Ligne {} : record type invalide",
                    line_num + 1
                ));
                continue;
            }
        };

        // Vérifier la longueur attendue
        let expected_hex_len = (byte_count as usize + 5) * 2; // LL + AAAA + TT + DATA + CC
        if hex_data.len() != expected_hex_len {
            result.valid = false;
            result.errors.push(format!(
                "Ligne {} : longueur incorrecte (attendu {} hex chars, trouvé {})",
                line_num + 1,
                expected_hex_len,
                hex_data.len()
            ));
            continue;
        }

        // Vérifier le checksum Intel HEX (somme de tous les octets modulo 256 == 0)
        let mut checksum: u16 = 0;
        let mut i = 0;
        while i + 1 < hex_data.len() {
            if let Ok(byte) = u8::from_str_radix(&hex_data[i..i + 2], 16) {
                checksum += byte as u16;
            }
            i += 2;
        }
        if (checksum & 0xFF) != 0 {
            result.valid = false;
            result.errors.push(format!(
                "Ligne {} : checksum Intel HEX invalide",
                line_num + 1
            ));
        }

        // Compteur de data records
        match record_type {
            0x00 => {
                result.data_records += 1;
                result.total_bytes += byte_count as usize;
            }
            0x01 => {
                result.has_eof = true;
            }
            0x02 | 0x04 => {
                // Extended segment/linear address — OK
            }
            _ => {
                result.warnings.push(format!(
                    "Ligne {} : record type inconnu 0x{:02X}",
                    line_num + 1,
                    record_type
                ));
            }
        }
    }

    if !result.has_eof {
        result.warnings.push(
            "Le fichier ne contient pas de record EOF (type 01). Le fichier pourrait être tronqué."
                .to_string(),
        );
    }

    if result.data_records == 0 {
        result.valid = false;
        result
            .errors
            .push("Le fichier ne contient aucun enregistrement de données.".to_string());
    }

    result
}

// ---------------------------------------------------------------------------
// Contrôle du process de flash
// ---------------------------------------------------------------------------

/// Détecte le port série connecté à la carte de puissance.
/// Retourne "/dev/ttyUSB0" ou "/dev/ttyUSB1" selon ce qui est disponible.
pub fn detect_serial_port() -> Option<String> {
    for port in &["/dev/ttyUSB0", "/dev/ttyUSB1"] {
        if std::path::Path::new(port).exists() {
            return Some(port.to_string());
        }
    }
    None
}

/// Prépare le flash : valide le fichier, détermine si c'est un downgrade,
/// et met à jour l'état partagé.
///
/// Retourne Ok(()) si le fichier est prêt à être flashé, Err sinon.
pub fn prepare_flash(
    state: &SharedState,
    hex_path: &str,
    target_port: &str,
) -> Result<FlashState, String> {
    logger::log("FLASH", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    logger::log("FLASH", "⚠️  FLASH FIRMWARE — FONCTIONNALITÉ EXPÉRIMENTALE");
    logger::log("FLASH", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 1. Vérifier que le fichier existe
    if !std::path::Path::new(hex_path).exists() {
        return Err(format!("Fichier introuvable : {}", hex_path));
    }

    // 2. Valider le format Intel HEX
    logger::log("FLASH", &format!("Validation du fichier : {}", hex_path));
    let validation = validate_hex_file(hex_path);

    if !validation.valid {
        let errors = validation.errors.join("; ");
        logger::log("FLASH", &format!("❌ Fichier invalide : {}", errors));
        return Err(format!("Fichier Intel HEX invalide : {}", errors));
    }

    logger::log(
        "FLASH",
        &format!(
            "✅ Fichier valide : {} records, {} octets de données",
            validation.data_records, validation.total_bytes
        ),
    );

    for warning in &validation.warnings {
        logger::log("FLASH", &format!("⚠️  {}", warning));
    }

    // 3. Extraire la version du nom du fichier (optionnel)
    let filename = std::path::Path::new(hex_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let file_version = FirmwareVersion::from_filename(filename);

    if let Some(ref v) = file_version {
        logger::log("FLASH", &format!("📦 Version du firmware à flasher : {}", v));
    } else {
        logger::log(
            "FLASH",
            &format!(
                "📦 Firmware custom détecté (version non parseable depuis \"{}\")",
                filename
            ),
        );
    }

    // 4. Lire la version actuellement installée
    let current_version = {
        let info = state.info.lock().map_err(|e| format!("Lock error: {}", e))?;
        FirmwareVersion::from_board_string(&info.software_version)
    };

    if let Some(ref v) = current_version {
        logger::log("FLASH", &format!("📋 Version actuellement installée : {}", v));
    } else {
        logger::log(
            "FLASH",
            "📋 Version actuelle inconnue (carte non connectée ou format non reconnu)",
        );
    }

    // 5. Déterminer si c'est un downgrade
    let is_downgrade = match (&file_version, &current_version) {
        (Some(new), Some(cur)) => {
            if new < cur {
                logger::log(
                    "FLASH",
                    &format!(
                        "⚠️  DOWNGRADE DÉTECTÉ : {} → {} (le Java Legrand bloquerait ceci)",
                        cur, new
                    ),
                );
                true
            } else if new == cur {
                logger::log("FLASH", "ℹ️  Même version — re-flash");
                false
            } else {
                logger::log("FLASH", "ℹ️  Upgrade normal");
                false
            }
        }
        _ => false,
    };

    // 6. Construire l'état
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let flash_state = FlashState {
        status: FlashStatus::Ready,
        hex_file: Some(hex_path.to_string()),
        firmware_version: file_version,
        current_version,
        is_downgrade,
        target_port: target_port.to_string(),
        message: "Prêt à flasher. Confirmation requise.".to_string(),
        avrdude_output: String::new(),
        last_updated: now,
    };

    // Mettre à jour l'état partagé
    if let Ok(mut fs) = state.flash_state.lock() {
        *fs = flash_state.clone();
    }

    Ok(flash_state)
}

/// Exécute le flash firmware.
///
/// Cette fonction :
///  1. Envoie la commande "Z" à la carte pour la passer en mode bootloader
///  2. Attend un court délai pour le reset
///  3. Lance avrdude pour flasher le .hex
///  4. Capture la sortie d'avrdude et met à jour l'état
///
/// BLOQUANTE — doit être appelée depuis un thread dédié.
pub fn execute_flash(state: &SharedState) -> Result<(), String> {
    // Récupérer les paramètres depuis l'état
    let (hex_file, target_port) = {
        let fs = state
            .flash_state
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if fs.status != FlashStatus::Ready {
            return Err("Aucun flash préparé. Appelez prepare_flash d'abord.".to_string());
        }
        (
            fs.hex_file
                .clone()
                .ok_or("Pas de fichier .hex configuré")?,
            fs.target_port.clone(),
        )
    };

    // Log du disclaimer complet
    for line in EXPERIMENTAL_DISCLAIMER.lines() {
        logger::log("FLASH", line);
    }

    // --- Phase 1 : Entrée en bootloader ---
    update_flash_status(
        state,
        FlashStatus::EnteringBootloader,
        "Envoi de la commande de reset vers le bootloader...",
    );

    // On envoie "Z" via le canal série pour que la carte reboot en bootloader
    // Cela reproduit le writeChargePointComFile(14, "Z") du Java
    logger::log("FLASH", "📡 Envoi commande bootloader via canal série...");
    if let Err(e) = state.serial_tx.try_send("Z\r".to_string()) {
        logger::log(
            "FLASH",
            &format!(
                "⚠️  Impossible d'envoyer via canal série ({}), avrdude tentera quand même",
                e
            ),
        );
    }

    // Délai pour laisser le reset se faire et le bootloader démarrer
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // --- Phase 2 : Flash via avrdude ---
    update_flash_status(state, FlashStatus::Flashing, "avrdude en cours d'exécution...");

    logger::log("FLASH", "🔧 Lancement d'avrdude...");
    let avrdude_args = [
        format!("-C{}", AVRDUDE_CONF),
        "-v".to_string(),
        "-patmega2560".to_string(),
        "-cwiring".to_string(),
        format!("-P{}", target_port),
        format!("-b{}", AVRDUDE_BAUD),
        "-D".to_string(),
        format!("-Uflash:w:{}:i", hex_file),
    ];

    logger::log(
        "FLASH",
        &format!("   $ sudo avrdude {}", avrdude_args.join(" ")),
    );

    let mut child = Command::new("sudo")
        .arg("avrdude")
        .args(&avrdude_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let msg = format!("Impossible de lancer avrdude : {}", e);
            update_flash_status(state, FlashStatus::Failed, &msg);
            msg
        })?;

    // Capturer la sortie (avrdude écrit principalement sur stderr)
    let mut avrdude_output = String::new();

    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                logger::log("AVRDUDE", &line);
                avrdude_output.push_str(&line);
                avrdude_output.push('\n');

                // Mise à jour temps réel du message de progression
                if line.contains("writing flash") || line.contains("reading on-chip") {
                    update_flash_message(state, &line);
                }
            }
        }
    }

    let exit_status = child.wait().map_err(|e| {
        let msg = format!("Erreur pendant l'attente d'avrdude : {}", e);
        update_flash_status(state, FlashStatus::Failed, &msg);
        msg
    })?;

    // Stocker la sortie complète d'avrdude
    if let Ok(mut fs) = state.flash_state.lock() {
        fs.avrdude_output = avrdude_output.clone();
    }

    if exit_status.success() {
        logger::log("FLASH", "✅ Flash terminé avec succès !");
        update_flash_status(state, FlashStatus::Success, "Flash terminé avec succès !");

        // Envoyer RaspberryPiModeOK pour reprendre la communication normale
        std::thread::sleep(std::time::Duration::from_millis(2000));
        let _ = state
            .serial_tx
            .try_send("RaspberryPiModeOK\r".to_string());
        logger::log("FLASH", "📡 Reprise communication série (RaspberryPiModeOK)");

        Ok(())
    } else {
        let code = exit_status.code().unwrap_or(-1);
        let msg = format!("avrdude a échoué avec le code de sortie {}", code);
        logger::log("FLASH", &format!("❌ {}", msg));
        update_flash_status(state, FlashStatus::Failed, &msg);
        Err(msg)
    }
}

/// Réinitialise l'état du flash à Idle
pub fn reset_flash_state(state: &SharedState) {
    if let Ok(mut fs) = state.flash_state.lock() {
        *fs = FlashState::default();
    }
}

// ---------------------------------------------------------------------------
// Utilitaires internes
// ---------------------------------------------------------------------------

/// Met à jour le statut et le message du flash dans l'état partagé
fn update_flash_status(state: &SharedState, status: FlashStatus, message: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    if let Ok(mut fs) = state.flash_state.lock() {
        fs.status = status;
        fs.message = message.to_string();
        fs.last_updated = now;
    }
}

/// Met à jour uniquement le message de progression
fn update_flash_message(state: &SharedState, message: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    if let Ok(mut fs) = state.flash_state.lock() {
        fs.message = message.to_string();
        fs.last_updated = now;
    }
}

/// Liste les fichiers .hex disponibles dans le répertoire de firmware
pub fn list_available_firmwares() -> Vec<FirmwareInfo> {
    let mut firmwares = Vec::new();
    let firmware_dir = PathBuf::from(FIRMWARE_UPLOAD_DIR);

    if !firmware_dir.exists() {
        // Créer le répertoire s'il n'existe pas
        let _ = fs::create_dir_all(&firmware_dir);
        return firmwares;
    }

    if let Ok(entries) = fs::read_dir(&firmware_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "hex" {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let version = FirmwareVersion::from_filename(&filename);
                    let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                    firmwares.push(FirmwareInfo {
                        filename: filename.clone(),
                        path: path.to_string_lossy().to_string(),
                        version,
                        file_size,
                    });
                }
            }
        }
    }

    // Trier par version (les custom sans version à la fin)
    firmwares.sort_by(|a, b| match (&a.version, &b.version) {
        (Some(va), Some(vb)) => vb.cmp(va), // Plus récent en premier
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.filename.cmp(&b.filename),
    });

    firmwares
}

/// Informations sur un fichier firmware disponible
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareInfo {
    pub filename: String,
    pub path: String,
    pub version: Option<FirmwareVersion>,
    pub file_size: u64,
}

// ---------------------------------------------------------------------------
// Tests unitaires
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_from_filename() {
        let v = FirmwareVersion::from_filename("FirmwareBoardA-V01;18;10.hex").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 18);
        assert_eq!(v.patch, 10);
        assert_eq!(v.to_string(), "V01.18.10");
    }

    #[test]
    fn test_version_from_filename_custom() {
        assert!(FirmwareVersion::from_filename("my_custom_firmware.hex").is_none());
    }

    #[test]
    fn test_version_comparison() {
        let v1 = FirmwareVersion { major: 1, minor: 18, patch: 10 };
        let v2 = FirmwareVersion { major: 1, minor: 17, patch: 0 };
        let v3 = FirmwareVersion { major: 1, minor: 18, patch: 10 };

        assert!(v1 > v2);  // v1.18.10 > v1.17.0
        assert!(v2 < v1);  // v1.17.0 < v1.18.10
        assert!(v1 == v3); // v1.18.10 == v1.18.10
    }

    #[test]
    fn test_version_from_board_string() {
        let v = FirmwareVersion::from_board_string("V01.18.10").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 18);
        assert_eq!(v.patch, 10);
    }
}
