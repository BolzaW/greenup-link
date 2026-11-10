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
- Un **historique des sessions de charge** avec enregistrement automatique haute fidélité (échantillonnage adaptatif au format CSV et résumé TXT), visualiseur graphique de courbe de puissance et export direct en un clic (CSV, TXT, ZIP) depuis l'interface Web ou l'API REST.
- Une **gestion complète des logs** avec fichiers journaliers tournants (`greenup_YYYY-MM-DD.log`), rétention automatique de 7 jours et modification à chaud des niveaux (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`).
- Le **support expérimental des bornes à double point de charge** avec auto-détection dynamique du second port USB et contrôle indépendant des deux côtés (*théorique, non testé sur matériel réel* — voir la [Matrice de compatibilité matériel](docs/COMPATIBLE_HARDWARE.md)).
- Un client **MQTT natif** avec **Auto-Discovery Home Assistant** : intégration plug-and-play immédiate des capteurs, interrupteurs, curseurs de courant et Dashboard Énergie sans aucune configuration YAML manuelle (voir le [Guide d'intégration Home Assistant](docs/HOME_ASSISTANT_INTEGRATION.md) et le [Guide MQTT](docs/GREENUP_LINK_MQTT.md)).
- Une compatibilité totale avec **EVCC** via MQTT ou API REST pour la charge solaire dynamique monosite ou multisite (voir le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md)).

*(Expérimental / Démonstrateur)* : Un module `greenup-everest` autonome est également proposé à titre de démonstrateur expérimental pour interfacer la borne avec le framework open-source EVerest via MQTT (il ne constitue pas le cœur du projet ; voir également le projet [evorada/everest-greenup](https://github.com/evorada/everest-greenup)).

> [!NOTE]
> **Applications mobiles & Interface Bluetooth :**
> Green'Up Link ne remplace **pas** les applications mobiles officielles Legrand (**EV Charge Light** ou la nouvelle génération **EV Charge** sortie début 2026), qui communiquent directement avec la borne en Bluetooth. Si vous utilisez l'une de ces applications mobiles et ne savez pas si votre borne est équipée du kit de communication, c'est que vous ne le possédez très probablement pas : le kit de communication IP (référence 059056) est en effet un accessoire vendu en option.
> 
> Un travail complet de rétro-ingénierie du protocole Bluetooth a été réalisé (voir la [Documentation du protocole Bluetooth](docs/LEGRAND_BLE_PROTOCOL.md)). Toutefois, ce protocole n'est pour l'instant pas exploité par Green'Up Link car la liaison série USB physique interne s'avère nettement plus polyvalente et fiable pour la supervision domotique permanente. Cette documentation est fournie à titre indicatif et pourra servir à d'autres projets communautaires.

## 🤔 Pourquoi ce projet ?

Les bornes de recharge Legrand Green'Up Premium bénéficient d'une conception matérielle soignée : la qualité de fabrication industrielle, la robustesse du boîtier et le niveau de sécurité matérielle semblent solides et fiables pour un usage quotidien. C'est un bon matériel qui fonctionne bien.

En revanche, le contraste avec la partie logicielle d'origine est criant. Le logiciel constructeur fourni avec le kit de communication ("KitCom") date d'une autre époque : il s'avère lourd, peu réactif et difficile à interfacer avec des solutions domotiques modernes. De plus, il repose sur un environnement vieillissant (Raspbian 9 "Stretch", Apache Tomcat 9 et un runtime Java non maintenus accumulant de nombreuses vulnérabilités connues).

Green'Up Link est né de l'envie d'offrir à ce bon matériel la couche logicielle moderne, légère, sécurisée et ouverte qu'il mérite : un unique binaire écrit en Rust, ultra-performant, sans runtime lourd, vous redonnant la pleine maîtrise locale de votre borne.

## ⚠️ Limitations actuelles

Le projet est en évolution continue :
- **Périmètre du logiciel** : Green'Up Link ne remplace que le kit de communication (la couche logicielle exécutée sur le Raspberry Pi). La borne conserve l'intégralité de son intelligence native, de ses sécurités électriques et de son fonctionnement autonome via sa carte de puissance interne ATmega2560.
- **Matériel testé** : Validé sur la borne physique de l'auteur : référence **058001** (1 prise Type 2, 7.4 kW monophasé, sans prise domestique).
- **Compatibilité Type 2** : Toutes les bornes de la gamme Green'up Premium devraient fonctionner de manière nominale sur leur prise Type 2.
- **Bornes à double point de charge** : Pleinement implémenté dans le logiciel (double worker série, détection dynamique des ports USB, sessions indépendantes), mais reste **expérimental et non testé sur le terrain** faute de matériel (avis aux testeurs et retours de bugs !).
- **Prise domestique (Schuko) et Triphasé** : Théoriquement fonctionnel dans Green'Up Link sur la base des spécifications du firmware, mais reste **expérimental et non testé sur du matériel réel** faute de disposer de ces modèles. N'hésitez pas à remonter vos retours ou problèmes !
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

> [!TIP]
> **Recommandation d'OS pour un usage pérenne :**  
> Même si faire tourner Green'Up Link directement sur l'image d'usine Legrand est totalement supporté et encouragé pour tester ou faire un essai temporaire (sans toucher à la carte SD constructeur), **l'installation sur une image propre, récente et vierge de Raspberry Pi OS 64-bit Lite (sur une nouvelle carte microSD) est vivement recommandée pour une utilisation pérenne au quotidien**. Cela vous garantit un noyau Linux maintenu, des correctifs de sécurité modernes et les performances optimales du 64-bit.

> **AVERTISSEMENT :** Faites une sauvegarde complète de la carte SD d'origine Legrand avant toute manipulation.
>
> ⚠️ **Important :** Toute intervention physique sur la borne doit obligatoirement se faire **hors tension (disjoncteur coupé)**.

💡 **Accès rapide (Kit DIY / OS déjà configuré) :** Les chapitres **1 à 3** ci-dessous sont dédiés au **kit de communication officiel Legrand (059056)**. Si vous avez déjà un **Raspberry Pi avec accès SSH** connecté en USB dans la borne ([kit DIY](docs/KIT_COMMUNICATION_DIY.md) ou autre OS standard), vous pouvez aller directement au chapitre **[4. Installation de Green'Up Link](#4-installation-de-greenup-link)**.

### 1. Initialisation du kit Legrand
Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.

*(Si vous ne possédez pas le kit officiel Legrand 059056, consultez le [Guide du kit de communication alternatif DIY](docs/KIT_COMMUNICATION_DIY.md) pour concevoir un montage matériel équivalent avec un Raspberry Pi standard — alimenté en interne via convertisseur DC/DC ou simplement via une alimentation externe standard).*

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

### 4. Installation de Green'Up Link

> [!IMPORTANT]
> **Bonne pratique de sécurité :** Exécuter un script avec des privilèges administrateur (`sudo`) ne doit jamais se faire à l'aveugle. Nous vous encourageons vivement à inspecter le code de [`scripts/install.sh`](scripts/install.sh) avant de l'exécuter, afin de vérifier en toute transparence les actions qu'il effectue sur votre système.

#### Releases GitHub & Architectures supportées
Des paquets pré-compilés et binaires autonomes sont disponibles sur la [page des Releases GitHub](https://github.com/BolzaW/greenup-link/releases) :

| Fichier Release | Architecture / Cible | Système visé | Contenu |
| :--- | :--- | :--- | :--- |
| **`greenup-link-vX.Y.Z-armv7-legacy.zip`** | ARMv7 32-bit (`armv7-unknown-linux-gnueabihf.2.24`) | **Kit officiel Legrand** (Debian 9 Stretch, glibc 2.24) et Raspberry Pi OS 32-bit | `greenup-link`, `install.sh`, `uninstall.sh`, `README.md`, `README.fr.md` |
| **`greenup-link-vX.Y.Z-aarch64.zip`** | AArch64 64-bit (`aarch64-unknown-linux-gnu`) | **Raspberry Pi OS 64-bit Lite officiel** (Pi Zero 2 W, Pi 3, Pi 4, Pi 5) | `greenup-link`, `install.sh`, `uninstall.sh`, `README.md`, `README.fr.md` |
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
> - Le script détecte automatiquement l'architecture de votre système (`armv7` 32-bit legacy ou `aarch64` 64-bit), interroge la dernière release GitHub, télécharge directement le binaire autonome adéquat et l'installe dans `/opt/greenup-link/`.
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
- **Détection d'environnement** : Détecte automatiquement s'il tourne sur l'image d'usine Legrand ou sur un Raspberry Pi OS standard/propre (kit DIY, carte SD réinstallée).
- **Neutralisation de la stack Legrand (si présente)** : Arrête Apache Tomcat 9 / daemon Java, neutralise `/etc/init.d/CommunicationArduinoRasp` (avec sauvegarde `.old`), et neutralise `/home/pi/Desktop/DeletAll.sh` (avec sauvegarde `.old`). Sur un OS propre ou standard, cette étape est automatiquement ignorée.
- **Utilisateur système dédié & sécurisé** : Crée un utilisateur non privilégié `greenup` appartenant au groupe `dialout` (nécessaire pour communiquer avec le port série `/dev/ttyUSB0` sans être root) et au groupe `gpio` si présent.
- **Répertoire confiné dans `/opt`** : Regroupe tous les composants (binaire `greenup-link`, configuration `greenup.toml`, logs et outil de désinstallation) dans un unique dossier `/opt/greenup-link/` (`chmod 755`, utilisateur système `greenup`, configuration `greenup.toml` directement éditable par l'utilisateur hôte `pi`). Aucun composant n'est dispersé dans l'arborescence système.
- **Mise en place de l'outil de rollback** : Installe `uninstall.sh` dans `/opt/greenup-link/uninstall.sh` pour un retour arrière facile à tout moment.
- **Service Systemd durci** : Crée, active et démarre immédiatement le service `/etc/systemd/system/greenup-link.service` exécuté sous l'utilisateur `greenup` avec durcissement de sécurité (`ProtectSystem=full`, `ProtectHome=true`, `NoNewPrivileges=true`, `PrivateTmp=true`).

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
- **Interface Web** : Ouvrez `http://<ip_borne>:8080` dans votre navigateur. Visualiseur en direct, contrôles de charge (7A à 32A), statut Linky TIC et console live intégrée.
- **Intégration Home Assistant** : Consultez le [Guide d'intégration Home Assistant](docs/HOME_ASSISTANT_INTEGRATION.md) pour profiter de l'Auto-Discovery MQTT natif sans configuration YAML.
- **Intégration MQTT** : Consultez le [Guide MQTT](docs/GREENUP_LINK_MQTT.md) pour les topics publiés, topics de commande et schémas JSON.
- **API REST** : Consultez la [Documentation de l'API REST](docs/GREENUP_LINK_REST_API.md) pour l'intégration avec Home Assistant, Jeedom, Node-RED ou vos propres scripts.
- **Intégration EVCC** : Consultez le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md) pour la charge solaire dynamique via MQTT ou HTTP REST.
- **Client CLI interactif** : Utilisez `scripts/cli-greenup-link.ps1` sous PowerShell Windows pour piloter ou tester la borne.

### 7. Rollback & Désinstallation (Restauration de la stack Legrand)

Si vous souhaitez revenir au logiciel officiel Legrand ou désinstaller Green'Up Link, un script de rollback / désinstallation automatisé est fourni.

- **Lancement direct depuis la borne** (installé automatiquement avec Green'Up Link) :
  ```bash
  sudo /opt/greenup-link/uninstall.sh
  ```
- **Ou via commande unique en ligne** :
  ```bash
  curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/uninstall.sh | sudo bash
  ```

Le script vous propose interactivement de choisir entre :
1. **Désactiver uniquement (Conserver les fichiers)** : Arrête et supprime le service `greenup-link.service`, tout en préservant le dossier `/opt/greenup-link/` et l'historique des logs.
2. **Purge complète** : Arrête le service, supprime l'utilisateur système `greenup` et supprime intégralement le dossier `/opt/greenup-link/` sans laisser aucune trace.

Dans les deux cas, si la stack officielle Legrand est présente sur l'OS, le script la réactive automatiquement :
- Restaure le script `/etc/init.d/CommunicationArduinoRasp` depuis sa sauvegarde et le réenregistre au démarrage.
- Restaure le script `/home/pi/Desktop/DeletAll.sh` depuis sa sauvegarde et rétablit ses droits d'exécution.
- Réactive et redémarre le serveur Apache Tomcat 9 et le daemon Java Legrand.
*(Sur un Raspberry Pi OS propre/standard ou kit DIY, cette étape de restauration Legrand est automatiquement ignorée).*

> [!IMPORTANT]
> **Sécurité face au pire des scénarios :** Bien que le script de rollback restaure fidèlement la configuration d'origine, il est fortement recommandé de réaliser une image brute complète (clone bit-à-bit) de votre carte microSD d'origine (via *Win32DiskImager*, *Raspberry Pi Imager*, *balenaEtcher* ou `dd`) avant toute modification matérielle ou logicielle pour parer à toute éventualité (carte SD corrompue ou défaillance matérielle).

---

## ❓ Foire Aux Questions (FAQ)

<details>
<summary><strong>1. Ai-je obligatoirement besoin du kit de communication officiel Legrand (059056) ?</strong></summary>

**Non.** Si votre borne n'en est pas équipée, vous pouvez concevoir votre propre kit à moindre coût à l'aide d'un Raspberry Pi standard (Pi Zero 2 W, Pi 3, Pi 4...) sous Raspberry Pi OS 64-bit Lite officiel, relié à la carte de puissance par un simple câble USB.

L'alimentation du Raspberry Pi peut se faire :
- **En interne :** via un petit convertisseur DC/DC étanche (12V/24V vers 5V) branché sur le connecteur auxiliaire Molex de la borne (comme le kit constructeur).
- **En externe (alternative simplifiée) :** à l'aide d'une alimentation secteur pour Raspberry Pi standard (micro-USB ou USB-C) dont le câble est introduit dans la borne par l'un des passe-câbles étanches (presse-étoupe) situés sous le châssis (suppose d'avoir une prise secteur 230V ou une rallonge à proximité).

Consultez le [Guide du kit de communication alternatif DIY](docs/KIT_COMMUNICATION_DIY.md) pour la liste des composants, les prix et les instructions d'assemblage.
</details>

<details>
<summary><strong>2. Green'Up Link remplace-t-il les applications mobiles Legrand (EV Charge / EV Charge Light) ?</strong></summary>

**Non.** Les applications mobiles Legrand communiquent directement en Bluetooth avec la carte de puissance de la borne. Green'Up Link remplace la couche réseau IP (Ethernet / Wi-Fi). Si vous pilotez actuellement votre borne uniquement depuis votre smartphone en Bluetooth, vous n'avez très probablement pas de kit IP installé : il vous faudra une carte Raspberry Pi (officielle ou DIY) pour utiliser Green'Up Link. Un travail complet de rétro-ingénierie du protocole Bluetooth a été réalisé (voir la [Documentation du protocole Bluetooth](docs/LEGRAND_BLE_PROTOCOL.md)), mais Green'Up Link privilégie la liaison série USB pour une supervision locale continue, réactive et pérenne.
</details>

<details>
<summary><strong>3. Y a-t-il un risque pour la sécurité électrique de ma borne ou de mon véhicule ?</strong></summary>

**Le développement a été pensé et conçu pour limiter au maximum les risques**, mais le **risque zéro n'existe pas** et le projet reste en constante évolution :
- **Architecture de protection matérielle :** Green'Up Link ne remplace que la couche de communication exécutée sur le Raspberry Pi. La borne conserve l'intégralité de ses sécurités électriques matérielles d'origine, ses relais et contacteurs de puissance, sa surveillance de terre et son automate de sécurité autonome sur la carte interne ATmega2560. Green'Up Link ne fait que relayer des consignes d'intensité (7A à 32A) et lire la télémétrie sur la liaison série USB.
- **Responsabilité de l'utilisateur :** Tout n'est pas encore documenté ni entièrement connu sur les subtilités du firmware de la borne ou certains cas limites électriques. L'installation et l'usage de Green'Up Link relèvent donc de votre propre responsabilité (cf. avertissement en tête de documentation).
</details>

<details>
<summary><strong>4. Puis-je revenir en arrière sur le logiciel officiel Legrand ?</strong></summary>

**Oui.** Un script de rollback automatisé est fourni :
```bash
sudo /opt/greenup-link/uninstall.sh
```
Il restaure les scripts de démarrage Legrand, retire les neutralisations des scripts d'urgence, réactive Apache Tomcat 9 et les daemons Java, et vous permet de choisir entre conserver ou supprimer les fichiers Green'Up Link.

*Recommandation :* En complément du script, réaliser une sauvegarde complète (image clone) de la carte microSD d'origine avant toute manipulation est vivement conseillé pour parer au pire des scénarios.
</details>

<details>
<summary><strong>5. Ma borne est en triphasé ou dispose d'une prise domestique Schuko, est-elle supportée ?</strong></summary>

**Oui, le support est théoriquement implémenté dans Green'Up Link, mais il reste expérimental et non testé sur du matériel physique.**  
La borne fonctionne toujours nominalement en mode autonome local (gérée par sa carte de puissance interne ATmega2560). Green'Up Link intègre désormais la logique de gestion de la prise domestique Schuko et la télémétrie triphasée à partir des spécifications du firmware, mais l'auteur ne possède pas ces modèles pour valider physiquement les échanges.  
Si vous disposez d'un modèle triphasé ou avec prise Schuko, n'hésitez pas à tester et à remonter vos observations et logs série (voir [Appel à testeurs](#-appel-à-testeurs--contributions)).
</details>

<details>
<summary><strong>6. Comment intégrer Green'Up Link dans Home Assistant ou EVCC ?</strong></summary>

- **Home Assistant :** Consultez le [Guide d'intégration Home Assistant](docs/HOME_ASSISTANT_INTEGRATION.md) pour profiter de l'Auto-Discovery MQTT natif sans aucune configuration YAML (ou utilisez l'[API REST](docs/GREENUP_LINK_REST_API.md)).
- **EVCC :** Consultez le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md) pour la charge solaire dynamique via MQTT ou HTTP REST.
</details>

<details>
<summary><strong>7. Pourquoi remplacer le logiciel officiel Legrand ?</strong></summary>

Le matériel de la borne Green'Up Premium est soigné et fiable, mais sa suite logicielle d'origine accuse le poids des années : basée sur Debian 9 Stretch, Apache Tomcat 9 et un runtime Java ancien, elle manque d'ergonomie, n'est plus maintenue et reste fermée aux outils domotiques récents. Green'Up Link propose une alternative moderne, légère, ultra-réactive et sécurisée écrite en Rust sans dépendance externe pour tirer le plein potentiel de votre borne.
</details>

---

## 🚀 Fonctionnalités futures

Pistes et idées pour la feuille de route future (sans calendrier contractuel) :
- **Support natif d'OCPP 1.6-J** : Interface WebSocket JSON directe pour connecter la borne à un superviseur centralisé (CSMS type SteVe, MaEVe, CitrineOS) sans passerelle externe.
- **Rapports d'énergie & Suivi financier automatisés** : Bilans périodiques de consommation et calcul des coûts de recharge basés sur la grille tarifaire Linky HP/HC, exportables en PDF/CSV.
- **Gestionnaire solaire autonome intégré** : Moteur interne de délestage et surplus solaire branché directement sur les données d'un onduleur ou compteur Linky, pour ceux qui ne souhaitent pas déployer EVCC.
- **Recherches exploratoires sur la carte de puissance** : Études et rétro-ingénierie avancée autour des capacités du microcontrôleur ATmega2560.

---

## 🤝 Appel à testeurs & Contributions

Je recherche activement des testeurs disposant de configurations matérielles différentes de la gamme Legrand Green'up Premium (par exemple des bornes équipées d'une prise domestique Schuko, des modèles triphasés, des modèles à double point de charge ou avec d'autres révisions de cartes électroniques / firmwares).

L'objectif est de collecter des logs de communication série pour comprendre le fonctionnement de ces variantes et étendre le support de Green'Up Link à l'ensemble de la gamme.

Si vous disposez d'un de ces modèles et souhaitez participer aux tests ou partager vos retours :
- Ouvrez un ticket sur **[GitHub Issues](https://github.com/BolzaW/greenup-link/issues)**
- Ou contactez-moi directement par email : **[bolzaw.dev@gmail.com](mailto:bolzaw.dev@gmail.com)**
