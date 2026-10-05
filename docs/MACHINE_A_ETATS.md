# Machine à États de la Borne Legrand (Green'Up Premium)

Ce document détaille le fonctionnement de la machine à états propriétaire du firmware ATmega Legrand. 
Bien que la borne utilise le standard **IEC 61851** pour la communication physique (Control Pilot) avec la voiture, **les états renvoyés par la commande série `State:` NE SONT PAS les états bruts IEC 61851**.

L'ATmega implémente une machine d'état transactionnelle et comportementale de plus haut niveau, qui présente plusieurs particularités et incohérences.

## 1. Liste des états connus (`State:X`)

| État Legrand | Traduction IHM | Interprétation & Comportement Physique |
| :---: | :--- | :--- |
| **A** | Libre / Prêt | Borne disponible pour une nouvelle transaction. Attention : **le câble peut être physiquement branché**. |
| **B** | Prise Branchée | Câble connecté. La charge ne démarre pas (planning différé côté voiture ou côté borne). |
| **C** | Attente du véhicule | La borne autorise la charge (PWM actif), mais la voiture ne tire pas de courant (le contacteur de puissance n'est probablement pas fermé côté voiture). |
| **D** | Charge en cours | Transfert d'énergie en cours. |
| **E** | Charge en cours | Transfert d'énergie en cours. Apparaît lors d'un "Début de planning voiture" après une attente. |
| **I** | Interruption | État transitoire. Apparaît quand **la voiture** décide d'arrêter la charge (batterie pleine ou fin de son planning). Retombe généralement sur `State:B` ensuite. |
| **W** | Arrêt en cours (Waiting) | État transitoire. Apparaît quand **la borne** force l'arrêt de la charge (via commande `T2CNOK`). Immédiatement suivi de la trame de résumé de session (`WT:..:CT:..:EVplug:..`). |
| **M** | Arrêt terminé | Apparaît juste après le résumé de session, suivi d'un passage en `State:A`. |
| **L** | Libération / Verrouillage | Apparaît ponctuellement. Force le reset du flag de charge terminée. |
| **Y** | Veille Profonde | La borne est en économie d'énergie. |
| **F, G, H, R, S, T, U, V** | Erreur Matérielle | Défauts divers (température, fuite DC, défaut terre, etc.) signalés par les capteurs internes de la carte. |

---

## 2. Incohérences et Bugs Matériels Découverts (Quirks)

Lors de l'intégration logicielle et de l'analyse des logs, plusieurs comportements anormaux du firmware ont été documentés. Ces comportements devront être **corrigés/masqués logiciellement par la couche `greenup-driver`** avant d'être envoyés à EVerest ou à un serveur OCPP.

### Quirk #1 : `State:A` avec un câble branché
Lors d'un arrêt de charge déclenché par la borne, la séquence finale est `W` -> `M` -> `A`.
La borne se déclare en état `A` (Available / Libre) **même si le câble Type 2 est toujours physiquement inséré et verrouillé** côté voiture.
*Impact :* Il ne faut pas déduire la présence physique du câble à partir de l'état `A`. `A` signifie "Transaction terminée/disponible", pas "Prise débranchée".

### Quirk #2 : Passage forcé en `State:C` par les Heures Creuses
Lors d'un démarrage retardé (la voiture est branchée mais attend son propre planning), la borne est en `State:B`. 
Si un évènement TIC (ex: passage en Heures Creuses) survient, l'ATmega passe arbitrairement en `State:C` et génère le PWM, **même si la voiture (broche CP) n'a rien réclamé**. 
*Impact :* L'ATmega prend l'initiative d'offrir le courant sans vérifier si la voiture est en mode "prête à charger" (State C IEC 61851). C'est seulement plus tard (au déclenchement du planning de la voiture) que le courant circulera vraiment, en passant au `State:E`.

### Quirk #3 : Mauvais diagnostic OCPP (`OCPPStatus:SuspendedEVSE`)
La borne dispose d'un module OCPP interne basique. Si l'on demande son statut via la commande `OCPPPS?` :
* Lors d'une charge arrêtée par la **borne** : elle renvoie logiquement `SuspendedEVSE`.
* Lors d'une charge arrêtée par la **voiture** (Passage par `State:I` puis `State:B`) : elle renvoie **aussi** `SuspendedEVSE`.
*Impact :* C'est faux d'un point de vue OCPP, elle devrait renvoyer `SuspendedEV`. Le driver Rust devra inférer le bon statut OCPP en mémorisant si la charge s'est terminée par un passage en `State:I`.
