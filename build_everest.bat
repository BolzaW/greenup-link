@echo off
echo ====================================================
echo   Compilation EVerest (Zigbuild) pour Raspberry Pi
echo ====================================================
echo.

REM Utilisation de cargo-zigbuild pour compiler specifiquement le binaire greenup-everest en mode debug
wsl -d Ubuntu -e bash -c "PATH=$PATH:~/.cargo/bin cargo zigbuild -p greenup-everest --target armv7-unknown-linux-gnueabihf.2.24"

echo.
echo ====================================================
echo Compilation EVerest terminee !
echo Le fichier a envoyer sur le Raspberry se trouve ici :
echo target\armv7-unknown-linux-gnueabihf\debug\greenup-everest
echo ====================================================
pause
