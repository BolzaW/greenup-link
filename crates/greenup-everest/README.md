# Module EVerest - GreenUp Driver (`greenup-everest`)

Ce module a pour but d'exposer la borne Legrand Green'Up Premium comme un module `evse_board_support` (BSP) et `powermeter` standardisés pour le framework EVerest.

## Objectif

EVerest s'attend à piloter un contrôleur de charge "muet" (envoyer des impulsions PWM, fermer le contacteur, lire la tension CP). Or, la borne Legrand embarque sa propre machine à états (elle gère elle-même la sécurité, le contacteur et les heures creuses).

La crate `greenup-everest` joue donc le rôle de **Façade / Adaptateur** :
1. **Événements (BSP Events)** : Traduit l'état logique déduit (`Disconnected_A`, `Charging_C`...) en événements EVerest reconnus.
2. **Commandes (Allow Power On)** : Mappe la commande EVerest d'autorisation de charge (`allow_power_on`) vers des appuis logiciels sur les boutons START/STOP (`SBOK` / `SBNOK`), qui mettent proprement en pause la machine Legrand.
3. **Limite de courant (Set PWM)** : Convertit le rapport cyclique demandé par le gestionnaire d'énergie d'EVerest (`duty_cycle_pct`) en Ampères matériels pour la borne (`CC:XX`).
4. **Télémétrie (Powermeter)** : Publie la puissance, la tension et l'énergie sous un format digeste pour EVerest.

## Intégration Finale (Pont MQTT)

Actuellement, ce crate fonctionne comme un **exécutable autonome (Daemon MQTT)**. Il se connecte à un broker MQTT local et agit comme un pont de traduction entre le port série Legrand et le monde extérieur.

### API MQTT (Topics standards)

Pour intégrer cette borne dans EVerest, utilisez les modules génériques MQTT d'EVerest et configurez-les pour écouter/publier sur les topics suivants :

#### Émission (Legrand ➔ EVerest)
*   **Topic:** `everest/board_support/event`
    *   **Payload (Texte) :** `A`, `B`, `C`, `Error`, `Faulted`
    *   **Description :** État de la machine IEC 61851 (câble branché, charge en cours, etc.).
*   **Topic:** `everest/powermeter/telemetry`
    *   **Payload (JSON) :** `{"voltage_V": 230.0, "current_A": 16.0, "power_W": 3680.0, "energy_Wh": 15000.0}`
    *   **Description :** Remontée des compteurs d'énergie et puissances instantanées.

#### Réception (EVerest ➔ Legrand)
*   **Topic:** `everest/board_support/cmd/allow_power_on`
    *   **Payload (Texte) :** `true` ou `false`
    *   **Description :** Autorise ou suspend la charge (simule un appui sur Start/Stop).
*   **Topic:** `everest/board_support/cmd/set_pwm`
    *   **Payload (Texte) :** Rapport cyclique en pourcentage (ex: `26.6` pour 16A).
    *   **Description :** Modifie la consigne de courant. La borne Legrand ne gère pas le vrai PWM, ce pourcentage est converti en Ampères (`Amps = PWM * 0.6`) et envoyé à la carte. Limite bridée entre 7A et 32A.
*   **Topic:** `everest/board_support/cmd/reset`
    *   **Payload (Texte) :** N'importe quelle valeur (ex: `1`).
    *   **Description :** Déclenche un redémarrage matériel (Reset ATmega).

### Configuration EVerest

Dans votre fichier `config.json` d'EVerest, vous devrez configurer les modules de type "Generic MQTT" pour faire correspondre les entrées/sorties avec ces topics.

```rust
pub struct EverestAdapter {
    // Écoute des événements traduits pour EVerest
    pub fn subscribe_events(&self) -> broadcast::Receiver<EverestBspEvent>;
    pub fn subscribe_telemetry(&self) -> broadcast::Receiver<EverestTelemetry>;

    // Commandes traduites depuis EVerest vers Legrand
    pub async fn allow_power_on(&self, allow: bool) -> Result<(), String>;
    pub async fn set_pwm(&self, duty_cycle_pct: f32) -> Result<(), String>;
}
```
