@echo off
setlocal

echo ====================================================
echo   Generation de la Release Green'Up Link v0.0.1
echo ====================================================
echo.

REM 1. Nettoyage de l'ancien dossier release s'il existe
if exist "release_pkg" rmdir /S /Q "release_pkg"
mkdir "release_pkg"

REM 2. Compilation en mode RELEASE via WSL
echo [1/3] Compilation en cours (WSL - Cargo Zigbuild Release)...
wsl -d Ubuntu -e bash -c "cd '/mnt/c/Users/rapha/Desktop/Maj borne legrand/retro_ingenierie/legrand/greenup-link' && PATH=/usr/local/bin:$PATH ~/.cargo/bin/cargo zigbuild --release --target armv7-unknown-linux-gnueabihf.2.24"

if %errorlevel% neq 0 (
    echo.
    echo [ERREUR] La compilation a echoue.
    pause
    exit /b %errorlevel%
)

REM 3. Copie des fichiers dans le dossier de release
echo [2/3] Preparation des fichiers...
copy "target\armv7-unknown-linux-gnueabihf\release\greenup-link" "release_pkg\greenup-link" >nul
mkdir "release_pkg\scripts"
copy "scripts\start_legrand.sh" "release_pkg\scripts\" >nul
copy "scripts\stop_legrand.sh" "release_pkg\scripts\" >nul
copy "cli-greenup-link.ps1" "release_pkg\" >nul
copy "README.md" "release_pkg\" >nul

REM 4. Creation de l'archive ZIP via PowerShell
echo [3/3] Creation de l'archive ZIP (greenup-link-v0.0.1.zip)...
if exist "greenup-link-v0.0.1.zip" del "greenup-link-v0.0.1.zip"
powershell -Command "Compress-Archive -Path 'release_pkg\*' -DestinationPath 'greenup-link-v0.0.1.zip'"

REM 5. Nettoyage
rmdir /S /Q "release_pkg"

echo.
echo ====================================================
echo SUCCESS ! Release terminee avec succes.
echo L'archive greenup-link-v0.0.1.zip est prete a etre 
echo publiee sur la page Releases de GitHub !
echo ====================================================
pause
