@echo off
echo ====================================================
echo   Compilation Rust (Zigbuild) pour la borne Legrand
echo ====================================================
echo.

REM Utilisation de cargo-zigbuild pour compiler tous les binaires du workspace
wsl -d Ubuntu -e bash -c "PATH=/usr/local/bin: ~/.cargo/bin/cargo zigbuild --target armv7-unknown-linux-gnueabihf.2.24"

echo.
echo ====================================================
echo Compilation terminee !
echo Le fichier a envoyer sur le Raspberry se trouve ici :
echo target\armv7-unknown-linux-gnueabihf\debug\greenup-link
echo ====================================================
pause

