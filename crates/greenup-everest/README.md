# Module EVerest - GreenUp Driver (`greenup-everest`)

Ce module a pour but d'exposer la borne Legrand Green'Up Premium comme un module `evse_board_support` (BSP) et `powermeter` standardisés pour le framework EVerest.

## Objectif

EVerest s'attend à piloter un contrôleur de charge "muet" (envoyer des impulsions PWM, fermer le contacteur, lire la tension CP). Or, la borne Legrand embarque sa propre machine à états (elle gère elle-même la sécurité, le contacteur et les heures creuses).

La crate `greenup-everest` joue donc le rôle de **Façade / Adaptateur** :
1. **Événements (BSP Events)** : Traduit l'état logique déduit (`Disconnected_A`, `Charging_C`...) en événements EVerest reconnus.
2. **Commandes (Allow Power On)** : Mappe la commande EVerest d'autorisation de charge (`allow_power_on`) vers des appuis logiciels sur les boutons START/STOP (`SBOK` / `SBNOK`), qui mettent proprement en pause la machine Legrand.
3. **Limite de courant (Set PWM)** : Convertit le rapport cyclique demandé par le gestionnaire d'énergie d'EVerest (`duty_cycle_pct`) en Ampères matériels pour la borne (`CC:XX`).
4. **Télémétrie (Powermeter)** : Publie la puissance, la tension et l'énergie sous un format digeste pour EVerest.

## Intégration Finale

Actuellement, ce crate est prêt à être interfacé soit :
- Avec **`everest-rs`** si l'environnement de build (CMake, `ev-cli`) le permet.
- Via un pont **MQTT** (le plus universel) : on pourra créer un binaire qui instancie l'`EverestAdapter` et publie/souscrit aux topics MQTT EVerest (`everest/power_meter/...`, `everest/board_support/...`).

## Architecture de l'Adaptateur

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
