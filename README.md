# Green'Up Link v0.0.1

> **Disclaimer :** Green'Up Link is an independent, community-developed project and is not affiliated with, sponsored or endorsed by Legrand. "Green'Up" and "Legrand" are trademarks of their respective owners.

## 📖 Résumé du projet

**Green'Up Link** est un logiciel léger, performant et indépendant écrit en Rust. Il s'installe directement sur le Kit de Communication officiel Legrand (référence 059056) en remplacement du logiciel ("KitCom") fourni par le constructeur. 

Il communique avec la carte de puissance de la borne via la liaison série USB interne et expose :
- Une **IHM Web locale** claire et réactive.
- Une **API REST** complète et documentée, parfaite pour l'intégration en domotique (Home Assistant, Jeedom, etc.).

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

### 1. Obtenir l'accès SSH (Hack USB)
L'interface d'origine ne permet pas l'accès SSH. Il faut exploiter le processus de mise à jour par clé USB pour forcer l'activation du service :
1. Préparez une clé USB formatée en FAT32.
2. Créez à la racine de la clé le script de mise à jour reconnu par le système Legrand.
3. Insérez les commandes suivantes dans ce script pour activer et démarrer le service SSH :
   ```bash
   systemctl enable ssh
   systemctl start ssh
   ```
4. Insérez la clé USB dans le Raspberry Pi de la borne (sous tension) et laissez le script s'exécuter.

### 2. Sécurisation de l'accès SSH
Une fois connecté au Raspberry Pi en SSH, il est impératif de le sécuriser.
🚨 **NOTE DE SÉCURITÉ :** Le Raspberry Pi tourne sur un vieil OS avec de nombreuses failles connues. Ne l'exposez **JAMAIS** sur internet (pas de redirection de port sur votre box).

1. **Modifiez immédiatement le mot de passe root** :
   ```bash
   passwd root
   ```
2. **Sécurisez l'accès par échange de clé publique** :
   Depuis votre PC, envoyez votre clé publique (si vous n'en avez pas, générez-la avec `ssh-keygen`) :
   ```bash
   ssh-copy-id root@192.168.1.xxx
   ```
3. (Optionnel) Désactivez l'authentification par mot de passe dans `/etc/ssh/sshd_config` (`PasswordAuthentication no`).

### 3. Préparation matérielle et logicielle
1. Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.
   > ⚠️ **Important :** L'installation physique du Raspberry Pi dans la borne doit impérativement se faire **hors tension**.

2. **Neutralisation des sécurités Legrand** : 
   Le logiciel d'origine intègre un script de "destruction" d'urgence (`DeletAll.sh`) qui efface tout le logiciel si les adresses MAC réseau ne correspondent pas à celles attendues par Legrand.
   - Éditez le fichier : `nano ~/Desktop/DeletAll.sh`
   - Ajoutez la commande `exit 0` sur la deuxième ligne (juste après le `#!/bin/bash`).
   - Retirez les droits d'exécution : `chmod -x ~/Desktop/DeletAll.sh`

### 4. Installation de Green'Up Link
1. Transférez le binaire compilé (`greenup-link`) vers la borne via SCP. Depuis votre PC :
   ```bash
   scp target/armv7-unknown-linux-gnueabihf/debug/greenup-link root@192.168.1.xxx:/root/Desktop/
   ```
2. Arrêtez les services Legrand actuels en exécutant le script `stop` d'origine (situé sur le bureau).
3. **(Optionnel mais recommandé) Désactivez le démarrage automatique** du logiciel Legrand d'origine. Si vous ne le faites pas, le logiciel officiel redémarrera à chaque reboot de la borne :
   ```bash
   update-rc.d -f CommunicationArduinoRasp remove
   ```

### 5. Lancement automatique de Green'Up Link au boot
Pour lancer l'application automatiquement, créez un service Systemd :
1. Créez le fichier de service : `nano /etc/systemd/system/greenup-link.service`
2. Collez-y cette configuration (à adapter selon le chemin de votre exécutable) :
   ```ini
   [Unit]
   Description=GreenUp Link Service
   After=network.target

   [Service]
   ExecStart=/root/Desktop/greenup-link
   WorkingDirectory=/root/Desktop/
   StandardOutput=inherit
   StandardError=inherit
   Restart=always
   User=root

   [Install]
   WantedBy=multi-user.target
   ```
3. Activez et démarrez le service :
   ```bash
   systemctl enable greenup-link
   systemctl start greenup-link
   ```

### 6. Utilisation
- **Interface Graphique** : Ouvrez simplement l'adresse IP de la borne sur le port 8080 depuis votre navigateur (ex: `http://192.168.1.xxx:8080`).
- **Tests & CLI** : Utilisez le script PowerShell fourni `cli-greenup-link.ps1` depuis votre PC pour interagir avec l'API en ligne de commande.
- **Domotique** : Consultez la [Documentation de l'API REST](API_REST_LEGRAND.md) pour interfacer votre box domotique.

---

## 🔮 Fonctionnalités futures

La feuille de route inclut (sans date garantie) :
- Le support du mode de communication OCPP.
- L'élargissement de la compatibilité aux autres modèles de bornes de la gamme.

*Note : N'hésitez pas à utiliser les "Issues" GitHub pour suggérer ou demander des fonctionnalités spécifiques dont vous auriez besoin !*
