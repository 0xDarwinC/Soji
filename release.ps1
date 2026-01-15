# usage: .\release.ps1 patch -Suffix alpha
param (
    [string]$Level = "patch", # patch, minor, or major
    [string]$Suffix = ""      # Optional: "alpha", "beta", etc.
)

$ErrorActionPreference = "Stop"

# 1. Bump npm version
Write-Host "1. Incrementing version ($Level)..." -ForegroundColor Cyan
npm version $Level --no-git-tag-version
$ver = (Get-Content package.json | ConvertFrom-Json).version

# 2. Sync Cargo.toml
#    Tauri will read the version from here automatically now.
Write-Host "2. Updating Cargo.toml..." -ForegroundColor Cyan
$cargoPath = "src-tauri/Cargo.toml"
(Get-Content $cargoPath) -replace '^version = ".*?"$', "version = `"$ver`"" | Set-Content $cargoPath

# 3. Sync Cargo.lock
Write-Host "3. Syncing Cargo.lock..." -ForegroundColor Cyan
Push-Location src-tauri; cargo check; Pop-Location

# 4. Create the Tag
$tagName = "v$ver"
if ($Suffix) { $tagName = "$tagName-$Suffix" }

Write-Host "4. Tagging as $tagName..." -ForegroundColor Cyan
git add .
git commit -m "chore: release $tagName"
git tag $tagName

Write-Host "------------------------------------------------" -ForegroundColor Green
Write-Host "Success!"
Write-Host "Files are version: $ver"
Write-Host "Github Tag is:     $tagName"
Write-Host "Run 'git push origin main --tags' to publish." -ForegroundColor Yellow