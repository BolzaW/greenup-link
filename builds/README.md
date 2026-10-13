# Scripts de Build & Release

Ce répertoire regroupe l'ensemble des scripts de compilation et d'empaquetage pour Raspberry Pi.

Tous les scripts peuvent être exécutés directement par double-clic depuis Windows ou via le terminal : ils se repositionnent automatiquement à la racine du projet (`cd /d "%~dp0\.."`).

---

## 🚀 Cible AArch64 (Raspberry Pi OS 64-bit - Actuel)
*Utilise le compilateur croisé classique `cargo build` avec `gcc-aarch64-linux-gnu` (sans zigbuild).*

- **`build-link-aarch64.bat`** : Compile `greenup-link` en mode Debug vers `target/aarch64-unknown-linux-gnu/debug/greenup-link`.
- **`build-everest-aarch64.bat`** : Compile `greenup-everest` en mode Debug vers `target/aarch64-unknown-linux-gnu/debug/greenup-everest`.
- **`release-aarch64.bat`** : Compile `greenup-link` en mode Release (optimisé, allégé) et crée l'archive `greenup-link-vX.Y.Z-aarch64.zip`.

---

## 🏛️ Cible Legacy (ARM 32-bit - Ancien OS Legrand)
*Utilise `cargo-zigbuild` ciblant la glibc 2.24 pour compatibilité avec l'image Debian historique de la borne Legrand.*

- **`build-link-legacy.bat`** : Compile `greenup-link` en mode Debug vers `target/armv7-unknown-linux-gnueabihf/debug/greenup-link`.
- **`build-everest-legacy.bat`** : Compile `greenup-everest` en mode Debug vers `target/armv7-unknown-linux-gnueabihf/debug/greenup-everest`.
- **`release-legacy.bat`** : Compile `greenup-link` en mode Release et crée l'archive `greenup-link-vX.Y.Z-armv7-legacy.zip`.

---

Pour plus de détails sur la configuration de l'environnement, consultez [`docs/ENV_COMPILATION_AARCH64.md`](../docs/ENV_COMPILATION_AARCH64.md).
