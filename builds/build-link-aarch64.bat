@echo off
cd /d "%~dp0\.."
echo ====================================================
echo   Compilation Rust (aarch64 64-bit) pour Raspberry Pi
echo ====================================================
echo.

REM Compilation classique avec Cargo vers la cible aarch64-unknown-linux-gnu
wsl -d Ubuntu -e bash -c "PATH=$PATH:~/.cargo/bin cargo build -p greenup-link --target aarch64-unknown-linux-gnu"

echo.
echo ====================================================
echo Compilation terminee !
echo Le fichier a envoyer sur le Raspberry se trouve ici :
echo target\aarch64-unknown-linux-gnu\debug\greenup-link
echo ====================================================
pause
