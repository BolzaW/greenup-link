# Protocole Série - Carte de Puissance Legrand GreenUp

Ce document recense les commandes ASCII utilisées pour communiquer avec la carte de puissance. 
Ces commandes ont été déduites d'activités de rétro-ingénierie et de décompilation des logiciels d'origine (.jar) et du firmware (.hex).
Toutes ces commandes doivent être envoyées à la carte de puissance suivies d'un caractère "Retour Chariot" (Carriage Return), c'est-à-dire `\r` (code ASCII 0x0D).

## 🔌 1. Initialisation & Système
Ces commandes sont utilisées au démarrage pour établir le dialogue.

| Commande | Explication supposée |
| :--- | :--- |
| `RaspberryPiModeOK` | Trame magique qui indique à la carte de puissance que le Pi a pris le contrôle. |
| `SoftwareVersion?` | Demande la version du firmware de la carte de puissance. |
| `HardwareVersion?` | Demande la version matérielle (Hardware) de la carte. |
| `SerialNumber?` | Demande le numéro de série de la borne. |
| `Reference?` | Demande la référence du produit Legrand. |
| `WeekYearProduction?`| Demande la date de fabrication (Semaine/Année). |
| `Side?` | Demande le côté actif (Side 1 ou 2). Souvent utile sur les bornes doubles. |
| `Reset` | Redémarre (reboot) la carte de puissance. |
| `Test` | Passe la carte dans un mode "Test" usine ou labo. |
| `ping` | Ping basique (probablement pour vérifier si la carte est toujours en vie). |

## 📊 2. Statut & Télémesure
Ces commandes permettent d'interroger la borne sur son état actuel.

| Commande | Explication supposée |
| :--- | :--- |
| `State?` | Demande l'état de la charge (probablement l'état IEC 61851 : A, B, C, D). |
| `E?` | Demande les erreurs en cours (renvoie `E:0000` si tout va bien). |
| `CC?` | Demande la limite de courant configurée (renvoie par ex: `CC:16`). |
| `CCTIC?` | Demande la valeur du courant limite déduite par le TIC. Renvoie `32` même si le TIC est déconnecté. |
| `TICTM:1` / `TICTM:0` | Active (`1`) ou désactive (`0`) le mode Test TIC. En mode test, la borne renvoie `TICTestC:Init` puis périodiquement `TICTestB:X` où `X` est la vitesse en baud (`0` si absent, `1200` si Historique, `9600` si Standard). |

## ⚡ 3. Pilotage de la Charge (Le plus important !)
Ces commandes contrôlent directement la délivrance du courant.

| Commande | Explication supposée |
| :--- | :--- |
| `CC:16` | **Charge Current** : Règle la limite de puissance de la prise principale à X Ampères (ex: 16A, 32A). |
| `CCS:16` | **Charge Current Schuko** : Règle la limite de puissance pour la prise domestique (Schuko). |
| `T2COK` / `T2CNOK` | Autorise (`OK`) ou Bloque (`NOK`) la charge sur la prise **Type 2** (T2). |
| `2PCOK` / `2PCNOK` | Autorise (`OK`) ou Bloque (`NOK`) la charge sur la prise **Domestique** (2 Pins). |
| `T2FOK` / `T2FNOK` | **Force** la charge sur la prise Type 2 (contournement des sécurités ou du planning ?). |
| `2PFOK` / `2PFNOK` | **Force** la charge sur la prise domestique. |
| `SOK` / `SNOK` | **Suspend** : Autorise la mise en pause (`OK`) ou annule la pause (`NOK`) de la charge. |
| `Unlock` | Ordonne le **déverrouillage physique** du câble (si la borne a un verrouillage de la prise Type 2). |

## 💳 4. Gestion OCPP / RFID
Ces commandes gèrent l'interaction avec le badge de l'utilisateur ou la supervision logicielle.

| Commande | Explication supposée |
| :--- | :--- |
| `OCPPPS?` | OCPP Parameter/Status ? Interroge la carte sur des paramètres spécifiques liés à OCPP. |
| `OCPPPACOK` / `OCPPPACNOK` | **P**lug **A**nd **C**harge : Indique à la carte si la fonction "Branche et Charge" sans badge est activée ou non. |
| `RFIDId:XXXXX` | Transmet l'ID du badge RFID (lu par le Pi) à la carte de puissance. |
| `RFIDA:XXXXX` | Envoie le statut d'autorisation RFID (Authorization). |
| `OCPPCTO:XXX` | Connection Time Out : Règle le délai d'expiration (timeout) de connexion. |

---
*Note pour les tests : Dans le terminal cli_greenup-link.ps1, pas besoin de taper le `\r`, le programme l'ajoute automatiquement à l'appuie sur la touche Entrée.*

## 📻 5. Gestion du module Bluetooth (BLE)
La borne possède un module Bluetooth intégré (utilisé par l'application smartphone). Il peut être activé ou désactivé pour éviter les conflits d'ordres avec le Raspberry Pi.

| Commande (TX) | Réponse (RX) | Explication |
| :--- | :--- | :--- |
| `BT?` | `BT:1` ou `BT:0` | Demande l'état actuel du module Bluetooth (1 = Allumé, 0 = Éteint). |
| `BTOK` | `BT:1` | Ordonne l'allumage du module Bluetooth. |
| `BTNOK`| `BT:0` | Ordonne l'extinction du module Bluetooth. |
