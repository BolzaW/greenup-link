# Spécification et Rapport d'Audit : Firmware Patché `FirmwareBoardA-V01_17_77c.hex` (Télémétrie TIC via `MacAdr?`)

Ce document certifie et détaille la conception, l'injection et la vérification binaire du firmware personnalisé **`FirmwareBoardA-V01_17_77c.hex`** pour la carte électronique de puissance (microcontrôleur Microchip/Atmel ATmega2560) de la borne de recharge Legrand Green'Up Premium.

---

## 1. Synthèse des Modifications Réalisées

Le firmware officiel Legrand `FirmwareBoardA-V01_17_77.hex` a été patché de manière **strictement chirurgicale** sans décalage d'adresses dans les tables de vecteurs, le code existant ou les structures en mémoire Flash :

1. **Détournement de la commande série `MacAdr?`** :
   - L'ancienne boucle d'émission série à l'adresse Flash `0x0B630` (qui renvoyait une chaîne statique inutile de 12 zéros `000000000000`) a été remplacée par un saut (`jmp 0x23A70`) vers notre routine d'extraction télémétrique TIC.
   - **Branchement direct selon le mode Linky (`0x0B33`)** : la réponse commence directement par `STANDARD`, `HISTORIQUE` ou `AUCUN` (sans préfixe lourd `MODE:`).
   - **Homogénéisation linguistique et normalisation Enedis** :
     - Drapeaux système francisés : `ACTIF:<0|1>` (décodage TIC actif) et `ARME:<0|1>` (autodétection armée).
     - Champs Linky désignés par leurs acronymes normalisés Enedis : `URMS1`, `IRMS1`, `PREF`, `IINST`, `ISOUSC`.
   - **Intégration de la tension `URMS1`** : stockée en mémoire vive par la borne (`RAM 0x0217:0x0218`), elle est restituée en mode Standard.
   - **Retrait de `CCTIC`** : non renvoyé dans `MacAdr?` car déjà disponible via la commande dédiée `CCTIC?`.
2. **Respect absolu du protocole série Legrand** :
   - Préfixe officiel conservé : `MacAdr:`.
   - Séparateur de champs : point-virgule `;`.
   - Terminaison de ligne standard : `\r\n` (assurée par l'épilogue officiel de la borne à `0x0B394`).
3. **Mise à jour de la version du firmware** :
   - La chaîne de réponse à `SoftwareVersion?` (à l'adresse Flash `0x232C0`) passe de `"011777"` à `"011778"`.
   - *Note technique* : le parseur Legrand convertissant chaque caractère en valeur numérique avec `subi rXX, 0x30`, l'utilisation d'une lettre (comme `'c'`) tronquait l'affichage en `V01.17.70`. L'incrément propre en `'8'` restitue parfaitement `V01.17.78`.
4. **Indépendance vis-à-vis du Bluetooth** :
   - La commande `MacAdr?` n'est décodée que sur le port série USB (USART1 relié au Raspberry Pi / automate tiers). Le module Bluetooth (sur USART0) et l'application mobile Legrand continuent de fonctionner sans aucune altération.

---

## 2. Spécification des Trames Renvoyées

### 2.1. Format général

```text
MacAdr:<STANDARD|HISTORIQUE|AUCUN>;ACTIF:<0|1>;ARME:<0|1>[;<CHAMPS_SPECIFIQUES>]\r\n
```

### 2.2. Détail des variables SRAM interrogées

| Champ | Adresse SRAM | Taille / Type | Description |
| :--- | :--- | :--- | :--- |
| **`ACTIF`** | `0x0BA2` | 1 octet (u8) | `1` = Décodage TIC actif, `0` = Inactif / Absent (`AUCUN`) |
| **Mode** | `0x0B33` | 1 octet (u8) | `0` = `HISTORIQUE` (1200 bauds), `1` = `STANDARD` (9600 bauds) |
| **`ARME`** | `0x02EF` | 1 octet (u8) | `1` = Autodétection baudrate armée / en cours, `0` = Verrouillé |
| **`URMS1`** | `0x0217:0x0218` | 2 octets (u16 LE) | Tension RMS monophasée en Volts (Standard) |
| **`IRMS1`** | `0x0219:0x021A` | 2 octets (u16 LE) | Courant RMS monophasé en Ampères (Standard) |
| **`PREF`** | `0x0215:0x0216` | 2 octets (u16 LE) | Puissance souscrite en kVA (Standard, `65535` si non reçue) |
| **`IINST`** | `0x0207:0x0208` | 2 octets (u16 LE) | Intensité instantanée en Ampères (Historique) |
| **`ISOUSC`** | `0x0209:0x020A` | 2 octets (u16 LE) | Intensité souscrite en Ampères (Historique) |

---

### 2.3. Cas pratiques de réponse

#### Cas 1 : Compteur Linky en Mode Historique (`0x0BA2 == 1 && 0x0B33 == 0`)
```text
MacAdr:HISTORIQUE;ACTIF:1;ARME:0;IINST:18;ISOUSC:45\r\n
```
*(Validé en conditions réelles sur la borne : `MacAdr:HISTORIQUE;ACTIF:1;ARME:0;IINST:2;ISOUSC:30`)*

#### Cas 2 : Compteur Linky en Mode Standard (`0x0BA2 == 1 && 0x0B33 == 1`)
```text
MacAdr:STANDARD;ACTIF:1;ARME:0;URMS1:234;IRMS1:14;PREF:9\r\n
```

#### Cas 3 : TIC déconnectée / inactive (`0x0BA2 == 0`)
```text
MacAdr:AUCUN;ACTIF:0;ARME:0\r\n
```
*(Si la recherche automatique est en cours de cycle entre 1200 et 9600 bauds, `ARME:1` sera renvoyé)*

---

## 3. Architecture Technique du Patch Assembleur (AVR6 / ATmega2560)

Le code injecté à l'adresse Flash `0x23A70` utilise les fonctions internes du firmware de la borne tout en appliquant une hygiène de registres rigoureuse :

1. **Trampoline d'entrée (`0x0B630` - `0x0B655`)** :
   ```assembly
   0x0B630: 0D 94 38 1D   jmp 0x23A70     ; Saut 22 bits vers la page flash 2
   0x0B634: 00 00 ...     nop (17 fois)   ; Effacement propre de la boucle d'origine
   ```
2. **Gestion de la mémoire Flash au-delà de 128 Ko (Page 2)** :
   - L'ATmega2560 possède 256 Ko de Flash. Au-dessus de `0x20000`, l'instruction `lpm` classique reboucle sur les 128 premiers Ko.
   - Notre routine implémente `print_pstr` avec l'instruction étendue `elpm r22, Z+` en configurant le registre `RAMPZ` (I/O `0x3B`) à `2`.
   - **Sécurisation des interruptions et sous-fonctions** : `RAMPZ` est restauré à `0` immédiatement après la lecture de chaque caractère, évitant tout effet de bord avec les ISR du système.
3. **Préservation des pointeurs ABI AVR** :
   - La fonction `Serial1.write(r22)` à `0x1C7DE` (`HardwareSerial::write`) utilise le registre `Z` (`r30:r31`) pour charger les registres UART. Pour éviter toute altération du pointeur de chaîne, `r30:r31` est sauvegardé sur la pile (`push r30`, `push r31` / `pop r31`, `pop r30`) à chaque émission de caractère.
   - Le pointeur de cadre de pile `Y` (`r28:r29`) est préservé symétriquement.
   - La valeur du mode TIC (`0x0B33`) est relue directement en SRAM avant les branchements conditionnels, garantissant l'immunité contre les écrasements de registres volatils par `Print::printNumber`.
4. **Sortie et épilogue** :
   - La fin de la routine saute directement vers `0x0B394` (`jmp 0x0B394`), qui appelle `Serial1.println()` (envoi de `\r\n`) puis retourne dans la boucle principale d'attente des commandes série (`0x0AFE2`).

---

## 4. Rapport d'Audit Binaire Différentiel

L'audit différentiel automatisé entre `FirmwareBoardA-V01_17_77.hex` et `FirmwareBoardA-V01_17_77c.hex` certifie une intervention 100% chirurgicale :

```text
=== AUDIT DIFFÉRENTIEL (Original vs Patché) ===
Taille binaire original:  146028 octets (0x00000 - 0x23A6C)
Taille binaire patché:    146356 octets (0x00000 - 0x23BB4)

Nombre de modifications dans la zone originale : EXACTEMENT 2

1. Modification #1 (Trampoline MacAdr?) :
   Adresse : 0x0B630 - 0x0B656 (38 octets)
   Original : 42 e2 e4 2e 42 e0 f4 2e 5e e2 c5 2e 52 e0 d5 2e f7 01 61 91 7f 01 84 ec 9f e0 0e 94 ef e3 ce 14 df 04 b1 f7 9f ce
   Patché   : 0d 94 38 1d 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
   Rôle     : Saut 22 bits vers 0x23A70 suivi de 17 NOP.

2. Modification #2 (Version logicielle) :
   Adresse : 0x232C5 - 0x232C6 (1 octet)
   Original : 0x37 ('7') -> "011777\0"
   Patché   : 0x38 ('8') -> "011778\0" (restitué "V01.17.78")
   Rôle     : Identification formelle de la version patchée via "SoftwareVersion?".

3. Zone de code ajoutée :
   Adresse : 0x23A6C - 0x23A70 (4 octets) : alignement 0xFF 0xFF 0xFF 0xFF
   Adresse : 0x23A70 - 0x23BB4 (324 octets) : routine de télémétrie TIC & chaînes de constantes

Aucun autre octet n'a été altéré. L'intégrité de toutes les fonctions de sécurité, de pilotage du relais de charge, de mesure d'énergie et de communication Bluetooth est garantie.
```

---

## 5. Fichiers Disponibles

- **Fichier HEX prêt à flasher** : `hex/FirmwareBoardA-V01_17_77c.hex`
- **Fichier binaire brut** : `scratch/fw/fw1777c.bin`
- **Code source assembleur du patch** : `scratch/fw/patch_macadr.s`
- **Script de génération et d'audit** : `scratch/fw/build_patch.py`
