# Green'Up Link

[🇬🇧 Read in English](README.md)

> [!CAUTION]
> ### ⚡ Avertissement de Sécurité Électrique & Décharge Totale de Responsabilité
> 
> **Danger Haute Tension (230V / 400V) :**
> Des tensions électriques mortelles sont présentes à l'intérieur de la borne de recharge. **Toute intervention physique, ouverture du boîtier, manipulation du matériel ou câblage doit IMPÉRATIVEMENT et STRICTEMENT s'effectuer HORS TENSION, avec l'alimentation électrique générale coupée au disjoncteur du tableau principal.** Vérifiez systématiquement l'absence réelle de tension avant d'ouvrir la borne. Si vous n'êtes pas un professionnel qualifié pour les interventions électriques IRVE, ne tentez pas d'ouvrir la borne.
> 
> **Exclusion et décharge intégrale de responsabilité (Logiciel fourni "EN L'ÉTAT") :**
> - **Green'Up Link** est un projet communautaire open-source fourni **« EN L'ÉTAT »**, sans aucune garantie d'aucune sorte, expresse ou implicite (y compris, sans s'y limiter, les garanties d'adéquation à un usage particulier, de sécurité ou de bon fonctionnement).
> - L'auteur, les développeurs et les contributeurs déclinent formellement **toute responsabilité en cas d'accident corporel (électrisation, électrocution, blessure grave, décès), d'incendie, de départ de feu, de court-circuit, de dégradation matérielle (de la borne, du véhicule, de la batterie de traction ou du tableau électrique), de perte de garantie constructeur ou de tout dommage direct ou indirect** consécutif à l'installation, à l'utilisation, à la modification ou à l'incapacité d'utiliser ce logiciel ou sa documentation.
> - **L'utilisateur final assume l'entière, totale et exclusive responsabilité de l'utilisation de ce logiciel et de toute manipulation effectuée sur son matériel.**
> 
> **Marques déposées :** Green'Up Link est un projet communautaire indépendant et n'est ni affilié, ni sponsorisé, ni approuvé par Legrand. "Green'Up" et "Legrand" sont des marques déposées de leurs propriétaires respectifs.

## 📖 Présentation du projet

**Green'Up Link** est un logiciel léger, performant et indépendant écrit en Rust. Il peut s'installer directement sur le Kit de Communication officiel Legrand (référence 059056) pour remplacer le logiciel ("KitCom") fourni par le fabricant, ou sur un **Raspberry Pi standard** (Pi Zero 2 W, Pi 3, Pi 4...) équipé d'une image officielle Raspberry Pi OS 64-bit Lite et relié via un câble USB (voir le [Guide du kit de communication alternatif DIY](docs/KIT_COMMUNICATION_DIY.md)).

Il communique avec la carte de puissance de la borne via un câble USB interne et expose :
- Une **interface Web locale** épurée et réactive avec visualiseur animé, contrôle de charge (7A à 32A), statut Linky TIC et console live intégrée (flux de logs temps réel SSE).
- Une **API REST** complète et documentée, idéale pour l'intégration domotique (Home Assistant, Jeedom, Node-RED, etc.).
- Une **gestion complète des logs** avec fichiers journaliers tournants (`greenup_YYYY-MM-DD.log`), rétention automatique de 7 jours et modification à chaud des niveaux (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`).
- Une compatibilité totale avec **EVCC** pour la charge solaire dynamique (voir le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md)).

*(Expérimental / Démonstrateur)* : Un module `greenup-everest` autonome est également proposé à titre de démonstrateur expérimental pour interfacer la borne avec le framework open-source EVerest via MQTT (il ne constitue pas le cœur du projet ; voir également le projet [evorada/everest-greenup](https://github.com/evorada/everest-greenup)).

## 🤔 Pourquoi ce projet ?

Le logiciel officiel fourni avec le kit est lourd, peu ergonomique et difficile à interfacer avec un système domotique tiers.

De plus, la pile logicielle d'origine Legrand (exécutant des versions obsolètes d'Apache Tomcat 9, un runtime Java ancien et des composants non maintenus sur Raspbian 9 "Stretch") comporte de **multiples vulnérabilités et CVE critiques**. Exposer le logiciel d'origine directement à Internet (ou même sur un réseau local non cloisonné) est extrêmement dangereux. Green'Up Link remplace complètement cette pile vulnérable par un unique binaire Rust autonome, sécurisé, sans runtime lourd, vous redonnant la pleine maîtrise de votre matériel.

## ⚠️ Limitations actuelles

Le projet est en évolution continue :
- **Périmètre du logiciel** : Green'Up Link ne remplace que le kit de communication (la couche logicielle exécutée sur le Raspberry Pi). La borne conserve l'intégralité de son intelligence native, de ses sécurités électriques et de son fonctionnement autonome via sa carte de puissance interne ATmega2560.
- **Matériel testé** : Je ne peux tester et valider le logiciel que sur mon propre matériel : référence **058001** (1 prise Type 2, 7.4 kW monophasé, sans prise domestique).
- **Compatibilité Type 2** : Toutes les bornes de la gamme Green'up Premium devraient fonctionner de manière nominale sur leur prise Type 2.
- **Prise domestique (Schuko) et Triphasé** : Non prises en charge pour l'instant (manque de connaissances, de logs de communication série et de matériel de test pour investiguer ces configurations).
- **Charge directe uniquement** : Pas de gestion de plages horaires intégrées (confiez la programmation horaire à Home Assistant ou EVCC).
- **Ne gère pas** : Lecteur de badges RFID, communication Modbus, protocole OCPP.

## ✅ Compatibilité

- **Système d'exploitation** :
  - Image officielle Legrand historique : "Raspbian GNU/Linux 9 (stretch)" 32-bit (`armv7`, glibc 2.24). *(Attention : il n'existe pas de version 32-bit moderne ; la version 32-bit legacy est dédiée à cette image).*
  - Raspberry Pi OS moderne : 64-bit (`aarch64`, Bullseye / Bookworm) Lite ou Desktop.
- **Modèles de bornes** : Consultez la [Matrice de compatibilité matériel](docs/COMPATIBLE_HARDWARE.md) pour la liste exhaustive des références Legrand et leur niveau de prise en charge (Total, Partiel ou Aucun).
- **Firmware de la carte de puissance** :
  - `FirmwareBoardA-V01;18;10.hex` (Testé et validé)
  - `FirmwareBoardA-V01;17;27.hex` (Testé et validé)
  - `FirmwareBoardA-V01;18;04.hex` (Théoriquement compatible)

> [!NOTE]
> **À propos des images système Legrand :** Il est possible qu'il existe d'autres versions d'images d'usine Legrand plus récentes dont les spécifications exactes ne sont pas connues. Legrand ne prévoyant aucun mécanisme de migration d'OS (seule la suite applicative et quelques paquets Debian sont mis à jour via le mécanisme de mise à jour Legrand), certaines versions d'images pourraient présenter des incompatibilités avec le binaire fourni. Le cas échéant, ouvrez un ticket avec les informations de votre système (`uname -a`, `ldd --version`) : une nouvelle chaîne de compilation adaptée pourra être mise en place.

---

## 🛠️ Compilation du projet (Pour les développeurs)

Le projet est structuré sous forme de Cargo workspace et supporte la compilation croisée pour ARM 32-bit (Debian Stretch historique) et ARM 64-bit moderne.

Des scripts prêts à l'emploi sont disponibles dans le dossier [`builds/`](builds/). **Remarque :** Ces scripts batch sont conçus pour être exécutés sous Windows via **WSL (Ubuntu)** avec les chaînes de compilation croisée configurées.

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
Pour le détail de l'environnement, consultez le [Guide d'environnement de compilation](docs/ENV_COMPILATION_AARCH64_ARM7.md).

---

## ⚙️ Installation sur la borne (Raspberry Pi)

> **AVERTISSEMENT :** Faites une sauvegarde complète de la carte SD d'origine Legrand avant toute manipulation.

> ⚠️ **Important :** Toute intervention physique sur la borne doit obligatoirement se faire **hors tension (disjoncteur coupé)**.

### 1. Initialisation du kit Legrand
Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.

*(Si vous ne possédez pas le kit officiel Legrand 059056, consultez le [Guide du kit de communication alternatif DIY](docs/KIT_COMMUNICATION_DIY.md) pour concevoir un montage matériel équivalent avec un Raspberry Pi standard).*

### 2. Accès SSH (Modification de la carte SD)

> [!NOTE]
> **Travaux en cours :** Des outils et procédures automatisées sont actuellement à l'étude pour permettre l'activation du SSH directement à distance via le réseau, sans nécessiter de démontage physique de la borne ni d'extraction de la carte SD.

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

### 4. Installation de Green'Up Link

#### Releases GitHub & Architectures supportées
Des paquets pré-compilés et binaires autonomes sont disponibles sur la [page des Releases GitHub](https://github.com/BolzaW/greenup-link/releases) :

| Fichier Release | Architecture / Cible | Système visé | Contenu |
| :--- | :--- | :--- | :--- |
| **`greenup-link-vX.Y.Z-armv7-legacy.zip`** | ARMv7 32-bit (`armv7-unknown-linux-gnueabihf.2.24`) | **Kit officiel Legrand** (Debian 9 Stretch, glibc 2.24) et Raspberry Pi OS 32-bit | `greenup-link`, `install.sh`, `README.md`, `README.fr.md` |
| **`greenup-link-vX.Y.Z-aarch64.zip`** | AArch64 64-bit (`aarch64-unknown-linux-gnu`) | **Raspberry Pi OS 64-bit Lite officiel** (Pi Zero 2 W, Pi 3, Pi 4, Pi 5) | `greenup-link`, `install.sh`, `README.md`, `README.fr.md` |
| **`greenup-link-armv7-legacy`** | ARMv7 32-bit | Binaire autonome direct (compatible glibc 2.24) | Binaire seul |
| **`greenup-link-aarch64`** | AArch64 64-bit | Binaire autonome direct | Binaire seul |
| **`SHA256SUMS.txt`** | - | Empreintes SHA256 pour vérification d'intégrité | Fichier texte |

#### Option A : Commande unique rapide (Détection d'architecture et téléchargement automatique)
Si le Raspberry Pi dispose d'un accès à Internet, lancez directement le script d'installation :
```bash
ssh pi@<ip_borne>
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash
```
> [!TIP]
> - Le script détecte automatiquement l'architecture de votre système (`armv7` 32-bit legacy ou `aarch64` 64-bit), interroge la dernière release GitHub, télécharge directement le binaire autonome adéquat et l'installe dans `/home/pi/greenup/`.
> - **Pour mettre à jour :** Relancez simplement la même commande 1-liner à tout moment. Le script détectera l'installation existante, téléchargera la dernière release et redémarrera le service automatiquement.

#### Option B : Installation manuelle / Hors-ligne (Archive ZIP Release)
Si la borne n'a pas d'accès direct à Internet :
1. **Téléchargez l'archive ZIP appropriée** depuis la [page des Releases GitHub](https://github.com/BolzaW/greenup-link/releases) sur votre PC :
   - Pour le kit officiel Legrand : `greenup-link-vX.Y.Z-armv7-legacy.zip`
   - Pour Raspberry Pi OS 64-bit moderne : `greenup-link-vX.Y.Z-aarch64.zip`
2. **Transférez l'archive vers la borne** via SCP :
   ```bash
   scp greenup-link-v*.zip pi@<ip_borne>:~/
   ```
3. **Connectez-vous en SSH, décompressez et lancez l'installateur** :
   ```bash
   ssh pi@<ip_borne>
   unzip greenup-link-v*.zip
   chmod +x install.sh
   sudo ./install.sh
   ```
   *(Note : L'archive contient déjà le script `install.sh` et le binaire `greenup-link` adapté. Si `unzip` n'est pas installé sur la borne, décompressez l'archive sur votre PC et transférez les fichiers extraits).*

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

### 7. Rollback & Désinstallation (Restauration de la stack Legrand)

Si vous souhaitez revenir au logiciel officiel Legrand ou désinstaller Green'Up Link, un script de rollback / désinstallation automatisé est fourni.

- **Lancement direct depuis la borne** (installé automatiquement avec Green'Up Link) :
  ```bash
  sudo /home/pi/greenup/uninstall.sh
  ```
- **Ou via commande unique en ligne** :
  ```bash
  curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/uninstall.sh | sudo bash
  ```

Le script vous propose interactivement de choisir entre :
1. **Désactiver uniquement (Conserver les fichiers)** : Arrête et supprime le service `greenup-link.service`, tout en préservant le dossier `/home/pi/greenup/` et l'historique des logs.
2. **Purge complète** : Arrête le service et supprime intégralement le dossier `/home/pi/greenup/`.

Dans les deux cas, le script réactive automatiquement l'environnement officiel Legrand :
- Restaure le script `/etc/init.d/CommunicationArduinoRasp` depuis sa sauvegarde et le réenregistre au démarrage.
- Restaure les scripts d'urgence `DeletAll.sh` neutralisés et rétablit leurs droits d'exécution.
- Réactive et redémarre le serveur Apache Tomcat 9 et le daemon Java Legrand.

---

## 🚀 Fonctionnalités futures

Les fonctionnalités envisagées (sans calendrier contractuel) :
- **Intégration MQTT native** avec **Home Assistant Auto-Discovery** (création automatique des entités : capteurs de télémétrie, interrupteurs et curseur de consigne de charge).
- **Historique des sessions de charge** : Sauvegarde des sessions au format CSV, avec visualisation de l'historique et export CSV directement depuis l'interface Web.
- Firmware custom ATmega avec télémétrie et capacités étendues.
- Prise en charge du protocole OCPP.
- Extension aux autres variantes de la gamme (prise domestique Schuko, bornes triphasées, bornes à double point de charge).

---

## 🤝 Appel à testeurs & Contributions

Je recherche activement des testeurs disposant de configurations matérielles différentes de la gamme Legrand Green'up Premium (par exemple des bornes équipées d'une prise domestique Schuko, des modèles triphasés, des modèles à double point de charge ou avec d'autres révisions de cartes électroniques / firmwares).

L'objectif est de collecter des logs de communication série pour comprendre le fonctionnement de ces variantes et étendre le support de Green'Up Link à l'ensemble de la gamme.

Si vous disposez d'un de ces modèles et souhaitez participer aux tests ou partager vos retours :
- Ouvrez un ticket sur **[GitHub Issues](https://github.com/BolzaW/greenup-link/issues)**
- Ou contactez-moi directement par email : **[bolzaw.dev@gmail.com](mailto:bolzaw.dev@gmail.com)**
