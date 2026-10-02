@echo off
setlocal

echo ====================================================
echo   Generation de la Release Green'Up Link v0.1.0
echo ====================================================
echo.

REM 1. Nettoyage de l'ancien dossier release s'il existe
if exist "release_pkg" rmdir /S /Q "release_pkg"
mkdir "release_pkg"

REM 2. Compilation en mode RELEASE via WSL
echo [1/3] Compilation en cours (WSL - Cargo Zigbuild Release)...
wsl -d Ubuntu -e bash -c "PATH=/usr/local/bin:$PATH ~/.cargo/bin/cargo zigbuild --release --target armv7-unknown-linux-gnueabihf.2.24"

if %errorlevel% neq 0 (
    echo.
    echo [ERREUR] La compilation a echoue.
    pause
    exit /b %errorlevel%
)

REM 3. Copie des fichiers dans le dossier de release
echo [2/3] Preparation des fichiers...
copy "target\armv7-unknown-linux-gnueabihf\release\greenup-link-standalone" "release_pkg\greenup-link-standalone" >nul
mkdir "release_pkg\scripts"
copy "scripts\start_legrand.sh" "release_pkg\scripts\" >nul
copy "scripts\stop_legrand.sh" "release_pkg\scripts\" >nul
copy "cli-greenup-link.ps1" "release_pkg\" >nul
copy "README.md" "release_pkg\" >nul

REM 4. Creation de l'archive ZIP via l'utilitaire natif tar (Windows 10+)
echo [3/3] Creation de l'archive ZIP (greenup-link-standalone-v0.1.0.zip)...
if exist "greenup-link-standalone-v0.1.0.zip" del "greenup-link-standalone-v0.1.0.zip"
cd release_pkg
tar -a -c -f ..\greenup-link-standalone-v0.1.0.zip *
cd ..

REM 5. Nettoyage
rmdir /S /Q "release_pkg"

echo.
echo ====================================================
echo SUCCESS ! Release terminee avec succes.
echo L'archive greenup-link-standalone-v0.1.0.zip est prete !
echo ====================================================
pause
