@echo off
setlocal enabledelayedexpansion

REM 1. Extraction dynamique de la version depuis Cargo.toml
for /f "tokens=3 delims= " %%a in ('findstr /r "^version" Cargo.toml') do set RAW_VERSION=%%~a
set VERSION=%RAW_VERSION:"=%
if "%VERSION%"=="" set VERSION=0.2.0

echo ====================================================
echo   Generation de la Release Green'Up Link v%VERSION%
echo ====================================================
echo.

REM 2. Compilation en mode RELEASE via WSL (cargo-zigbuild)
echo [1/3] Compilation en cours (WSL - Cargo Zigbuild Release)...
wsl -d Ubuntu -e bash -c "PATH=$PATH:~/.cargo/bin cargo zigbuild --release -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24"

if %errorlevel% neq 0 (
    echo.
    echo [ERREUR] La compilation a echoue.
    pause
    exit /b %errorlevel%
)

set BINARY_PATH=target\armv7-unknown-linux-gnueabihf\release\greenup-link

if not exist "%BINARY_PATH%" (
    echo.
    echo [ERREUR] Binaire introuvable : %BINARY_PATH%
    pause
    exit /b 1
)

REM 3. Creation de l'archive ZIP optionnelle (greenup-link-v%VERSION%.zip)
echo [2/3] Creation de l'archive (greenup-link-v%VERSION%.zip)...
if exist "release_pkg" rmdir /S /Q "release_pkg"
mkdir "release_pkg"
copy "%BINARY_PATH%" "release_pkg\greenup-link" >nul
if exist "scripts\start_legrand.sh" (
    mkdir "release_pkg\scripts" 2>nul
    copy "scripts\start_legrand.sh" "release_pkg\scripts\" >nul
    copy "scripts\stop_legrand.sh" "release_pkg\scripts\" >nul
)
if exist "cli-greenup-link.ps1" copy "cli-greenup-link.ps1" "release_pkg\" >nul
if exist "README.md" copy "README.md" "release_pkg\" >nul

if exist "greenup-link-v%VERSION%.zip" del "greenup-link-v%VERSION%.zip"
cd release_pkg
tar -a -c -f "..\greenup-link-v%VERSION%.zip" *
cd ..
rmdir /S /Q "release_pkg"

echo [3/3] Finalisation...
echo.
echo ====================================================
echo SUCCESS ! Release v%VERSION% terminee avec succes.
echo.
echo [1] Binaire direct (a uploader sur GitHub Releases) :
echo     %BINARY_PATH%
echo.
echo [2] Archive ZIP optionnelle :
echo     greenup-link-v%VERSION%.zip
echo ====================================================
pause

