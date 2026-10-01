# Documentation de l'API REST - Borne Legrand GreenUp

Cette documentation décrit les points de terminaison (endpoints) exposés par le serveur web local de la borne (par défaut sur le port `8080`). Toutes les réponses sont retournées au format JSON.

## 📡 Endpoints de Lecture (GET)

### 1. Obtenir les informations matérielles et logicielles
- **URL** : `/api/info`
- **Méthode** : `GET`
- **Description** : Renvoie les informations d'identification de la carte de puissance (version firmware, numéro de série, date de production...). Ces informations sont interrogées une seule fois au démarrage de l'application et mises en cache.
- **Réponse type** :
  ```json
  {
    "software_version": "V01;18;04",
    "hardware_version": "V01",
    "serial_number": "12345678",
    "reference": "059000",
    "week_year_production": "42/20"
  }
  ```

### 2. Obtenir la télémétrie en temps réel
- **URL** : `/api/telemetry`
- **Méthode** : `GET`
- **Description** : Renvoie les données en temps réel de la borne. Ce point de terminaison est conçu pour être interrogé (polling) fréquemment par l'IHM (ex: toutes les 2 secondes).
- **Réponse type** :
  ```json
  {
    "voltage": 232.0,
    "current": 15.6,
    "power": 3619.2,
    "energy": 12.5,
    "frequency": 50.0,
    "state": "E",
    "error_code": "0000",
    "limit_amps": 16,
    "charge_complete": false,
    "tic_mode": "9600"
  }
  ```
  - *Note :* `tic_mode` peut valoir `"0"` (absent), `"1200"` (historique), `"9600"` (standard), `"detecting"` (en cours de détection), ou `""` (inconnu au démarrage).

---

## 🛠️ Endpoints d'Action (POST)

### 3. Démarrer la charge
- **URL** : `/api/charge/start`
- **Méthode** : `POST`
- **Description** : Autorise la charge sur la prise Type 2 (envoie la commande `T2COK` à la carte de puissance).
- **Réponse type** :
  ```json
  {
    "status": "success",
    "message": "Charge autorisée (Type 2)"
  }
  ```

### 4. Stopper la charge
- **URL** : `/api/charge/stop`
- **Méthode** : `POST`
- **Description** : Bloque la charge sur la prise Type 2 (envoie la commande `T2CNOK` à la carte de puissance).
- **Réponse type** :
  ```json
  {
    "status": "success",
    "message": "Charge stoppée (Type 2)"
  }
  ```

### 5. Modifier la limite de courant
- **URL** : `/api/current/:amps`
- **Méthode** : `POST`
- **Paramètre URL** : `:amps` (entier compris entre 10 et 32).
- **Description** : Définit la consigne matérielle de limite de courant (envoie la commande `CC:XX` à la carte de puissance). 
- **Sécurité** : L'API rejette toute valeur inférieure à 10A ou supérieure à 32A pour protéger le matériel.
- **Réponse type (Succès)** :
  ```json
  {
    "status": "success",
    "message": "Limite de courant définie sur 32A"
  }
  ```

### 6. Relancer la détection du TIC (Linky)
- **URL** : `/api/tic/refresh`
- **Méthode** : `POST`
- **Description** : Force une nouvelle tentative de détection de la vitesse du télé-information (TIC). Le serveur enverra la séquence `TICTM:1`, analysera le retour (ex: `TICTestB:9600`), mettra à jour la télémétrie, puis coupera le mode de test avec `TICTM:0`.
- **Sécurité** : Cette action n'est permise que si la borne est au repos absolu (`State: A`). Si la borne est branchée ou en charge, l'API renverra une erreur `400 Bad Request`.
- **Réponse type (Succès)** :
  ```json
  {
    "status": "success",
    "message": "Détection TIC lancée"
  }
  ```

### 7. Activer / Désactiver le Bluetooth
- **URL** : `/api/bluetooth`
- **Méthode** : `POST`
- **Body (JSON)** : 
  ```json
  { "enabled": true }
  ```
- **Description** : Permet d'allumer (`enabled: true`) ou d'éteindre (`enabled: false`) le module Bluetooth intégré à la borne en envoyant `BT:1` ou `BT:0`. Très utile pour éviter que l'application smartphone Legrand ne vienne écraser les consignes de la domotique.
- **Réponse type** :
  ```json
  {
    "status": "success",
    "message": "Bluetooth désactivé avec succès"
  }
  ```

### 8. Envoyer une commande brute (Debug/Avancé)
- **URL** : `/api/command`
- **Méthode** : `POST`
- **Body (Texte brut)** : `La commande à envoyer (ex: State?)`
- **Description** : Transmet directement la chaîne de caractères (avec un ajout automatique du `\r` final) sur le port série de la borne. Pratique pour les tests ou le debugging.
- **Réponse type** :
  ```json
  {
    "status": "success",
    "message": "Commande envoyée"
  }
  ```
