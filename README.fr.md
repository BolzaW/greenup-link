# Green'Up Link

[🇬🇧 Read in English](README.md)

> **Avertissement :** Green'Up Link est un projet communautaire indépendant et n'est ni affilié, ni sponsorisé, ni validé par Legrand. "Green'Up" et "Legrand" sont des marques déposées de leurs propriétaires respectifs.

## 📖 Présentation du projet

**Green'Up Link** est un logiciel léger, performant et indépendant écrit en Rust. Il s'installe directement sur le Kit de Communication officiel Legrand (référence 059056) pour remplacer le logiciel ("KitCom") fourni par le fabricant.

Il communique avec la carte de puissance de la borne via la liaison série interne et expose :
- Une **interface Web locale** épurée et réactive avec visualiseur animé, contrôle de charge (7A à 32A), statut Linky TIC et console live intégrée (flux de logs temps réel SSE).
- Une **API REST** complète et documentée, idéale pour l'intégration domotique (Home Assistant, Jeedom, Node-RED, etc.).
- Une **gestion complète des logs** avec fichiers journaliers tournants (`greenup_YYYY-MM-DD.log`), rétention automatique de 7 jours et modification à chaud des niveaux (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`).
- Une compatibilité totale avec **EVCC** pour la charge solaire dynamique (voir le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md)).

Il intègre également un pont MQTT direct avec le framework open-source EVerest, permettant de piloter la borne comme un module matériel EVerest standard. *(Note : Pour une implémentation native EVerest complète remplaçant EvseManager, consultez l'excellent projet communautaire [evorada/everest-greenup](https://github.com/evorada/everest-greenup) par suda).*

## 🤔 Pourquoi ce projet ?

Le logiciel officiel fourni avec le kit est lourd, peu ergonomique et difficile à interfacer avec un système domotique tiers.

De plus, il repose sur un Raspberry Pi 3 tournant sous un système d'exploitation obsolète (Raspbian 9 "Stretch"). Ce projet permet de reprendre le contrôle de votre matériel de manière saine, sécurisée et pérenne.

## ⚠️ Limitations actuelles

Le projet est en évolution continue :
- Gère uniquement la **charge directe**.
- Testé et validé sur la référence **058001 / 059001** (1 prise Type 2, pas de prise domestique, 7kW).
- **NE GÈRE PAS** le lecteur de badges RFID.
- **NE GÈRE PAS** les plages horaires (programmation horaire).
- **NE GÈRE PAS** la communication Modbus.
- **NE GÈRE PAS** le protocole OCPP.

## ✅ Compatibilité

- **Système d'exploitation** :
  - Image officielle Legrand "Raspbian GNU/Linux 9 (stretch)" (32-bit ARMv7).
  - Raspberry Pi OS moderne (32-bit `armv7` et 64-bit `aarch64`).
- **Firmware de la carte de puissance** :
  - `FirmwareBoardA-V01;18;10.hex` (Testé et validé)
  - `FirmwareBoardA-V01;18;04.hex` (Théoriquement compatible)
  - `FirmwareBoardA-V01;17;27.hex` (Théoriquement compatible)

---

## 🛠️ Compilation du projet (Pour les développeurs)

Le projet est structuré sous forme de Cargo workspace et supporte la compilation croisée pour ARM 32-bit (Debian Stretch historique) et ARM 64-bit moderne.

Des scripts prêts à l'emploi sont disponibles dans le dossier [`builds/`](builds/) :

| Script | Architecture cible | Type de build | Binaire / Archive produite |
| :--- | :--- | :--- | :--- |
| **`builds/release-legacy.bat`** | ARM 32-bit (`armv7`) | Release (glibc 2.24) | `target/armv7-unknown-linux-gnueabihf/release/greenup-link` + `.zip` |
| **`builds/release-aarch64.bat`** | ARM 64-bit (`aarch64`) | Release (Standard) | `target/aarch64-unknown-linux-gnu/release/greenup-link` + `.zip` |

Pour une compilation manuelle sous WSL ou Linux :
```bash
# Pour Raspberry Pi OS 64-bit moderne (Pi 3 / 4 / 5) :
cargo build --release -p greenup-link --target aarch64-unknown-linux-gnu

# Pour l'image Legrand 32-bit historique (glibc 2.24 / Debian Stretch) :
cargo zigbuild --release -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24
```
Pour le détail de l'environnement, consultez le [Guide de compilation 64-bit](docs/ENV_COMPILATION_AARCH64.md).

---

## ⚙️ Installation sur la borne (Raspberry Pi)

> **AVERTISSEMENT :** Faites une sauvegarde complète de la carte SD d'origine Legrand avant toute manipulation.

> ⚠️ **Important :** Toute intervention physique sur la borne doit obligatoirement se faire **hors tension (disjoncteur coupé)**.

### 1. Initialisation du kit Legrand
Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.

### 2. Accès SSH (Modification de la carte SD)
L'accès SSH étant désactivé par défaut sur l'image d'origine :

1. Coupez le disjoncteur et retirez la carte microSD du Raspberry Pi.
2. **Activer SSH** : Créez un fichier vide nommé `ssh` (sans extension) à la racine de la partition `boot`.
3. **Réinitialiser le mot de passe** :
   - Sur votre PC (Linux/WSL), générez un hash : `mkpasswd -m sha-512`
   - Ouvrez la partition `rootfs` et éditez `/etc/shadow`.
   - Remplacez le hash de la ligne commençant par `pi:` par celui que vous venez de générer.
4. Réinsérez la carte SD et réenclenchez le disjoncteur.
5. Connectez-vous en SSH : `ssh pi@<ip_borne>` avec le mot de passe défini (`pi` dispose des droits sudo).

### 3. Sécurisation de l'accès SSH
⚠️ **NOTE DE SÉCURITÉ :** Le Raspberry Pi tourne sur un OS ancien. Ne l'exposez **JAMAIS** directement à Internet (aucune redirection de port sur la box/routeur).

1. **Changez le mot de passe root** : `sudo passwd root`
2. **Déployez votre clé publique SSH** :
   ```bash
   # Depuis votre PC :
   ssh-copy-id pi@<ip_borne>
   ```
3. Désactivez l'authentification par mot de passe dans `/etc/ssh/sshd_config` (`PasswordAuthentication no`, `PermitRootLogin no`), puis rechargez le démon SSH : `sudo systemctl reload ssh`.

### 4. Installation automatisée de Green'Up Link

#### Option A : Commande unique rapide (Détection d'architecture et téléchargement automatique)
Si le Raspberry Pi dispose d'un accès à Internet, lancez directement le script d'installation :
```bash
ssh pi@<ip_borne>
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash
```
> [!TIP]
> Le script détecte automatiquement l'architecture de votre système (`armv7` 32-bit legacy ou `aarch64` 64-bit), interroge la dernière release GitHub, télécharge et extrait le binaire adéquat directement dans `/home/pi/greenup/`.

Pour mettre à jour une installation existante vers la dernière release GitHub, relancez avec l'option `--update` :
```bash
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash -s -- --update
```

#### Option B : Installation hors-ligne / Transfert manuel (SCP)
1. **Transférez le binaire et l'installateur** vers la borne depuis votre PC via SCP :
   ```bash
   # Depuis le dossier de vos binaires :
   scp greenup-link scripts/install.sh pi@<ip_borne>:~/
   ```
   *(Ou depuis le dossier source : `scp target/armv7-unknown-linux-gnueabihf/release/greenup-link scripts/install.sh pi@<ip_borne>:~/`)*

2. **Connectez-vous en SSH et lancez l'installateur** :
   ```bash
   ssh pi@<ip_borne>
   chmod +x install.sh
   sudo ./install.sh
   ```

---

Le script d'installation configure l'ensemble du système automatiquement :
- Installe `greenup-link` dans `/home/pi/greenup/` avec les permissions d'exécution appropriées (téléchargé automatiquement depuis GitHub ou déplacé depuis le dossier local).
- Arrête et désactive les services d'origine Legrand (serveur Apache Tomcat 9 et daemons Java).
- Neutralise le script de démarrage Legrand (`/etc/init.d/CommunicationArduinoRasp`) avec un `exit 0` de sécurité (sauvegarde en `.old`).
- Neutralise le script d'auto-destruction Legrand (`DeletAll.sh`) avec un `exit 0` de sécurité et retire les droits d'exécution (sauvegarde en `.old`).
- Crée et active le service Systemd `/etc/systemd/system/greenup-link.service`.
- Démarre immédiatement le service `greenup-link`.

### 5. Gestion du service

- **Vérifier l'état du service** :
  ```bash
  sudo systemctl status greenup-link
  ```
- **Afficher les logs en temps réel** :
  ```bash
  sudo journalctl -u greenup-link -f
  ```
- **Redémarrer le service** :
  ```bash
  sudo systemctl restart greenup-link
  ```
- **Arrêter le service** :
  ```bash
  sudo systemctl stop greenup-link
  ```

### 6. Utilisation
- **Interface Web** : Ouvrez `http://<ip_borne>:8080` dans votre navigateur.
- **API REST** : Consultez la [Documentation de l'API REST](docs/GREENUP_LINK_REST_API.md) pour l'intégration avec Home Assistant, Jeedom, Node-RED, etc.
- **Client CLI interactif** : Utilisez `scripts/cli-greenup-link.ps1` sous PowerShell Windows pour piloter ou tester la borne.
- **Intégration EVCC** : Consultez le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md) pour la charge solaire dynamique.
- **Module EVerest** : Consultez la [Documentation EVerest](crates/greenup-everest/README.md) pour l'intégration native MQTT.

---

## 🚀 Fonctionnalités futures

- Intégration MQTT directe pour la domotique
- Compatibilité avec les versions récentes de Raspberry Pi OS
- Firmware custom ATmega avec télémétrie étendue
- Prise en charge du protocole OCPP
- Extension aux autres variantes de la gamme (prise domestique Schuko, triphasé, double point de charge)
