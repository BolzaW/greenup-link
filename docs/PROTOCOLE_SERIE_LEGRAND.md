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
| `State?` | Demande l'état transactionnel/comportemental de la charge (A, B, C, D, E, I, W, M). Attention : ce n'est PAS l'état physique IEC 61851 brut. Voir docs/MACHINE_A_ETATS.md pour le détail complet. |
| `E?` | Demande les erreurs en cours (renvoie `E:0000` si tout va bien). Voici la table officielle d'interprétation des codes d'erreur extraite du code Java (mapping OCPP) :<br>• **`E:0000`** : `NoError` (Aucun défaut)<br>• **`E:0001`** : `ConnectorLockFailure` (Erreur de verrouillage prise T2S)<br>• **`E:0002`** : `ConnectorLockFailure` (Erreur de déverrouillage prise T2S)<br>• **`E:0003`** : `OtherError` (Court-circuit du Control Pilot détecté côté prise, câble ou véhicule)<br>• **`E:0004`** : `OtherError` (Court-circuit du Control Pilot détecté côté borne)<br>• **`E:0005`** : `PowerSwitchFailure` (Erreur d'ouverture du contacteur de la prise Domestique)<br>• **`E:0006`** : `PowerSwitchFailure` (Erreur de fermeture du contacteur de la prise Domestique)<br>• **`E:0007`** : `PowerSwitchFailure` (Erreur d'ouverture du contacteur de la prise T2S)<br>• **`E:0008`** : `PowerSwitchFailure` (Erreur de fermeture du contacteur de la prise T2S)<br>• **`E:0009`** : `OtherError` (Diode non détectée côté véhicule)<br>• **`E:0010`** : `OverCurrentFailure` (Surcharge de courant sur prise T2S. *Aussi déclenché en cas de crash PWM / consigne < 6A !*)<br>• **`E:0011`** : `OverCurrentFailure` (Surcharge de courant sur prise Domestique)<br>• **`E:0012`** : `UnderVoltage` (Coupure de courant / Panne de courant électrique)<br>• **`E:0013`** : `OtherError` (Erreur de communication interne USB)<br>• **`E:0015`** : `OtherError` (Défaut de fuite courant continu 6mA détecté) |

| `CC?` | Demande la limite de courant configurée (renvoie par ex: `CC:16`). |
| `CCTIC?` | Demande la valeur du courant limite déduite par le TIC. Renvoie `32` même si le TIC est déconnecté. |
| `TICTM:1` / `TICTM:0` | Active (`1`) ou désactive (`0`) le mode Test TIC. En mode test, la borne renvoie `TICTestC:Init` puis périodiquement `TICTestB:X` où `X` est la vitesse en baud (`0` si absent, `1200` si Historique, `9600` si Standard). Ensuite, elle diffuse en boucle `TICTestC:XX` où `XX` est la limite de charge dynamique (CCTIC) calculée par la borne pour le délestage. |

## ⚡ 
### Mesures et Consignes de Courant / Puissance
Ces variables permettent de comprendre exactement quelles sont les limites imposées et ce qui est consommé.

**Variables requêtables (avec `?`) ou reçues en écho :**
*   **`CCCa`** (*max current cable*) : Capacité matérielle du câble (lue via la résistance PP). Ex: `CCCa:32` pour 32A.
*   **`CCS`** (*max current station*) : Capacité matérielle de la borne (réglée via les switchs DIP internes). Ex: `CCS:32`.
*   **`CCEl`** (*max current eliot*) : Limite imposée par le Cloud (Eliot / App Legrand).
*   **`CC`** (*charging current*) : La consigne finale retenue et imposée par l'ATmega (souvent le minimum des limites précédentes).
*   **`CP`** (*control pilot*) : Tension brute mesurée sur la broche Control Pilot. **Vital :** Interroger `CP?` permet de connaître l'état de connexion physique réel (`12`=débranché, `9`=branché, `6`=en charge) et permet de contourner le verrouillage de la machine à état causé par `T2CNOK`.

**Variables spontanées (émises par la borne) :**
*   **`CCI:X.XX`** (*current instantaneous*) : Courant instantané réel tiré par le véhicule (en Ampères). S'effondre en fin de charge.
*   **`CPh:Mono` / `CPh:Tri`** (*charge phases*) : Détection automatique du nombre de phases utilisées par le véhicule. Émis juste avant la montée en charge.

3. Pilotage de la Charge (Le plus important !)
Ces commandes contrôlent directement la délivrance du courant.

| Commande | Explication supposée |
| :--- | :--- |
| `CC:16` | **Charge Current** : Règle la limite de puissance de la prise principale à X Ampères (ex: 16A, 32A). |
| `CCS:16` | **Charge Current Schuko** : Règle la limite de puissance pour la prise domestique (Schuko). |
| `T2COK` / `T2CNOK` | Autorise (`OK`) ou Bloque (`NOK`) la charge sur la prise **Type 2** (T2). |
| `2PCOK` / `2PCNOK` | Autorise (`OK`) ou Bloque (`NOK`) la charge sur la prise **Domestique** (2 Pins). |
| `T2FOK` / `T2FNOK` | **Force** la charge sur la prise Type 2 (contournement des sécurités ou du planning ?). |
| `2PFOK` / `2PFNOK` | **Force** la charge sur la prise domestique. |
| `SOK` / `SNOK` | **Sleep (Veille)** : `SOK` force la borne à entrer en mode veille profonde (État `Y`, LEDs éteintes avec flash lent, réponse `Slp:1`). Ce mode n'est atteignable que depuis l'état `A`. `SNOK` réveille la borne (`Slp:0`, État `A`). |
| `SBOK` / `SBNOK` | **Commandes de pilotage logiciel absolu (Pause/Reprise)**. Simulent l'appui sur le bouton physique START/STOP de la façade. `SBNOK` arrête proprement la charge et stabilise la borne en `State:M`. `SBOK` réveille la borne et relance le cycle (`State:B` -> `State:C`). Ne pas confondre avec l'événement de lecture `SBF`. |
| `Unlock` | Ordonne le **déverrouillage physique** du câble (si la borne a un verrouillage de la prise Type 2). |

## 💳 4. Gestion OCPP / RFID
Ces commandes gèrent l'interaction avec le badge de l'utilisateur ou la supervision logicielle.

| Commande | Explication supposée |
| :--- | :--- |
| `OCPPPS?` | OCPP Status : Demande le statut OCPP de la borne. Elle répond par un état OCPP (ex: OCPPStatus:Available ou OCPPStatus:SuspendedEVSE). Attention : son implémentation est imparfaite (elle répond SuspendedEVSE même si l'interruption vient du véhicule, au lieu de SuspendedEV). |
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

## 🧾 6. Rapports de fin de charge (Session Tickets)
À la fin d'une session de charge (lorsque la borne passe par les états W puis M avant de revenir à A), la carte de puissance émet automatiquement une trame récapitulative contenant les données de la session.

**Exemple de trame RX :**
\WT:0:0:5:CT:0:6:58:EVplug:4485.20:0.00:\

**Décryptage (déduit du code source Java d'origine) :**
Cette trame est séparée par des deux-points (\:\).
*   \WT\ : **Waiting Time** (Temps d'attente).
*   \ :0:5\ : Durée d'attente en Heures:Minutes:Secondes (ici 5 secondes).
*   \CT\ : **Charging Time** (Temps de charge).
*   \ :6:58\ : Durée effective de la charge en Heures:Minutes:Secondes.
*   \EVplug\ : Identifiant de la prise utilisée (\EVplug\ = Type 2, \DOMplug\ = Prise domestique E/F).
*   \4485.20\ : **Puissance Moyenne (en Watts)** délivrée pendant les Heures Pleines (HP).
*   \ .00\ : **Puissance Moyenne (en Watts)** délivrée pendant les Heures Creuses (HC).

*Note sur l'énergie : La carte ne remonte pas directement des Wh, mais la puissance moyenne. Pour obtenir l'énergie totale de la session en Wh, le démon Java d'origine appliquait la formule : \Énergie (Wh) = Puissance Moyenne (W) * (Temps de Charge (min) / 60)\.*

## ⚙️ 7. Modes de Fonctionnement (FM)
L'ATmega possède plusieurs modes de fonctionnement internes. Le mode principal peut être modifié en envoyant `FM:X` (où X est le numéro du mode) et vérifié avec `FM?`.

| Mode | Désignation officielle | Explication du mode |
| :--- | :--- | :--- |
| `FM:1` | **Direct Charge (Permanent)** | La borne charge dès qu'un véhicule est branché, sans aucune condition. C'est le mode "exécutant bête" que nous forçons par défaut dans Green'Up Link. |
| `FM:2` | **Remote controls (Auto-heures)** | Mode "Heures Creuses / Heures Pleines", basé sur l'entrée contacteur (Contact sec) de la carte pour démarrer/arrêter la charge. |
| `FM:3` | **Smart meter (TIC Linky)** | Pilotage intelligent basé sur la téléinformation du compteur Linky. **Note :** Ce mode n'est en réalité **pas implémenté dans le code de l'ATmega** (firmware 18.04), ce qui explique pourquoi il a été masqué (mis en commentaire) par Legrand dans le code de l'interface Web d'origine. |
| `FM:4` | **Programming (Planning)** | Mode de programmation horaire interne. L'ATmega se base sur des fichiers de calendrier pour déclencher la charge. |
| `FM:5` | **Modbus (DLM)** | Dans ce mode, la borne est pilotée via le bus RS485 (protocole Modbus) par un gestionnaire d'énergie externe (Load Management). |
| `FM:6` | **OCPP** | Mode de supervision Cloud. La borne attend ses ordres du serveur OCPP central. **Attention :** ce mode modifie le comportement interne de l'ATmega (désactive l'autodétection de la TIC, impose une attente d'autorisation bloquante de 30s après la présentation d'un badge, modifie la logique de la commande `Unlock` et force les flags RFID à 1 au démarrage). |

### Fonction "Éco-démarrage" (FM2)
En complément du mode principal, la borne gère un paramètre secondaire dit d'Éco-démarrage :
*   `FM2?` : Interroge l'état actuel de l'Éco-démarrage.
*   `FM2:0` : Désactive la fonction.
*   `FM2:1` : Active l'Éco-démarrage. 

**Explication technique :** Quand `FM2:1` est activé (ce qui revient au même que de basculer physiquement le **DIP Switch 2** sur la carte), la machine d'état de l'ATmega bloque la charge de la voiture tant que les drapeaux internes de la TIC n'indiquent pas que l'on se trouve en Heures Creuses (HC). À noter que ces drapeaux sont lus par l'ATmega, mais ne sont pas exportables ou lisibles par le Raspberry Pi.
