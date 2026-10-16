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
    throw "Unable to read version from Cargo.toml"
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

Write-Header "Green'Up Link - GitHub Release Assistant"

# Check current branch and working tree status
$CurrentBranch = (git rev-parse --abbrev-ref HEAD).Trim()
$CurrentVersion = Get-CurrentCargoVersion
$DefaultReleaseVersion = $CurrentVersion -replace '-dev.*$', ''

Write-Host "Current branch : " -NoNewline
Write-Host $CurrentBranch -ForegroundColor Yellow
Write-Host "Current version: " -NoNewline
Write-Host $CurrentVersion -ForegroundColor Yellow
Write-Host ""

$DirtyFiles = git status --porcelain
if ($DirtyFiles) {
    Write-Host "⚠️  Warning: Working tree contains uncommitted changes:" -ForegroundColor Yellow
    git status --short
    Write-Host ""
    $ContinueDirty = Read-Host "Do you want to continue anyway? [y/N]"
    if ($ContinueDirty -notmatch '^[yY]$') {
        Write-Host "Aborted. Please commit or stash your changes first." -ForegroundColor Red
        exit 1
    }
    Write-Host ""
}

# ------------------------------------------------------------------------------
# 1) Ask which version to release
# ------------------------------------------------------------------------------
$InputVersion = Read-Host "1) Which version do you want to release? (Press Enter for '$DefaultReleaseVersion')"
if ([string]::IsNullOrWhiteSpace($InputVersion)) {
    $ReleaseVersion = $DefaultReleaseVersion
} else {
    # Strip leading 'v' if user typed 'v0.3.0' instead of '0.3.0'
    $ReleaseVersion = $InputVersion.Trim() -replace '^[vV]', ''
}

if ($ReleaseVersion -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
    Write-Host "❌ Error: Invalid version format '$ReleaseVersion'. Expected: X.Y.Z (e.g. 0.3.0)" -ForegroundColor Red
    exit 1
}

$Major = [int]$Matches[1]
$Minor = [int]$Matches[2]
$Patch = [int]$Matches[3]

# ------------------------------------------------------------------------------
# 2) Confirm tag name
# ------------------------------------------------------------------------------
$DefaultTag = "v$ReleaseVersion"
$ConfirmTag = Read-Host "2) Confirm tag name '$DefaultTag'? ([Y/n] or enter a custom tag)"

if ([string]::IsNullOrWhiteSpace($ConfirmTag) -or $ConfirmTag -match '^[yY]$') {
    $TagName = $DefaultTag
} elseif ($ConfirmTag -match '^[nN]$') {
    $CustomTag = Read-Host "   Enter desired tag name (e.g. v$ReleaseVersion)"
    if ([string]::IsNullOrWhiteSpace($CustomTag)) {
        Write-Host "❌ Aborted: No tag name provided." -ForegroundColor Red
        exit 1
    }
    $TagName = $CustomTag.Trim()
} else {
    $TagName = $ConfirmTag.Trim()
}

# Check if tag already exists locally
$ExistingTag = git tag -l $TagName
if ($ExistingTag) {
    Write-Host "❌ Error: Tag '$TagName' already exists locally!" -ForegroundColor Red
    Write-Host "   Delete it with 'git tag -d $TagName' if you want to recreate it." -ForegroundColor Yellow
    exit 1
}

# ------------------------------------------------------------------------------
# 3 & 4) Update project version to release version & create Git tag
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "3) Updating project version to '$ReleaseVersion'..." -ForegroundColor Cyan
Set-ProjectVersion -NewVersion $ReleaseVersion

$ReleaseCommitCreated = $false
$VersionDiff = git status --porcelain Cargo.toml Cargo.lock
if ($VersionDiff) {
    git add Cargo.toml Cargo.lock
    git commit -m "chore(release): prepare $TagName"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Error while committing release version." -ForegroundColor Red
        exit 1
    }
    $ReleaseCommitCreated = $true
    Write-Host "   -> Release commit created (version $ReleaseVersion)." -ForegroundColor Green
} else {
    Write-Host "   -> Cargo.toml is already at version $ReleaseVersion (no commit needed)." -ForegroundColor Green
}

Write-Host "4) Creating Git tag '$TagName'..." -ForegroundColor Cyan
git tag -a $TagName -m "Release $TagName"
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error while creating tag '$TagName'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Tag '$TagName' created locally on $(git rev-parse --short HEAD)." -ForegroundColor Green

# Compute next development version (increment 2nd digit / minor + -dev)
$NextMinor = $Minor + 1
$NextDevVersion = "$Major.$NextMinor.0-dev"

# ------------------------------------------------------------------------------
# 5) Ask confirmation to publish release on GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "----------------------------------------------------------" -ForegroundColor DarkGray
Write-Host "  Summary before publishing to GitHub:" -ForegroundColor White
Write-Host "  • Release version     : $ReleaseVersion" -ForegroundColor Green
Write-Host "  • Git tag             : $TagName" -ForegroundColor Green
Write-Host "  • Branch              : $CurrentBranch" -ForegroundColor Yellow
Write-Host "  • Next dev version    : $NextDevVersion" -ForegroundColor Cyan
Write-Host "----------------------------------------------------------" -ForegroundColor DarkGray
Write-Host ""

$ConfirmPush = Read-Host "5) Confirm GitHub release publication (push commit and tag '$TagName')? [y/N]"
if ($ConfirmPush -notmatch '^[yY]$') {
    Write-Host ""
    Write-Host "⏪ Cancellation requested: cleaning up local tag..." -ForegroundColor Yellow
    git tag -d $TagName | Out-Null
    if ($ReleaseCommitCreated) {
        git reset --hard HEAD~1 | Out-Null
        Write-Host "   -> Release commit reverted (back to $CurrentVersion)." -ForegroundColor Yellow
    }
    Write-Host "Aborted cleanly. No changes were pushed to GitHub." -ForegroundColor Red
    exit 0
}

# ------------------------------------------------------------------------------
# 6) Push release version and tag to GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "6) Pushing branch '$CurrentBranch' and tag '$TagName' to GitHub..." -ForegroundColor Cyan
git push origin $CurrentBranch
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error while pushing branch '$CurrentBranch'." -ForegroundColor Red
    exit 1
}

git push origin $TagName
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error while pushing tag '$TagName'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Tag '$TagName' pushed! GitHub Actions workflow has been triggered." -ForegroundColor Green

# ------------------------------------------------------------------------------
# 7) Increment minor version (2nd digit) with -dev suffix and commit
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "7) Bumping project to next development version: '$NextDevVersion'..." -ForegroundColor Cyan
Set-ProjectVersion -NewVersion $NextDevVersion

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NextDevVersion"
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error while committing version '$NextDevVersion'." -ForegroundColor Red
    exit 1
}
Write-Host "   -> Commit created for '$NextDevVersion'." -ForegroundColor Green

# ------------------------------------------------------------------------------
# 8) Push new -dev version to GitHub
# ------------------------------------------------------------------------------
Write-Host ""
Write-Host "8) Pushing new version '$NextDevVersion' to GitHub..." -ForegroundColor Cyan
git push origin $CurrentBranch
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Error while pushing version '$NextDevVersion'." -ForegroundColor Red
    exit 1
}

Write-Header "🎉 Operation completed successfully!"
Write-Host "  • Published release : $TagName (v$ReleaseVersion)" -ForegroundColor Green
Write-Host "  • Branch $CurrentBranch       : $NextDevVersion" -ForegroundColor Cyan
Write-Host "  • CI build tracking : https://github.com/BolzaW/greenup-link/actions" -ForegroundColor Yellow
Write-Host ""
