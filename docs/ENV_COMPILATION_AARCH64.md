# Guide d'environnement : Cross-compilation 64-bit (AArch64)

Ce guide résume les étapes nécessaires pour configurer votre environnement WSL (Ubuntu) afin de compiler les binaires `greenup-link` et `greenup-everest` en mode natif 64-bit (`aarch64-unknown-linux-gnu`) pour Raspberry Pi OS 64-bit (Lite ou Desktop).

---

## 1. Prérequis
- Windows 10/11 avec WSL 2 (distribution Ubuntu).
- Rust et Cargo installés dans l'environnement WSL (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

---

## 2. Installation de la cible Rust 64-bit

Dans le terminal WSL (ou via `wsl -d Ubuntu`) :

```bash
rustup target add aarch64-unknown-linux-gnu
```

Cette commande télécharge la bibliothèque standard Rust (`rust-std`) compilée pour les processeurs ARM 64-bit (Cortex-A53, A72, A76 utilisés sur Raspberry Pi 3, 4, 5 et Zero 2 W).

---

## 3. Installation du linker croisé GCC AArch64

Contrairement à la compilation hôte (x86_64), Cargo a besoin d'un éditeur de liens (linker) capable d'assembler des binaires ARM64 depuis un hôte x86_64.

Installez la chaîne d'outils croisée officielle :

```bash
sudo apt-get update
sudo apt-get install -y gcc-aarch64-linux-gnu
```

Le binaire `aarch64-linux-gnu-gcc` est désormais disponible dans votre `PATH`.

---

## 4. Configuration de Cargo (`.cargo/config.toml`)

Pour éviter de devoir spécifier manuellement le linker à chaque commande, le fichier [`.cargo/config.toml`](../.cargo/config.toml) a été placé à la racine du projet :

```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
```

Grâce à cette directive, Cargo sait automatiquement quel linker appeler lors d'un build `--target aarch64-unknown-linux-gnu`.

---

## 5. Utilisation des scripts de compilation

Tous les scripts de build sont centralisés dans le dossier [`builds/`](../builds/) :

| Script | Cible | Type de build | Destination du binaire |
| :--- | :--- | :--- | :--- |
| **`build-link-aarch64.bat`** | 64-bit (`aarch64`) | Debug | `target/aarch64-unknown-linux-gnu/debug/greenup-link` |
| **`build-everest-aarch64.bat`** | 64-bit (`aarch64`) | Debug | `target/aarch64-unknown-linux-gnu/debug/greenup-everest` |
| **`release-aarch64.bat`** | 64-bit (`aarch64`) | Release + ZIP | `target/aarch64-unknown-linux-gnu/release/greenup-link` + `greenup-link-vX.Y.Z-aarch64.zip` |
| **`build-link-legacy.bat`** | 32-bit (`armv7`) | Zigbuild Debug (glibc 2.24) | `target/armv7-unknown-linux-gnueabihf/debug/greenup-link` |
| **`build-everest-legacy.bat`** | 32-bit (`armv7`) | Zigbuild Debug (glibc 2.24) | `target/armv7-unknown-linux-gnueabihf/debug/greenup-everest` |
| **`release-legacy.bat`** | 32-bit (`armv7`) | Zigbuild Release (glibc 2.24) | `target/armv7-unknown-linux-gnueabihf/release/greenup-link` + `greenup-link-vX.Y.Z-armv7-legacy.zip` |

---

## 6. Déploiement rapide sur le Raspberry Pi

Depuis PowerShell :

```powershell
# Envoi de la version release
scp target/aarch64-unknown-linux-gnu/release/greenup-link pi@greenup-host:~/

# Ou envoi de la version debug
scp target/aarch64-unknown-linux-gnu/debug/greenup-link pi@greenup-host:~/
```

Sur le Raspberry Pi :

```bash
chmod +x ./greenup-link
./greenup-link
```
