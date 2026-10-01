@echo off
echo ====================================================
echo   Compilation Rust (Zigbuild) pour la borne Legrand
echo ====================================================
echo.

REM Utilisation de cargo-zigbuild. 
REM On force PATH=/usr/local/bin:$PATH pour que cargo-zigbuild trouve bien le zig Linux et non une commande Windows.
wsl -d Ubuntu -e bash -c "cd '/mnt/c/Users/rapha/Desktop/Maj borne legrand/retro_ingenierie/legrand/greenup-link' && PATH=/usr/local/bin:$PATH ~/.cargo/bin/cargo zigbuild --target armv7-unknown-linux-gnueabihf.2.24"

echo.
echo ====================================================
echo Compilation terminee !
echo Le fichier a envoyer sur le Raspberry se trouve ici :
echo target\armv7-unknown-linux-gnueabihf\debug\greenup-link
echo ====================================================
pause
