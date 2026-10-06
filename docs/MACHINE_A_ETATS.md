# Machine à États de la Borne Legrand (Green'Up Premium)

Ce document détaille le fonctionnement de la machine à états propriétaire du firmware ATmega Legrand. 
Les états renvoyés par la commande série State: ne sont pas de simples états électriques IEC 61851, mais une véritable machine à états comportementale.

## 1. Liste complète des états (State:X)

Cette sémantique a été établie avec certitude par rétro-ingénierie du firmware 18.04 de la carte de puissance et recoupement avec l'implémentation Java d'origine.

| État | Interprétation matérielle | Correspondance OCPP (selon l'ATmega) |
|---|---|---|
| **A** | Repos, aucun véhicule connecté | Available |
| **B** | Prise **T2S** occupée, en attente d'autorisation | SuspendedEVSE déclenche StartTransaction |
| **C** | T2S → charge suspendue côté véhicule | SuspendedEV |
| **D** | T2S → **charge en cours** | Charging |
| **E** | T2S → **charge en cours** (variante) | Charging |
| **F** | Prise **domestique 2P+T** occupée, en attente | SuspendedEVSE déclenche StartTransaction |
| **G** | Domestique → **charge en cours** | Charging |
| **H** | Domestique → **charge en cours** (variante) | Charging |
| **I** | T2S → suspendu véhicule (variante) | SuspendedEV |
| **J** | T2S → suspendu borne | SuspendedEVSE |
| **K** | Suspendu borne | SuspendedEVSE |
| **L** | **Débranchement véhicule** / fin de session | arrêt, motif EVDisconnected |
| **M** | **Arrêt local** (bouton) | arrêt, motif Local |
| **N** | Suspendu borne (T2S) | SuspendedEVSE |
| **O** | Suspendu borne (domestique) | SuspendedEVSE |
| **P** | Arrêt local en cours | arrêt, motif Local |
| **R** | **Défaut** (verrou, surcharge, contacteur) | Faulted, Other |
| **S** | **Défaut** (contacteur, CP) | Faulted, Other |
| **T** | **Défaut** (court-circuit CP) | Faulted, Other |
| **U** | **Coupure secteur** | PowerLoss |
| **V** | **Coupure secteur** (variante) | PowerLoss |
| **W** | État transitoire, émis mais jamais stable | ignoré |
| **X** | Redémarrage | ignoré |
| **Y** | **Veille** profonde (commande Slp:1) | Unavailable |
| **Z** | Initialisation / mode production | Unavailable |

---

## 2. Séquencements des états

```
                        +-----------------------------------+
                        |                                   |
        Slp:1           v                                   |
   Y <---------------  [A]  Repos / Available               |
   |                   / | \                                |
   | Slp:0            /  |  \                               |
   +-----------------+   |   +--------------------+         |
                     |   |                        |         |
              T2S    v   v DOM                    |         |
                    [B]  [F]  <--------+          |         |
                   / | \  / | \        |          |         |
                  /  |  \/  |  \       |          |         |
                 /   |  /\  |   \      |          |         |
                v    v v  v v    v     |          |         |
              [L]  [C]  [G]  [H]  [R]  |          |         |
               |   /|\    \   /        |          |         |
               |  / | \    \ /         |          |         |
               | v  v  v    v          |          |         |
               |[D] [E] [N] [B]--------+          |         |
               | | \ / |                          |         |
               | |  X  |                          |         |
               | v v v v                          |         |
               |[I]<->[J]                         |         |
               |  \    /                          |         |
               |   v  v                           |         |
               +-->[L]--> [R] --------------------+---------+
                                                            |
      [K] --> [R] --> [A]      [M] --> [P] --> [M]          |
      [O] --> [A]              [P] --> [R] --> [A] ---------+
```

## 3. Incohérences constatées dans les logs (Quirks)

Bien que la liste ci-dessus soit extraite du code source de la borne, l'observation en direct (logs de charge) a mis en évidence quelques subtilités du moteur d'états qui devront être gérées par greenup-driver.

### Quirk #1 : Passage forcé en State:C par les Heures Creuses
Lors d'un démarrage retardé (la voiture est branchée mais attend son propre planning), la borne est en State:B. 
Si un évènement TIC (ex: passage en Heures Creuses) survient, l'ATmega passe en State:C, **même si la voiture (broche CP) n'a rien réclamé**. C'est ensuite, quand la voiture lance sa charge, que l'état bascule en State:E.

### Quirk #2 : L'anomalie de OCPPStatus:SuspendedEVSE
Lorsqu'on interroge l'état OCPP interne via OCPPPS? après que la **voiture** ait coupé la charge (passage transitoire par State:I puis retour à State:B), la borne a le défaut de répondre SuspendedEVSE au lieu de SuspendedEV. Le driver Rust devra parfois "corriger" ce diagnostic en mémorisant l'historique des états.

### Quirk #3 : State:A forcé malgré un câble branché après arrêt local
Si l'on force l'arrêt de la charge depuis la borne via la commande T2CNOK, la borne effectue sa séquence de clôture (passant par W puis M) et retombe ensuite à l'état A (Available). **Cependant, le câble côté véhicule est toujours physiquement branché.** Logiquement, la borne devrait retourner en état B (Prise occupée, en attente d'autorisation), mais elle se déclare complètement libre.
**Conséquence (Verrouillage logiciel de la prise) :** La commande `T2CNOK` (qui passe la variable interne `T2C` à `0`) force non seulement la borne en `State:A`, mais **verrouille la machine à états de la prise Type 2 dans cet état**. Tant que `T2C:0`, absolument tous les événements physiques sur cette prise (insertion, retrait du câble, appui sur le bouton) sont ignorés par le firmware. L'API est donc aveugle à l'état physique du câble, car la prise T2S est logiciellement désactivée en attendant d'être réarmée par un `T2COK`.
**Solution de contournement (Polling Analogique) :** Même si les événements asynchrones sont verrouillés, l'API peut interroger directement le convertisseur analogique-numérique (ADC) de la carte avec la commande `CP?`. Celle-ci renvoie la tension brute en Volts sur la broche Control Pilot : `12` (Débranché), `9` (Branché/State B), `6` (En charge/State C). Poller `CP?` permet de retrouver la vue !

### Quirk #4 : Le mode Eco-Start (Heures Creuses) fantôme et son contournement
Normalement, la commande FM2:0 est censée désactiver la fonction éco-démarrage (la charge devrait démarrer instantanément sans attendre les Heures Creuses du TIC). Cependant, les logs montrent que même avec FM2:0, la borne semble rester influencée par le signal TIC. Au passage en Heures Creuses, l'ATmega pousse le passage à State:C de manière inattendue.

Fait particulièrement troublant : la borne peut répondre FM:1 à la commande FM? (indiquant qu'elle est bien en mode Direct Charge permanent), tout en appliquant quand même ce mode éco-start fantôme ! 
**Solution de contournement :** L'envoi explicite de la commande FM:1 (même si la borne indique déjà être dans ce mode) désactive et purge efficacement ce mode fantôme. Cependant, **ATTENTION** : l'envoi de FM:1 (notamment pendant un State:B) désactive purement et simplement toute la détection TIC ! La borne devient incapable de gérer le délestage ou les heures creuses. Il faut donc être très prudent avec cette commande.

### Quirk #6 : Le bug du parseur `CC:` et le crash sous 6A
L'analyse des consignes de courant a mis en évidence deux failles matérielles / logicielles consécutives :
1. **Le parseur `CC:` est buggé** : Il attend strictement 2 chiffres. Si l'on envoie `CC:9` au lieu de `CC:09`, la borne l'interprète mal et applique une consigne de `0A` (elle répond d'ailleurs `CC:00`).
2. **Crash sous 6A** : La norme IEC 61851 impose un courant minimum de 6A. Si la borne reçoit une consigne strictement inférieure à 6A (comme `CC:05`, ou `0A` à cause du bug précédent), elle n'arrête pas proprement la charge. Le courant chute au minimum matériel (~6.8A), puis au bout de 8 secondes la borne panique. Elle émet le défaut `E:0010` (`State:R`), clôture la session, et **redémarre complètement** (`State:X`) !

Le driver `greenup-everest` devra donc systématiquement formater ses consignes sur 2 chiffres (ex: `CC:06`). **De plus, bien que `CC:06` ne fasse pas crasher la borne, l'électronique de régulation est incapable de descendre physiquement sous ~6.8A.** Par sécurité et pour assurer une régulation saine, le driver devra imposer une limite logicielle basse stricte de **`7A`**.

### Quirk #7 : L'interruption par `Unlock` et l'auto-validation du mode `FM:1`
La commande `Unlock` permet de forcer proprement l'arrêt d'une charge en cours. Elle déclenche la séquence de fin (`State:W` -> Envoi du résumé -> `State:A`).
Cependant, puisque le câble est physiquement toujours branché, la borne détecte immédiatement le contacteur (`SBF:1`) et repasse en `State:B`.

**Le problème de l'auto-validation :**
En mode `FM:1` (Direct Charge), la borne est programmée pour valider *automatiquement* l'état `B`. Sans intervention externe, elle passe de suite en `State:C` puis reprend la charge (`State:E`). Ce mode `FM:1` nous prive donc du contrôle de l'autorisation : la borne décide de charger d'elle-même.
Pour implémenter une borne intelligente (Smart Charging) où le driver décide *quand* la charge doit démarrer, le mode `FM:1` n'est probablement pas adapté. Il faudra explorer d'autres modes (comme le mode OCPP `FM:6` ou l'activation de la gestion RFID) qui maintiennent la borne bloquée en `State:B` en attente d'une autorisation logicielle explicite.
### Quirk #8 : La découverte majeure du pilotage Start/Stop via `SBOK` / `SBNOK`
Le pilotage intelligent (Smart Charging) nécessite de pouvoir mettre en pause et reprendre une charge sans verrouiller le système. L'utilisation d' `Unlock` boucle à l'infini (voir Quirk #7), et `T2CNOK` aveugle la machine à état (Quirk #3).
**La solution ultime réside dans les commandes `SBOK` et `SBNOK`.**
Ces commandes simulent un appui logiciel sur le gros bouton physique STOP/START de la façade (qui est lui-même lié au contacteur de présence câble `SBF`).

*   Envoi de **`SBNOK`** : La borne croit qu'on a appuyé sur le bouton STOP. Elle passe immédiatement en `State:W` (Arrêt en cours), envoie le ticket de session, puis se stabilise sagement en **`State:M`** (Arrêt manuel). Elle ne boucle pas, elle attend.
*   Envoi de **`SBOK`** : La borne croit qu'on a appuyé sur START (ou branché le câble). Elle repasse en `State:A`, détecte la prise (`SBF:1`), émet `Start`, passe en `State:B` puis enclenche la charge (`State:C`).

C'est la mécanique **parfaite** pour piloter les sessions de charge et de délestage pour l'intégration EVerest / Home Assistant, sans avoir à subir les effets secondaires des autres commandes d'interruption !

### Mappage Standard EVCC / EVerest (Niveau 2)

Pour exposer une machine à état propre et stable à un superviseur (comme EVCC ou EVerest), nous utilisons une couche d'abstraction (Niveau 2) qui traduit les états natifs Legrand en états standardisés de la norme IEC 61851 : **Disconnected_A**, **Connected_B**, **Charging_C**, **Error_E** (Défaut mineur/récupérable), et **Faulted_F** (Défaut matériel fatal).

La logique choisie pour le code du driver `greenup-everest` est la suivante :

**Cas Particulier (Borne verrouillée logiciellement) :**
Si `T2C:0` : La borne est désactivée et sa machine à état est aveugle (`State` reste bloqué à `A`). L'état EVCC est déduit exclusivement de la tension du Control Pilot (`CP?`) :
*   `CP:12` ➡️ **Disconnected_A**
*   `CP:9` ➡️ **Connected_B**
*   `CP:6` ➡️ **Charging_C**
*(Note de conception : Le driver devra repasser `T2C:1` lorsqu'il voudra réautoriser la charge).*

**Cas Nominal (T2C:1) :**
La machine à état Legrand est cohérente et peut être traduite directement :
*   `State:A` (Repos) ➡️ **Disconnected_A**
*   `State:L` (Débranchement) ➡️ **Disconnected_A** *(transitionne automatiquement vers A)*
*   `State:B` (Connecté, attente borne/TIC) ➡️ **Connected_B**
*   `State:C` (Attente véhicule / Prêt) ➡️ **Connected_B**
*   `State:I` (Interrompu par EV) ➡️ **Connected_B** *(attention, reboucle automatiquement vers A->B->C, mais reste logique B)*
*   `State:W` (Arrêt en cours) ➡️ **Connected_B**
*   `State:M` (Arrêt manuel / via `SBNOK`) ➡️ **Connected_B** *(état stable tant que le véhicule n'est pas débranché/rebranché)*
*   `State:D` / `State:E` (En charge) ➡️ **Charging_C**
*   `State:R` / `State:X` (Défaut / Reboot) ➡️ **Error_E**
*   `State:V` (Coupure d'alimentation fatale) ➡️ **Faulted_F** *(survient avec E:0012 juste avant l'extinction)*
