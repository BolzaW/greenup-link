# Guide de Flashage Firmware ATmega2560 (Officiel & Custom)

Ce document décrit la procédure complète, universelle et éprouvée pour flasher un firmware officiel ou personnalisé sur la carte électronique de puissance (microcontrôleur Microchip/Atmel ATmega2560) d'une borne de recharge **Legrand Green'Up Premium**.

---

## 1. Principe de Fonctionnement & Déverrouillage Bootloader

Sur les bornes Legrand Green'Up Premium, la ligne DTR du convertisseur USB-Série n'est **pas reliée** à la broche RESET du microcontrôleur ATmega2560 (sécurité industrielle anti-reboot inopiné en cours de charge).

Pour forcer le microcontrôleur à basculer dans son bootloader série (**Wiring / STK500v2** à **115 200 bauds**), Legrand a implémenté un mécanisme de **Challenge-Response** :

1. L'application envoie une commande série `UpdateAuth:<CRC16-Modbus>` calculée à partir des 5 identifiants uniques de la borne (`Date`, `SW`, `SN`, `HW`, `Ref`).
2. L'ATmega valide le code, répond `UpdateCodeOK\r\n` et arme un drapeau persistant en mémoire vive (`RAM[0x0ACC] = 1`).
3. Dès que l'outil de flash `avrdude` ouvre le port et transmet son premier octet de synchronisation `0x1B`, l'ATmega détecte le drapeau armé et déclenche un **Watchdog Reset** immédiat (`0x0EEC4`).
4. L'ATmega redémarre instantanément dans le bootloader STK500v2 et `avrdude` prend la main pour programmer la Flash.

> [!WARNING]
> **Le code `UpdateAuth` est unique pour CHAQUE borne !**
> Il ne s'agit pas d'un mot de passe universel constructeur. Il est recalculé dynamiquement par la carte à chaque requête en croisant ses identifiants matériels et son numéro de version logicielle active.

---

## 2. Procédure Pas à Pas (Simple et Rapide)

### Étape 1 : Calculer le code et armer le bootloader (`UpdateAuth`)

Tant que `greenup-link` (ou votre superviseur) est actif sur `/dev/ttyUSB0` :

1. **Option A (100% automatique avec le script) :**
   ```bash
   python3 scripts/calc_update_auth.py --api http://localhost:8080 --arm
   ```
   *Le script interroge la borne via l'API REST locale, extrait les 5 paramètres, calcule le CRC16 et envoie la commande `UpdateAuth` à la carte.*

2. **Option B (Manuelle via curl) :**
   Calculez votre code via le script :
   ```bash
   python3 scripts/calc_update_auth.py <DATE> <SW> <SN> <HW> <REF>
   # Exemple : python3 scripts/calc_update_auth.py 42W22 V01.17.78 015184 V02.01.01 058001
   ```
   Puis transmettez la commande :
   ```bash
   curl -X POST http://localhost:8080/api/command -d "UpdateAuth:<CODE_CALCULÉ>"
   ```

3. **Vérification :**
   Confirmez dans les logs de `greenup-link` la validation immédiate par la carte :
   ```text
   [SERIAL_TX] UpdateAuth:xxxxx
   [SERIAL_RX] UpdateCodeOK
   ```

> [!NOTE]
> Le drapeau d'armement `0x0ACC = 1` reste en RAM tant que la carte est sous tension ou jusqu'à la première trame `avrdude`.

---

### Étape 2 : Libérer le port série pour le système

Arrêtez le superviseur pour libérer `/dev/ttyUSB0` au profit d'`avrdude` :

```bash
# Si exécuté dans un terminal / tmux :
pkill -f greenup-link
# (ou Ctrl+C dans le terminal)

# Si exécuté en tant que service systemd :
sudo systemctl stop greenup-link
```

---

### Étape 3 : Lancer la programmation Flash avec `avrdude`

Exécutez `avrdude` avec le programmeur **wiring** à 115 200 bauds :

```bash
sudo avrdude -C /etc/avrdude.conf -v -p atmega2560 -c wiring -P /dev/ttyUSB0 -b 115200 -D -U flash:w:/chemin/vers/<FIRMWARE>.hex:i
```

**Sortie attendue :**
```text
avrdude: AVR device initialized and ready to accept instructions
avrdude: Device signature = 0x1e9801 (probably m2560)
avrdude: reading input file "..."
avrdude: writing flash (146xxx bytes):
Writing | ################################################## | 100% 21.12s
avrdude: verifying ...
avrdude: 146xxx bytes of flash verified
avrdude: safemode: Fuses OK (E:FD, H:D8, L:FF)
avrdude: stk500v2_command(): unknown status 0x01
avrdude: stk500v2_disable(): failed to leave programming mode
avrdude done.  Thank you.
```

> [!TIP]
> Le message final `stk500v2_disable(): failed to leave programming mode` est **tout à fait normal** avec le bootloader Wiring d'Atmel : le microcontrôleur saute directement dans la nouvelle application flashée sans accuser réception de la trame de déconnexion.

---

### Étape 4 : Relancer `greenup-link` et vérifier la version

Relancez le service :
```bash
./target/release/greenup-link
# ou : sudo systemctl start greenup-link
```

Puis vérifiez la prise en compte du nouveau firmware :
```bash
curl -X POST http://localhost:8080/api/command -d "SoftwareVersion?"
```

---

## 3. Spécification Mathématique de la Formule `UpdateAuth`

### 3.1. Les 5 Paramètres d'Entrée

Les valeurs sont interrogées sur le port série USB (USART1) via les requêtes officielles :

| Paramètre | Commande Série | Format Attendu | Exemple |
| :--- | :--- | :--- | :--- |
| **`Date`** | `WeekYearProduction?` | 5 caractères ASCII : `SSWAA` | `42W22` (semaine 42, année 2022) |
| **`SW`** | `SoftwareVersion?` | 9 caractères normalisés : `Vxx.yy.zz` | `V01.17.78` ou `V01.18.10` |
| **`SN`** | `SerialNumber?` | 6 chiffres ASCII | `015184` |
| **`HW`** | `HardwareVersion?` | 9 caractères normalisés : `Vxx.yy.zz` | `V02.01.01` |
| **`Ref`** | `Reference?` | 6 chiffres ASCII | `058001` |

> [!IMPORTANT]
> La version logicielle `SW` doit correspondre à **la version active sur la carte au moment de l'envoi de la commande**, et non à la version du firmware que vous vous apprêtez à flasher !

---

### 3.2. Formule d'Entrelacement des Caractères (Clé de 28 octets)

L'algorithme Legrand construit une chaîne ASCII intermédiaire de **28 caractères** en entrelaçant précisément les indices des 5 paramètres :

```text
Position :  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22  23  24  25  26  27
Origine  : Date SW  SN  HW Ref Date SW  SN  HW Ref Date SW  SN  HW Ref Date SW  SN  HW Ref  SW  SN  HW Ref  SW  SN  HW Ref
Indice   : [0] [8] [2] [8] [1] [4] [1] [1] [7] [3] [1] [4] [5] [1] [4] [3] [2] [0] [2] [5] [5] [4] [4] [2] [7] [3] [5] [0]
```

**Tableau détaillé d'entrelacement :**

| Bloc | Indice Clé | Variable | Indice dans la chaîne | Rôle / Valeur extraite |
| :--- | :--- | :--- | :--- | :--- |
| **1** | `0` | `Date` | `[0]` | Premier chiffre de la semaine de prod |
| | `1` | `SW` | `[8]` | Dernier chiffre du numéro de version |
| | `2` | `SN` | `[2]` | 3ème chiffre du numéro de série |
| | `3` | `HW` | `[8]` | Dernier chiffre de version HW |
| | `4` | `Ref` | `[1]` | 2ème chiffre de référence |
| **2** | `5` | `Date` | `[4]` | Dernier chiffre de l'année de prod |
| | `6` | `SW` | `[1]` | Premier chiffre majeur SW |
| | `7` | `SN` | `[1]` | 2ème chiffre du numéro de série |
| | `8` | `HW` | `[7]` | Avant-dernier chiffre HW |
| | `9` | `Ref` | `[3]` | 4ème chiffre de référence |
| **3** | `10` | `Date` | `[1]` | 2ème chiffre de la semaine de prod |
| | `11` | `SW` | `[4]` | Premier chiffre mineur SW |
| | `12` | `SN` | `[5]` | Dernier chiffre du numéro de série |
| | `13` | `HW` | `[1]` | Premier chiffre majeur HW |
| | `14` | `Ref` | `[4]` | 5ème chiffre de référence |
| **4** | `15` | `Date` | `[3]` | Premier chiffre de l'année de prod |
| | `16` | `SW` | `[2]` | 2ème chiffre majeur SW |
| | `17` | `SN` | `[0]` | Premier chiffre du numéro de série |
| | `18` | `HW` | `[2]` | 2ème chiffre majeur HW |
| | `19` | `Ref` | `[5]` | Dernier chiffre de référence |
| **5** | `20` | `SW` | `[5]` | 2ème chiffre mineur SW |
| | `21` | `SN` | `[4]` | 5ème chiffre du numéro de série |
| | `22` | `HW` | `[4]` | Premier chiffre mineur HW |
| | `23` | `Ref` | `[2]` | 3ème chiffre de référence |
| **6** | `24` | `SW` | `[7]` | Premier chiffre patch/build SW |
| | `25` | `SN` | `[3]` | 4ème chiffre du numéro de série |
| | `26` | `HW` | `[5]` | 2ème chiffre mineur HW |
| | `27` | `Ref` | `[0]` | Premier chiffre de référence |

---

### 3.3. Calcul du CRC-16 Modbus

La chaîne de 28 octets ASCII résultante est passée à l'algorithme standard **CRC-16 Modbus** :
- **Polynôme (inversé / LSB first)** : `0xA001` (équivalent à `0x8005` normal).
- **Valeur initiale du registre** : `0xFFFF`.
- **Sens de traitement** : bit de poids faible en premier (LSB first).
- **Résultat** : entier non signé 16 bits (0 à 65535) converti en décimal ASCII.

---

### 3.4. Vecteur de Test de Référence (Pour Validation)

Pour valider le bon fonctionnement de votre implémentation ou calcul :

- **Paramètres :**
  - `WeekYearProduction` = `"42W22"`
  - `SoftwareVersion` = `"V01.17.78"`
  - `SerialNumber` = `"015184"`
  - `HardwareVersion` = `"V02.01.01"`
  - `Reference` = `"058001"`
- **Clé entrelacée générée :**
  `4851520100214002102178087110`
- **Code CRC16 Modbus attendu :**
  `43417`
- **Commande résultante :**
  `UpdateAuth:43417`

---

## 4. Script Universel Autonome : `calc_update_auth.py`

Le script officiel est présent dans [`greenup-link/scripts/calc_update_auth.py`](file:///c:/Users/rapha/Desktop/Maj%20borne%20legrand/retro_ingenierie/legrand/greenup-link/scripts/calc_update_auth.py). Il ne dépend d'**aucune bibliothèque tierce** (utilise uniquement la bibliothèque standard Python 3).

### Modes d'utilisation :

#### 1. Mode Direct (arguments positionnels) :
```bash
python3 scripts/calc_update_auth.py 42W22 V01.17.78 015184 V02.01.01 058001
```

#### 2. Mode Flags explicites :
```bash
python3 scripts/calc_update_auth.py --date 42W22 --sw V01.17.78 --sn 015184 --hw V02.01.01 --ref 058001
```

#### 3. Mode Automatique via l'API REST `greenup-link` :
```bash
# Lit les paramètres sur l'API et affiche le code :
python3 scripts/calc_update_auth.py --api http://localhost:8080

# Lit les paramètres, calcule le code, et arme directement le bootloader :
python3 scripts/calc_update_auth.py --api http://localhost:8080 --arm
```

#### 4. Mode Interactif :
Lancez simplement le script sans arguments :
```bash
python3 scripts/calc_update_auth.py
```
Le script vous posera interactivement les 5 questions.
