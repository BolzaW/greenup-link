# ==============================================================================
# Green'Up Link - Interactive Release & Version Bump Script
# ==============================================================================
# Usage: .\scripts\release.ps1
# ==============================================================================

$ErrorActionPreference = "Stop"

# Move to repository root
$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Write-Header($Title) {
    Write-Host ""
    Write-Host "==========================================================" -ForegroundColor Cyan
    Write-Host "  $Title" -ForegroundColor Cyan
    Write-Host "==========================================================" -ForegroundColor Cyan
    Write-Host ""
}

function Get-CurrentCargoVersion {
    $cargoToml = Get-Content -Path (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    throw "Impossible de lire la version dans Cargo.toml"
}

function Set-ProjectVersion([string]$NewVersion) {
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)

    # 1. Update root Cargo.toml
    $cargoTomlPath = Join-Path $RepoRoot "Cargo.toml"
    $tomlContent = [System.IO.File]::ReadAllText($cargoTomlPath)
    $tomlContent = [regex]::Replace($tomlContent, '(?m)^version\s*=\s*"[^"]+"', "version = `"$NewVersion`"")
    [System.IO.File]::WriteAllText($cargoTomlPath, $tomlContent, $utf8NoBom)

    # 2. Update workspace packages in Cargo.lock
    $cargoLockPath = Join-Path $RepoRoot "Cargo.lock"
    if (Test-Path $cargoLockPath) {
        $lockContent = [System.IO.File]::ReadAllText($cargoLockPath)
        $lockContent = [regex]::Replace(
            $lockContent,
            '(?m)(name\s*=\s*"greenup-(?:driver|everest|link|protocol)"\r?\nversion\s*=\s*)"[^"]+"',
            "`${1}`"$NewVersion`""
        )
        [System.IO.File]::WriteAllText($cargoLockPath, $lockContent, $utf8NoBom)
    }
}

Write-Header "Green'Up Link - Assistant de Release GitHub"

# Check current branch and working tree status
$CurrentBranch = (git rev-parse --abbrev-ref HEAD).Trim()
$CurrentVersion = Get-CurrentCargoVersion
$DefaultReleaseVersion = $CurrentVersion -replace '-dev.*$', ''

Write-Host "Branche actuelle : " -NoNewline
Write-Host $CurrentBranch -ForegroundColor Yellow
Write-Host "Version actuelle : " -NoNewline
Write-Host $CurrentVersion -ForegroundColor Yellow
Write-Host ""

$DirtyFiles = git status --porcelain
if ($DirtyFiles) {
    Write-Host "⚠️  Attention : Le dépôt contient des modifications non commitées :" -ForegroundColor Yellow
    git status --short
    Write-Host ""
    $ContinueDirty = Read-Host "Voulez-vous continuer malgré tout ? [o/N]"
    if ($ContinueDirty -notmatch '^[oOyY]$') {
        Write-Host "Abandon. Veuillez commiter ou remiser (stash) vos modifications d'abord." -ForegroundColor Red
        exit 1
    }
    Write-Host ""
}

# ------------------------------------------------------------------------------
# 1) Ask which version to release
# ------------------------------------------------------------------------------
$InputVersion = Read-Host "1) Quelle version souhaitez-vous releaser ? (Entrée pour '$DefaultReleaseVersion')"
if ([string]::IsNullOrWhiteSpace($InputVersion)) {
    $ReleaseVersion = $DefaultReleaseVersion
} else {
    # Strip leading 'v' if user typed 'v0.3.0' instead of '0.3.0'
    $ReleaseVersion = $InputVersion.Trim() -replace '^[vV]', ''
}

if ($ReleaseVersion -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
    Write-Host "❌ Erreur : Format de version invalide '$ReleaseVersion'. Attendu : X.Y.Z (ex: 0.3.0)" -ForegroundColor Red
    exit 1
}

$Major = [int]$Matches[1]
$Minor = [int]$Matches[2]
$Patch = [int]$Matches[3]

# ------------------------------------------------------------------------------
# 2) Confirm tag name
# ------------------------------------------------------------------------------
$DefaultTag = "v$ReleaseVersion"
$ConfirmTag = Read-Host "2) Confirmez-vous le nom du tag '$DefaultTag' ? ([O/n] ou tapez un autre nom de tag)"

if ([string]::IsNullOrWhiteSpace($ConfirmTag) -or $ConfirmTag -match '^[oOyY]$') {
    $TagName = $DefaultTag
} elseif ($ConfirmTag -match '^[nN]$') {
    $CustomTag = Read-Host "   Entrez le nom du tag souhaité (ex: v$ReleaseVersion)"
    if ([string]::IsNullOrWhiteSpace($CustomTag)) {
        Write-Host "❌ Abandon : Aucun nom de tag fourni." -ForegroundColor Red
        exit 1
    }
    $TagName = $CustomTag.Trim()
} else {
    $TagName = $ConfirmTag.Trim()
}

# Check if tag already exists locally
$ExistingTag = git tag -l $TagName
if ($ExistingTag) {
    Write-Host "❌ Erreur : Le tag '$TagName' existe déjà localement !" -ForegroundColor Red
    Write-Host "   Supprimez-le avec 'git tag -d $TagName' si vous souhaitez le recréer." -ForegroundColor Yellow
    exit 1
}

# ------------------------------------------------------------------------------
# 3 & 4) Update project version to release version & create Git tag
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "3) Mise à jour de la version du projet vers '$ReleaseVersion'..." -ForegroundColor Cyan
Set-ProjectVersion -NewVersion $ReleaseVersion

$ReleaseCommitCreated = $false
$VersionDiff = git status --porcelain Cargo.toml Cargo.lock
if ($VersionDiff) {
    git add Cargo.toml Cargo.lock
    git commit -m "chore(release): prepare $TagName"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Erreur lors du commit de la version de release." -ForegroundColor Red
        exit 1
    }
    $ReleaseCommitCreated = $true
    Write-Host "   -> Commit de release créé (version $ReleaseVersion)." -ForegroundColor Green
} else {
    Write-Host "   -> Cargo.toml est déjà en version $ReleaseVersion (aucun commit nécessaire)." -ForegroundColor Green
}

Write-Host "4) Pose du tag Git '$TagName'..." -ForegroundColor Cyan
git tag -a $TagName -m "Release $TagName"
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Erreur lors de la création du tag '$TagName'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Tag '$TagName' posé localement sur $(git rev-parse --short HEAD)." -ForegroundColor Green

# Compute next development version (increment 2nd digit / minor + -dev)
$NextMinor = $Minor + 1
$NextDevVersion = "$Major.$NextMinor.0-dev"

# ------------------------------------------------------------------------------
# 5) Ask confirmation to publish release on GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "----------------------------------------------------------" -ForegroundColor DarkGray
Write-Host "  Récapitulatif avant publication sur GitHub :" -ForegroundColor White
Write-Host "  • Version de la release : $ReleaseVersion" -ForegroundColor Green
Write-Host "  • Tag Git               : $TagName" -ForegroundColor Green
Write-Host "  • Branche               : $CurrentBranch" -ForegroundColor Yellow
Write-Host "  • Version suivante dev  : $NextDevVersion" -ForegroundColor Cyan
Write-Host "----------------------------------------------------------" -ForegroundColor DarkGray
Write-Host ""

$ConfirmPush = Read-Host "5) Confirmez-vous la création de la release sur GitHub (push du commit et du tag '$TagName') ? [o/N]"
if ($ConfirmPush -notmatch '^[oOyY]$') {
    Write-Host ""
    Write-Host "⏪ Annulation demandée : nettoyage du tag local..." -ForegroundColor Yellow
    git tag -d $TagName | Out-Null
    if ($ReleaseCommitCreated) {
        git reset --hard HEAD~1 | Out-Null
        Write-Host "   -> Commit de release annulé (retour à $CurrentVersion)." -ForegroundColor Yellow
    }
    Write-Host "Abandonné proprement. Aucune modification n'a été poussée sur GitHub." -ForegroundColor Red
    exit 0
}

# ------------------------------------------------------------------------------
# 6) Push release version and tag to GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "6) Push de la branche '$CurrentBranch' et du tag '$TagName' vers GitHub..." -ForegroundColor Cyan
git push origin $CurrentBranch
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Erreur lors du push de la branche '$CurrentBranch'." -ForegroundColor Red
    exit 1
}

git push origin $TagName
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Erreur lors du push du tag '$TagName'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Tag '$TagName' poussé ! Le workflow GitHub Actions est lancé." -ForegroundColor Green

# ------------------------------------------------------------------------------
# 7) Increment minor version (2nd digit) with -dev suffix and commit
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "7) Passage du projet à la prochaine version de développement : '$NextDevVersion'..." -ForegroundColor Cyan
Set-ProjectVersion -NewVersion $NextDevVersion

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NextDevVersion"
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Erreur lors du commit de la version '$NextDevVersion'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Commit créé pour '$NextDevVersion'." -ForegroundColor Green

# ------------------------------------------------------------------------------
# 8) Push new -dev version to GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "8) Push de la nouvelle version '$NextDevVersion' sur GitHub..." -ForegroundColor Cyan
git push origin $CurrentBranch
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Erreur lors du push de la version '$NextDevVersion'." -ForegroundColor Red
    exit 1
}

Write-Header "🎉 Opération terminée avec succès !"
Write-Host "  • Release publiée   : $TagName (v$ReleaseVersion)" -ForegroundColor Green
Write-Host "  • Branche $CurrentBranch     : $NextDevVersion" -ForegroundColor Cyan
Write-Host "  • Suivi du build CI : https://github.com/BolzaW/greenup-link/actions" -ForegroundColor Yellow
Write-Host ""
