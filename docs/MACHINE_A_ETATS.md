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
**Conséquence grave (Cécité matérielle) :** Lorsque la borne a basculé en State:A suite à un T2CNOK (donc avec T2C:0), elle n'émet **PLUS AUCUN** événement lors du débranchement physique de la voiture (State:L et SBF:0 sont silencieusement ignorés). L'API se retrouve donc complètement aveugle : impossible de savoir si le câble est toujours là ou si l'utilisateur est parti !

### Quirk #4 : Le mode Eco-Start (Heures Creuses) fantôme et son contournement
Normalement, la commande FM2:0 est censée désactiver la fonction éco-démarrage (la charge devrait démarrer instantanément sans attendre les Heures Creuses du TIC). Cependant, les logs montrent que même avec FM2:0, la borne semble rester influencée par le signal TIC. Au passage en Heures Creuses, l'ATmega pousse le passage à State:C de manière inattendue.

Fait particulièrement troublant : la borne peut répondre FM:1 à la commande FM? (indiquant qu'elle est bien en mode Direct Charge permanent), tout en appliquant quand même ce mode éco-start fantôme ! 
**Solution de contournement :** L'envoi explicite de la commande FM:1 (même si la borne indique déjà être dans ce mode) désactive et purge efficacement ce mode fantôme. Cependant, **ATTENTION** : l'envoi de FM:1 (notamment pendant un State:B) désactive purement et simplement toute la détection TIC ! La borne devient incapable de gérer le délestage ou les heures creuses. Il faut donc être très prudent avec cette commande.
