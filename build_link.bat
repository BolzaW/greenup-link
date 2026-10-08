@echo off
echo ====================================================
echo   Compilation Rust (Zigbuild) pour la borne Legrand
echo ====================================================
echo.

REM Utilisation de cargo-zigbuild pour compiler le binaire greenup-link
wsl -d Ubuntu -e bash -c "PATH=$PATH:~/.cargo/bin cargo zigbuild -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24"

echo.
echo ====================================================
echo Compilation terminee !
echo Le fichier a envoyer sur le Raspberry se trouve ici :
echo target\armv7-unknown-linux-gnueabihf\debug\greenup-link
echo ====================================================
pause

