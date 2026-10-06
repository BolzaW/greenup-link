# Documentation de l'API REST - Borne Legrand GreenUp

Cette documentation décrit les points de terminaison (endpoints) exposés par le serveur web local `greenup-link` (par défaut sur le port `8080`). Toutes les réponses sont retournées au format JSON.

En cas d'erreur, la réponse a la forme `{ "error": "<message>" }` avec un code HTTP `400` (requête refusée) ou `500` (communication série impossible).

| Méthode | URL | Rôle |
| :--- | :--- | :--- |
| `GET` | `/api/info` | Informations matérielles et capacités de la borne |
| `GET` | `/api/telemetry` | Télémétrie temps réel + états Legrand / IEC |
| `POST` | `/api/charge/start` | Reprise de charge (`SBOK`) |
| `POST` | `/api/charge/stop` | Pause de charge (`SBNOK`) |
| `POST` | `/api/t2/enable` | Activation logicielle de la prise T2 (`T2COK`) |
| `POST` | `/api/t2/disable` | Désactivation logicielle de la prise T2 (`T2CNOK`) |
| `POST` | `/api/current/:amps` | Limite de courant (`CC:XX`) |
| `POST` | `/api/init` | Relance de la séquence de démarrage |
| `POST` | `/api/reset` | Redémarrage matériel de la carte ATmega (`Reset`) |
| `POST` | `/api/tic/refresh` | Détection du TIC (uniquement en `State:A`) |
| `POST` | `/api/bluetooth` | Activation / désactivation du Bluetooth |
| `POST` | `/api/command` | Commande série brute (debug) |

---

## 📡 Endpoints de Lecture (GET)

### 1. Obtenir les informations matérielles et logicielles
- **URL** : `/api/info`
- **Méthode** : `GET`
- **Description** : Renvoie les informations d'identification de la carte de puissance (version firmware, numéro de série, date de production...) ainsi que les capacités matérielles déduites de la référence (`capabilities`). Ces informations sont interrogées au démarrage et mises en cache.
- **Réponse type** :
  ```json
  {
    "software_version": "V01.18.10",
    "hardware_version": "V02.01.01",
    "serial_number": "015184",
    "reference": "058001",
    "week_year_production": "42W22",
    "bluetooth_enabled": false,
    "link_version": "0.1.0",
    "capabilities": {
      "name": "Green'up Premium 3,7–7,4 kW T2",
      "reference": "058001",
      "phases": 1,
      "max_power_kw": 7.4,
      "max_current_amps": 32,
      "has_schuko": false,
      "charging_points": 1,
      "is_known": true
    }
  }
  ```
  - *Note :* `capabilities` vaut `null` tant que la référence n'a pas été reçue.

### 2. Obtenir la télémétrie en temps réel
- **URL** : `/api/telemetry`
- **Méthode** : `GET`
- **Description** : Renvoie les données temps réel de la borne. Conçu pour être interrogé fréquemment (polling ~2 s par l'IHM). Les appels sont journalisés au niveau `TRACE` (fichier de log uniquement, pas la console).
- **Réponse type** :
  ```json
  {
    "voltage": 232.0,
    "current": 15.6,
    "power": 3619.2,
    "energy": 12.5,
    "frequency": 50.0,
    "greenup_state": "E",
    "error_code": "0000",
    "limit_amps": 16,
    "eliot_limit_amps": 16,
    "cp_voltage": 6,
    "t2c_enabled": true,
    "sb_state": true,
    "iec_state": "Charging_C",
    "charge_complete": false,
    "tic_mode": "9600"
  }
  ```
  - `greenup_state` : état propriétaire Legrand (`A`, `B`, `C`, `D`, `E`, `I`, `W`, `M`, `L`, `R`, `X`, `V`). Voir `docs/MACHINE_A_ETATS.md`.
  - `iec_state` : état normalisé déduit (niveau 2) : `Disconnected_A`, `Connected_B`, `Charging_C`, `Error_E`, `Faulted_F`. Vaut `null` tant qu'il ne peut pas être déduit.
  - `cp_voltage` (`CP?`) : tension Control Pilot en volts (`12`, `9`, `6`).
  - `t2c_enabled` (`T2C?`) / `sb_state` (`SB?`) : états logiciels de la prise T2 et du bouton Start/Stop.
  - `eliot_limit_amps` (`CCEl?`) : limite de courant effective appliquée par la borne.
  - `tic_mode` : `"0"` (absent), `"1200"` (historique), `"9600"` (standard), `"detecting"` (détection en cours) ou `""` (inconnu, aucune détection lancée).

---

## 🛠️ Endpoints d'Action (POST)

### 3. Reprendre la charge
- **URL** : `/api/charge/start`
- **Méthode** : `POST`
- **Description** : Simule un appui sur le bouton START (envoie `SBOK`). Depuis `State:M`, la borne repasse en `A` → `B` → `C`.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Charge autorisée (Type 2)" }
  ```

### 4. Mettre la charge en pause
- **URL** : `/api/charge/stop`
- **Méthode** : `POST`
- **Description** : Simule un appui sur le bouton STOP (envoie `SBNOK`). La borne passe en `State:W` puis se stabilise en `State:M` (arrêt manuel) sans aveugler la machine à états.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Charge stoppée (Type 2)" }
  ```

### 5. Activer la prise Type 2
- **URL** : `/api/t2/enable`
- **Méthode** : `POST`
- **Description** : Réactive logiciellement la prise Type 2 (envoie `T2COK`).
- **Réponse type** :
  ```json
  { "status": "success", "message": "Prise activée (T2COK)" }
  ```

### 6. Désactiver la prise Type 2
- **URL** : `/api/t2/disable`
- **Méthode** : `POST`
- **Description** : Désactive logiciellement la prise Type 2 (envoie `T2CNOK`).
- **⚠️ Attention** : avec `T2C:0`, la machine à états Legrand est figée en `State:A` (Quirk #3). L'état IEC est alors déduit uniquement de `cp_voltage`.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Prise désactivée (T2CNOK)" }
  ```

### 7. Modifier la limite de courant
- **URL** : `/api/current/:amps`
- **Méthode** : `POST`
- **Paramètre URL** : `:amps` (entier compris entre **7** et **32**).
- **Description** : Définit la consigne de limite de courant (envoie `CC:XX`, toujours formaté sur 2 chiffres).
- **Sécurité** : Toute valeur hors `[7, 32]` est rejetée (`400`). Sous 6 A la borne plante (`E:0010`), et l'électronique ne régule pas en dessous de ~6,8 A (Quirk #6).
- **Réponse type** :
  ```json
  { "status": "success", "message": "Limite de courant définie sur 16A" }
  ```

### 8. Relancer la séquence d'initialisation
- **URL** : `/api/init`
- **Méthode** : `POST`
- **Description** : Rejoue la séquence de démarrage : `RaspberryPiModeOK`, puis interrogation des informations (`SoftwareVersion?`, `HardwareVersion?`, `SerialNumber?`, `Reference?`, `WeekYearProduction?`) et des états (`State?`, `FM?`, `CC?`, `CCEl?`, `CP?`, `T2C?`, `SB?`, `E?`, `BT?`). La réponse est immédiate, la séquence s'exécute en tâche de fond.
- **Note** : la détection TIC n'est **jamais** lancée automatiquement. Elle doit être demandée explicitement via `/api/tic/refresh`.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Séquence d'initialisation lancée" }
  ```

### 9. Redémarrer la carte de puissance
- **URL** : `/api/reset`
- **Méthode** : `POST`
- **Description** : Envoie la commande `Reset` à la carte ATmega pour forcer un redémarrage matériel. Utile pour débloquer la borne lorsqu'elle reste figée en `State:A` alors que `T2C:1` et `CP:9` (véhicule branché) : l'état `B` n'arrive jamais sans reset.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Redémarrage de la carte ATmega demandé" }
  ```

### 10. Lancer la détection du TIC (Linky)
- **URL** : `/api/tic/refresh`
- **Méthode** : `POST`
- **Description** : Lance la détection de la vitesse de la télé-information client. Le serveur envoie `TICTM:1`, analyse le retour (ex : `TICTestB:9600`), met à jour `tic_mode`, puis coupe le mode test avec `TICTM:0`.
- **Sécurité** : la détection n'est autorisée que si la borne est au repos (`greenup_state == "A"`). Dans tout autre état, **rien n'est envoyé à la borne** et l'API répond `400` :
  ```json
  { "error": "La détection TIC ne peut se faire que lorsque la borne est libre (State:A). État actuel : M" }
  ```
- **Réponse type (Succès)** :
  ```json
  { "status": "success", "message": "Détection TIC lancée" }
  ```

### 11. Activer / Désactiver le Bluetooth
- **URL** : `/api/bluetooth`
- **Méthode** : `POST`
- **Body (JSON)** :
  ```json
  { "enabled": true }
  ```
- **Description** : Allume (`enabled: true`) ou éteint (`enabled: false`) le module Bluetooth de la borne (`BTOK` / `BTNOK`). Utile pour empêcher l'application smartphone Legrand d'écraser les consignes de la domotique.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Bluetooth désactivé avec succès" }
  ```

### 12. Envoyer une commande brute (Debug/Avancé)
- **URL** : `/api/command`
- **Méthode** : `POST`
- **Body (Texte brut)** : la commande à envoyer (ex : `State?`)
- **Description** : Transmet directement la chaîne (avec ajout automatique du `\r` final) sur le port série. Réservé aux tests et au debugging.
- **Réponse type** :
  ```json
  { "status": "success", "message": "Commande envoyée" }
  ```
