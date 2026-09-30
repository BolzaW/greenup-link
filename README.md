# Green'Up Link v0.0.1

> **Disclaimer :** Green'Up Link is an independent, community-developed project and is not affiliated with, sponsored or endorsed by Legrand. "Green'Up" and "Legrand" are trademarks of their respective owners.

## 📖 Résumé du projet

**Green'Up Link** est un logiciel léger, performant et indépendant écrit en Rust. Il s'installe directement sur le Kit de Communication officiel Legrand (référence 059056) en remplacement du logiciel ("KitCom") fourni par le constructeur. 

Il communique avec la carte de puissance de la borne via la liaison série USB interne et expose :
- Une **Interface Homme-Machine (IHM) Web locale** claire et réactive.
- Une **API REST** complète et documentée, parfaite pour l'intégration en domotique (Home Assistant, Jeedom, etc.).

## 🤔 Pourquoi ce projet ?

Le logiciel fourni avec le Kit de Communication officiel s'apparente à une "usine à gaz". Il est truffé de bugs, peu réactif, et s'avère très difficilement utilisable pour de l'automatisation domotique standard. 

De plus, il repose sur un Raspberry Pi 3 équipé d'une version obsolète du système d'exploitation (Raspbian 9 "Stretch"), vulnérable sur le plan de la sécurité. Ce projet permet de reprendre le contrôle total de son matériel de manière beaucoup plus saine et réactive.

## ⚠️ Limitations actuelles

Ce projet est en cours de développement. Les fonctionnalités seront ajoutées au fur et à mesure. Pour le moment :
- Ne gère que la **charge directe**.
- Ne gère que la borne référence **058001** (1 seul côté Type 2, sans prise domestique Shuko, 7kW).
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
3. **Installer les dépendances Linux (WSL)** nécessaires à la compilation croisée (ex: gcc, pkg-config).
4. **Installer Cargo Zigbuild** : `cargo install cargo-zigbuild` (permet de cross-compiler facilement vers ARM).
5. Exécuter le script **`.\build_pi.bat`** (depuis PowerShell ou l'invite de commande Windows). Ce script va invoquer WSL et générer le binaire optimisé pour la borne.

---

## 🚀 Installation sur la borne (Raspberry Pi)

> **ATTENTION :** Sauvegardez l'image (clone de la carte SD) officielle Legrand avant toute manipulation afin de pouvoir revenir en arrière en cas de problème.

### 1. Préparation du Raspberry Pi
1. Suivez la **[documentation officielle Legrand](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** pour initialiser le Raspberry Pi sur votre réseau.
2. Pour obtenir l'accès SSH, il faut exploiter une faille connue du processus d'initialisation de la borne au moment du flashage via clé USB.
3. **Une fois l'accès SSH obtenu** :
   - Modifiez immédiatement le mot de passe `root`.
   - Sécurisez le SSH par échange de clé publique.
   - 🚨 **NOTE DE SÉCURITÉ :** Le Raspberry Pi tourne sur un vieil OS avec de nombreuses failles connues. Ne l'exposez **JAMAIS** sur internet (pas de redirection de port sur votre box).

### 2. Neutralisation des sécurités Legrand
Le logiciel d'origine intègre un script de "destruction" d'urgence (`DeletAll.sh`) qui efface tout le logiciel si les adresses MAC réseau ne correspondent pas à celles attendues par Legrand. Il faut le neutraliser :
1. Éditez le fichier : `nano ~/Desktop/DeletAll.sh`
2. Ajoutez la commande `exit 0` sur la deuxième ligne (juste après le `#!/bin/bash`).
3. Retirez les droits d'exécution : `chmod -x ~/Desktop/DeletAll.sh`

### 3. Installation de Green'Up Link
1. Transférez le binaire compilé (`greenup-link`) et les scripts de contrôle éventuels vers la borne via SCP. 
   - *Exemple de destination :* `~/Desktop/rust/`
2. Arrêtez les services Legrand actuels en exécutant le script `stop` d'origine (situé sur le bureau).
3. **Désactivez le démarrage automatique** du logiciel Legrand d'origine en désactivant le script d'initialisation :
   - `/etc/init.d/CommunicationArduinoRasp` (le retirer des runlevels via `update-rc.d` ou le rendre inerte).
4. Mettez en place un script de démarrage pour lancer automatiquement l'exécutable `greenup-link` au boot (ex: un simple service Systemd ou une entrée dans `/etc/rc.local`).

### 4. Utilisation
- **Interface Graphique** : Ouvrez simplement l'adresse IP de la borne sur le port 8080 depuis votre navigateur (ex: `http://192.168.1.50:8080`).
- **Tests & CLI** : Utilisez le script PowerShell fourni `cli-greenup-link.ps1` depuis votre PC pour interagir avec l'API en ligne de commande.
- **Domotique** : Consultez la [Documentation de l'API REST](API_REST_LEGRAND.md) pour interfacer votre box domotique.

---

## 🔮 Fonctionnalités futures

La feuille de route inclut (sans date garantie) :
- Le support du mode de communication OCPP.
- L'élargissement de la compatibilité aux autres modèles de bornes de la gamme.

*Note : N'hésitez pas à utiliser les "Issues" GitHub pour suggérer ou demander des fonctionnalités spécifiques dont vous auriez besoin !*
