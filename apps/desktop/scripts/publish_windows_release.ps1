# Publish the Windows installer (.msi + -setup.exe) to a GitHub Release so INSTALL.md's "Download from
# the Releases page" actually has a Windows artifact to offer (today only the macOS .dmg + model GGUF are
# published). Run this AFTER build_windows.ps1 has produced the installers.
#
#   powershell -ExecutionPolicy Bypass -File apps\desktop\scripts\publish_windows_release.ps1 [-Tag v0.1.0] [-Build]
#
# PREREQUISITES:
#   - GitHub CLI (`gh`) installed and authenticated:  gh auth login
#   - The installers already built (or pass -Build to run build_windows.ps1 first).
#
# The release is created if it doesn't exist yet, and assets are uploaded with --clobber so re-running
# replaces them in place (idempotent). The build is NOT code-signed — see docs/INSTALL.md for the
# SmartScreen note users will see.

param(
  [string]$Tag   = "v0.1.0",
  [string]$Title = "",
  [string]$Notes = "",
  [switch]$Build,
  [switch]$Draft
)

$ErrorActionPreference = "Stop"
$repo = (git rev-parse --show-toplevel).Trim()
Set-Location $repo

# gh must be present and authenticated, or the upload fails late and confusingly.
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
  throw "GitHub CLI (gh) not found. Install from https://cli.github.com/ and run 'gh auth login'."
}
try { gh auth status 1>$null } catch { throw "gh is not authenticated — run 'gh auth login' first." }

if ($Build) {
  Write-Host "== building installers first (build_windows.ps1) =="
  & (Join-Path $repo "apps\desktop\scripts\build_windows.ps1")
}

$bundle = Join-Path $repo "apps\desktop\src-tauri\target\release\bundle"
$msi = Get-ChildItem -Path (Join-Path $bundle "msi")  -Filter *.msi -ErrorAction SilentlyContinue |
       Sort-Object LastWriteTime -Descending | Select-Object -First 1
$exe = Get-ChildItem -Path (Join-Path $bundle "nsis") -Filter *.exe -ErrorAction SilentlyContinue |
       Sort-Object LastWriteTime -Descending | Select-Object -First 1

$assets = @()
if ($msi) { $assets += $msi.FullName } else { Write-Warning "no .msi found under $bundle\msi — did build_windows.ps1 run?" }
if ($exe) { $assets += $exe.FullName } else { Write-Warning "no -setup.exe found under $bundle\nsis — did build_windows.ps1 run?" }
if ($assets.Count -eq 0) { throw "No installers to publish. Run build_windows.ps1 (or pass -Build)." }

Write-Host "== assets to publish =="
$assets | ForEach-Object { Write-Host "  $_" }

# Create the release if the tag doesn't exist yet; otherwise upload into the existing one.
$exists = $true
try { gh release view $Tag 1>$null 2>$null; if ($LASTEXITCODE -ne 0) { $exists = $false } } catch { $exists = $false }

if (-not $exists) {
  if (-not $Title) { $Title = "Mod Creation Engine $Tag" }
  if (-not $Notes) { $Notes = "Windows installer for the Mod Creation Engine ($Tag). Not code-signed yet — Windows SmartScreen may warn: More info -> Run anyway. See docs/INSTALL.md." }
  Write-Host "== creating release $Tag =="
  $args = @("release","create",$Tag,"--title",$Title,"--notes",$Notes)
  if ($Draft) { $args += "--draft" }
  & gh @args
}

Write-Host "== uploading assets to $Tag (clobbering any existing) =="
& gh release upload $Tag @assets --clobber

Write-Host ""
Write-Host "DONE. View it with:  gh release view $Tag --web"
