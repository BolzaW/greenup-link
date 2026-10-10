# Green'Up Link

[🇬🇧 Read in English](README.md)

> [!WARNING]
> **Avertissement de non-responsabilité :** Green'Up Link est un projet communautaire indépendant et n'est ni affilié, ni sponsorisé, ni approuvé par Legrand. "Green'Up" et "Legrand" sont des marques déposées de leurs propriétaires respectifs. Le logiciel est fourni "tel quel", sans garantie. L'utilisation de ce logiciel avec votre borne se fait à vos risques et périls.

## 📖 Résumé du projet

**Green'Up Link** est un logiciel léger, performant et indépendant écrit en Rust. Il s'installe directement sur le Kit de Communication officiel Legrand (référence 059056) en remplacement du logiciel ("KitCom") fourni par le constructeur. 

Il communique avec la carte de puissance de la borne via la liaison série USB interne et expose :
- Une **IHM Web locale** claire et réactive.
- Une **API REST** complète et documentée, parfaite pour l'intégration en domotique (Home Assistant, Jeedom, etc.).
- Une compatibilité totale avec **EVCC** pour la recharge solaire intelligente (voir le [Guide d'intégration EVCC](docs/EVCC_INTEGRATION.md)).

Le projet propose également une intégration EVerest sans API/IHM via un pont MQTT direct, permettant à la borne d'être pilotée comme un module matériel EVerest standard. *(Note : Pour une implémentation native et plus "propre" d'un module EVerest remplaçant entièrement l'EvseManager, n'hésitez pas à consulter l'excellent projet communautaire [evorada/everest-greenup](https://github.com/evorada/everest-greenup) réalisé par suda).*

## 🤔 Pourquoi ce projet ?

Le logiciel fourni avec le Kit de Communication officiel s'apparente à une "usine à gaz". Il est truffé de bugs, peu réactif, et s'avère très difficilement utilisable pour de l'automatisation domotique standard. 

De plus, il repose sur un Raspberry Pi 3 équipé d'une version obsolète du système d'exploitation (Raspbian 9 "Stretch"), vulnérable sur le plan de la sécurité. Ce projet permet de reprendre le contrôle total de son matériel de manière beaucoup plus saine et réactive.

## ⚠️ Limitations actuelles

Ce projet est en cours de développement. Les fonctionnalités seront ajoutées au fur et à mesure. Pour le moment :
- Ne gère que la **charge directe**.
- Ne gère que la borne référence **058001** (1 seul côté Type 2, sans prise domestique Shuko, 7kW). *C'est le modèle que je possède et sur lequel j'ai pu tester.*
- **NE GÈRE PAS** le lecteur de badge RFID.
- **NE GÈRE PAS** les plannings (programmation horaire).
- **NE GÈRE PAS** la communication Modbus.
- **NE GÈRE PAS** le protocole OCPP.

## ✅ Compatibilité

- **Système d'exploitation** : Image officielle Legrand "Raspbian GNU/Linux 9 (stretch)" (pour le moment).
- **Firmware de la carte de puissance** :
  - `FirmwareBoardA-V01;18;10.hex` (Testé et validé)
  - `FirmwareBoardA-V01;18;04.hex` (Théoriquement compatible)
  - `FirmwareBoardA-V01;17;27.hex` (Théoriquement compatible)

---

## 🛠️ Compiler le projet (Pour les développeurs)

Le projet a été développé sous Windows 11 en utilisant le sous-système Linux (WSL2), ce qui permet la compilation croisée vers l'architecture ARM du Raspberry Pi de manière transparente. Les étapes ci-dessous assument un environnement Windows, mais peuvent facilement être adaptées pour Linux.

1. **Installer WSL2** sur Windows (ex: `wsl --install -d Ubuntu`).
2. **Installer Rust** dans l'environnement WSL (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
3. **Installer les dépendances Linux (WSL)** nécessaires à la compilation croisée. Sous Debian/Ubuntu, exécutez :
   ```bash
   sudo apt update
   sudo apt install -y gcc-arm-linux-gnueabihf pkg-config
   ```
4. **Installer Cargo Zigbuild** : `cargo install cargo-zigbuild` (permet de cross-compiler facilement vers ARM).
5. Exécuter le script **`.\build_pi.bat`** (depuis PowerShell ou l'invite de commande Windows). Ce script va invoquer WSL et générer le binaire optimisé pour la borne.

---

## 🚀 Installation sur la borne (Raspberry Pi)

> **ATTENTION :** Sauvegardez l'image (clone de la carte SD) officielle Legrand avant toute manipulation afin de pouvoir revenir en arrière en cas de problème.

> ⚠️ **Important :** L'installation physique du Raspberry Pi dans la borne doit impérativement se faire **hors tension**.

### 1. Initialisation du kit legrand
Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.
   
### 2. Obtenir l'accès SSH (Modification de la carte SD)
Le Raspberry Pi 3 intégré utilise une installation classique (non chiffrée) mais le port SSH est fermé et le mot de passe par défaut a été modifié par Legrand. Il faut donc intervenir directement sur la carte SD :

1. Démontez la borne (hors tension) pour récupérer la carte SD du Raspberry Pi et lisez-la sur votre ordinateur.
2. **Activer le SSH** : Créez simplement un fichier vide nommé `ssh` (sans extension) à la racine de la partition `boot`.
3. **Réinitialiser le mot de passe** : L'utilisateur `pi` existe mais son mot de passe est inconnu.
   - Sur votre ordinateur (sous Linux ou WSL), générez le hash d'un nouveau mot de passe avec la commande : `mkpasswd -m sha-512`
   - Ouvrez la partition principale (rootfs) de la carte SD et éditez le fichier `/etc/shadow`.
   - Repérez la ligne commençant par `pi:` (ex: `pi:<hash-inconnu>:18508:0:99999:7:::`) et remplacez le hash existant par celui que vous venez de générer.
4. Remettez la carte SD dans le Raspberry Pi et mettez la borne sous tension.
5. Vous pouvez désormais vous connecter en SSH : `ssh pi@<ip_de_la_borne>` avec le mot de passe que vous avez choisi. L'utilisateur `pi` possède les droits administrateur (sudo).

### 3. Sécurisation de l'accès SSH
Une fois connecté au Raspberry Pi en SSH, il est impératif de le sécuriser.
🚨 **NOTE DE SÉCURITÉ :** Le Raspberry Pi tourne sur un vieil OS avec de nombreuses failles connues. Ne l'exposez **JAMAIS** sur internet (pas de redirection de port sur votre box).

1. **Modifiez immédiatement le mot de passe root** :
   ```bash
   passwd root
   ```
2. **Sécurisez l'accès par échange de clé publique** :
   Depuis votre PC, envoyez votre clé publique (si vous n'en avez pas, générez-la avec `ssh-keygen`) :
   ```bash
   ssh-copy-id pi@192.168.1.xxx
   ```
   > ⚠️ **Attention :** Avant de désactiver la connexion par mot de passe, vérifier que la connexion par clé fonctionne, autrement vous perdrez l'accès ssh et il faudra de nouveau démonter la carte micro SD

3. Désactivez l'authentification par mot de passe dans `/etc/ssh/sshd_config` (`PasswordAuthentication no`), et l'authentifcation root (`PermitRootLogin no`).

4. **Neutralisation des sécurités Legrand** : 
   Le logiciel d'origine intègre un script de "destruction" d'urgence (`DeletAll.sh`) qui efface tout le logiciel si les adresses MAC réseau ne correspondent pas à celles attendues par Legrand.
   - Éditez le fichier : `nano ~/Desktop/DeletAll.sh`
   - Ajoutez la commande `exit 0` sur la deuxième ligne (juste après le `#!/bin/bash`).
   - Retirez les droits d'exécution : `chmod -x ~/Desktop/DeletAll.sh`

### 4. Installation de Green'Up Link
1. Transférez le binaire compilé (`greenup-link`) et les scripts vers la borne via SCP. Depuis votre PC :
   ```bash
   scp target/armv7-unknown-linux-gnueabihf/debug/greenup-link pi@192.168.1.xxx:~/Desktop/rust
   scp scripts/* pi@192.168.1.xxx:~/Desktop/rust
   ```
2. Arrêtez les services Legrand actuels en exécutant le script `stop`.
3. **(Optionnel mais recommandé) Désactivez le démarrage automatique** du logiciel Legrand d'origine. Si vous ne le faites pas, le logiciel officiel redémarrera à chaque reboot de la borne :
   ```bash
   sudo update-rc.d -f CommunicationArduinoRasp remove
   ```

### 5. Lancement automatique de Green'Up Link au boot
Pour lancer l'application automatiquement, créez un service Systemd :
1. Créez le fichier de service (nécessite les droits sudo) : `sudo nano /etc/systemd/system/greenup-link.service`
2. Collez-y cette configuration :
   ```ini
   [Unit]
   Description=GreenUp Link Service
   After=network.target

   [Service]
   ExecStart=/home/pi/Desktop/rust/greenup-link
   WorkingDirectory=/home/pi/Desktop/rust/
   StandardOutput=inherit
   StandardError=inherit
   Restart=always
   User=root

   [Install]
   WantedBy=multi-user.target
   ```
3. Activez et démarrez le service :
   ```bash
   sudo systemctl enable greenup-link
   sudo systemctl start greenup-link
   ```

### 6. Utilisation
- **Interface Graphique** : Ouvrez simplement l'adresse IP de la borne sur le port 8080 depuis votre navigateur (ex: `http://192.168.1.xxx:8080`).
- **Tests & CLI** : Utilisez le script PowerShell fourni `cli-greenup-link.ps1` depuis votre PC pour interagir avec l'API en ligne de commande.
- **Domotique** : Consultez la [Documentation de l'API REST](docs/GREENUP_LINK_REST_API.md) pour interfacer votre box domotique.
- **Module EVerest** : Pour une intégration avec le framework open-source de recharge de VE EVerest (via MQTT), veuillez vous référer à la [Documentation d'intégration EVerest](crates/greenup-everest/README.md).

---

## 🔮 Fonctionnalités futures

La feuille de route inclut (sans date garantie) :
- Intégration MQTT pour la domotique
- Compatibilité avec les OS récents de Raspberry Pi
- Firmware ATmega custom avec plus de capacités (comme plus d'infos télémétriques depuis le signal TIC)
- Le support du mode de communication OCPP.
- L'élargissement de la compatibilité aux autres modèles de bornes de la gamme :
  - Compatibilité prise Schuko (domestique)
  - Support du Triphasé
  - Support des bornes double point de charge

*Note : N'hésitez pas à utiliser les "Issues" GitHub pour suggérer ou demander des fonctionnalités spécifiques dont vous auriez besoin !*

