# Kit de Communication Alternatif "DIY" pour Borne Legrand Green'up Premium

Ce document liste l'ensemble du matériel, des références et des préconisations pour fabriquer un **kit de communication alternatif économique** compatible avec la borne Legrand Green'up Premium, en remplacement du kit officiel Legrand (réf. **059056**, vendu ~600 €).

Ce kit est conçu pour faire tourner le firmware léger **Green'Up Link** (écrit en Rust), offrant une interface moderne, une API locale et une intégration directe avec EVCC et Home Assistant.

---

## 1. Nomenclature Matérielle (BOM)

### A. Le Calculateur (Single Board Computer)

Deux options selon que votre borne est couverte en Wi-Fi ou reliée par câble Ethernet RJ45 :

| Composant | Référence / Modèle | Rôle | Avantages | Prix indicatif |
| :--- | :--- | :--- | :--- | :--- |
| **Option 1 (Recommandée - Wi-Fi)** | **Raspberry Pi Zero 2 W** | Fait tourner l'OS Linux et Green'Up Link. | Ultra compact, chauffe très peu (< 2W), faible coût, Wi-Fi 2.4 GHz intégré. | ~18 € - 22 € |
| **Option 2 (Filaire RJ45)** | **Raspberry Pi 3 Model B+** ou **Pi 4 (1 ou 2 Go)** | Alternative si la borne est raccordée par un câble réseau Ethernet physique. | Port Ethernet Gigabit intégré, ports USB-A standards. | ~35 € - 45 € |

> [!TIP]
> Avec **Green'Up Link en Rust**, la consommation mémoire est inférieure à 50 Mo de RAM. Les 512 Mo du **Pi Zero 2 W** sont donc largement suffisants et idéaux pour éviter toute surchauffe estivale dans le boîtier fermé de la borne.

---

### B. Module d'Alimentation DC-DC (Solution du commerce)

Le kit officiel utilise la tension continue fournie par l'alimentation industrielle interne de la borne (bloc métallique Mean Well délivrant généralement **12V DC**, parfois 24V DC selon les révisions).

Pour remplacer le petit PCB propriétaire Legrand par un module standard du commerce :

| Réf / Type | Caractéristiques requises | Mots-clés de recherche (Amazon / AliExpress / RS) | Prix indicatif |
| :--- | :--- | :--- | :--- |
| **Convertisseur Buck étanche 12V/24V vers 5V 3A (15W)** | - Entrée : 8V - 32V DC<br>- Sortie : 5.0V / 3A (15W)<br>- Format : Boîtier plastique résiné noir (étanche IP67)<br>- Connecteur de sortie : Câble **Micro-USB mâle** (pour Pi Zero 2W ou Pi 3B+) ou **USB-C mâle** (pour Pi 4) | *Convertisseur abaisseur 12V 5V 3A Micro USB étanche* / *DC-DC Step Down Converter 12V to 5V 3A Micro USB* | ~6 € - 9 € |

*Exemples de modèles courants : CPT Step-Down Converter, DROK Waterproof DC-DC Buck Converter 12V to 5V Micro-USB.*

---

### C. Connectique & Faisceau Plug-and-Play

Pour brancher l'alimentation du kit sur le connecteur d'origine de la borne **sans couper ni dénuder le faisceau d'origine Legrand** :

| Composant | Spécification | Rôle | Prix indicatif |
| :--- | :--- | :--- | :--- |
| **Connecteur d'alimentation borne** | Connecteur type **Molex Mini-Fit Jr.** (ou Micro-Fit selon série) **4 broches (2x2)** | S'enfiche directement dans la fiche d'alimentation auxiliaire de la borne. | ~2 € - 3 € |
| **Câble USB pour liaison série** | - Pour Pi Zero 2 W : Adaptateur **Micro-USB mâle vers USB-A femelle (câble OTG)** + câble USB court.<br>- Pour Pi 3B+/4 : Câble standard court **USB-A mâle** vers la prise de la carte de puissance. | Relie le port USB du Pi au contrôleur de puissance (ATmega2560). | ~3 € |
| **Carte Micro-SD** | **SanDisk High Endurance** ou **Kingston Endurance** (16 Go ou 32 Go) | Support de l'OS. Les cartes "Endurance" résistent aux écarts de température extrêmes d'une borne extérieure. | ~8 € - 10 € |

---

### D. Support Mécanique (Châssis)

| Composant | Réalisation | Remarques | Coût |
| :--- | :--- | :--- | :--- |
| **Platine de montage 3D** | Fichier STL à imprimer en **PETG** ou **ABS/ASA** | Remplace la tôle métallique d'origine. Se fixe sur les entretoises existantes de la borne et accueille le Pi + le boîtier DC-DC. *(Ne pas utiliser de PLA qui ramollit à 55°C en plein été)*. | ~1 € - 2 € |
| **Visserie** | 4 vis M2.5 (pour fixer le Pi) + vis de fixation châssis | Fixation mécanique propre. | ~1 € |

---

## 2. Récapitulatif Budgétaire

| Poste | Solution Officielle Legrand | Solution DIY Alternative (Pi Zero 2 W) |
| :--- | :--- | :--- |
| Carte électronique / Calculateur | Raspberry Pi 3B+ propriétaire | Raspberry Pi Zero 2 W (~20 €) |
| Alimentation 5V | PCB propriétaire spécifique | Régulateur DC-DC 12V->5V 3A étanche (~8 €) |
| Connectique & Câbles | Fournis | Connecteur Molex + câbles USB (~5 €) |
| Carte SD | 16 Go standard | 32 Go High Endurance (~8 €) |
| Châssis de fixation | Tôle acier pliée | Platine imprimée 3D PETG (~2 €) |
| **TOTAL** | **~500 € à 700 €** | **~43 €** |

---

## 3. Schéma de Raccordement Synoptique

```text
[ Alimentation Borne 12V/24V ]
           |
           | (Faisceau d'origine)
           v
[ Prise Molex 4 broches (2x2) ]
     |                  |
     | (+12V DC)        | (GND / Masse)
     +--------+   +-----+
              |   |
              v   v
[ Module Buck DC-DC 12V -> 5V 3A ]
              |
              | (Sortie 5V régulée - Micro-USB)
              v
[ Raspberry Pi Zero 2 W ] <===== (Port USB OTG / Câble série) =====> [ Carte de puissance Legrand ]
    (Green'Up Link)                                                         (ATmega2560)
           |
           +-- Wi-Fi local --> [ Réseau Domotique / Box / EVCC ]
```

---

## 4. Précautions & Montage

1. ⚠️ **Sécurité électrique (230V)** : Toute intervention à l'intérieur de la borne doit se faire **disjoncteur général coupé** au tableau électrique.
2. **Polarité d'alimentation** : Vérifier au multimètre la tension (12V ou 24V) et la polarité (+ et -) sur le connecteur blanc avant d'y brancher le module DC-DC.
3. **Étanchéité** : Veiller à ce que le joint du capot de la borne soit parfaitement repositionné au remontage pour éviter toute infiltration d'humidité.
