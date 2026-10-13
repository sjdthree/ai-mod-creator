# Build the Windows installer for the AI-mod creator desktop app.
#
# Produces a self-contained .msi + .exe installer (no Python) under
#   apps/desktop/src-tauri/target/release/bundle/{msi,nsis}
#
# Run from the repo root in a "x64 Native Tools Command Prompt for VS 2022" PowerShell, OR a normal
# PowerShell if cmake/cl are already on PATH:
#   powershell -ExecutionPolicy Bypass -File apps\desktop\scripts\build_windows.ps1
#
# PREREQUISITES (one-time on the Windows box):
#   - Visual Studio 2022 Build Tools with the "Desktop development with C++" workload (gives MSVC + CMake)
#   - Rust:      https://rustup.rs   (installs the x86_64-pc-windows-msvc target by default)
#   - Tauri CLI: cargo install tauri-cli --version "^2.0" --locked
#   - Git, and WebView2 Runtime (preinstalled on Windows 11)
#   - Nothing else — the model runs with NO Python.
# END USERS of the installer additionally need JDK 21 (Temurin) for the Gradle build step.

$ErrorActionPreference = "Stop"
$repo = (git rev-parse --show-toplevel).Trim()
Set-Location $repo
$triple = (rustc -vV | Select-String '^host:' ).ToString().Split(' ')[1]   # x86_64-pc-windows-msvc
Write-Host "== target triple: $triple =="

$tauri     = Join-Path $repo "apps\desktop\src-tauri"
$binaries  = Join-Path $tauri "binaries"
$resModels = Join-Path $tauri "resources\models"
$resLic    = Join-Path $tauri "resources\licenses"
New-Item -ItemType Directory -Force -Path $binaries,$resModels,$resLic | Out-Null

# 1) engine — amc.exe
Write-Host "== [1/5] building amc.exe =="
cargo build --release -p amc-cli --manifest-path (Join-Path $repo "Cargo.toml")
Copy-Item (Join-Path $repo "target\release\amc.exe") (Join-Path $binaries "amc-$triple.exe") -Force

# 2) model runtime — a STATIC llama-server.exe (single self-contained exe, static CRT)
Write-Host "== [2/5] building static llama-server.exe =="
$llamaSrc = Join-Path $env:TEMP "llama.cpp"
if (-not (Test-Path $llamaSrc)) { git clone --depth 1 https://github.com/ggml-org/llama.cpp $llamaSrc }
Push-Location $llamaSrc
cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF `
      -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded `
      -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DGGML_OPENMP=OFF `
      -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF
cmake --build build --target llama-server --config Release -j
Pop-Location
$llamaExe = Get-ChildItem -Path (Join-Path $llamaSrc "build") -Recurse -Filter "llama-server.exe" | Select-Object -First 1
Copy-Item $llamaExe.FullName (Join-Path $binaries "llama-server-$triple.exe") -Force

# 3) the model pinned in model.lock, from Hugging Face — amc-model.json (sha256-pinned by model.lock) +
#    the GGUF it names, then verified against this engine (contract/grammar pins + weights sha256).
#    While the Hugging Face repo is private, set $env:HF_TOKEN.
Write-Host "== [3/5] fetching + verifying the model =="
$lock = @{}
Get-Content (Join-Path $repo "model.lock") | Where-Object { $_ -match '^\s*([a-z_0-9]+)=(.*)$' } |
  ForEach-Object { $lock[$Matches[1]] = $Matches[2].Trim() }
$name = $lock["name"]
$modelDir = if ($env:AMC_MODEL_DIR) { $env:AMC_MODEL_DIR } else { Join-Path $repo "models\$name" }
$amc = Join-Path $repo "target\release\amc.exe"
if (-not $env:AMC_MODEL_DIR) {
  New-Item -ItemType Directory -Force -Path $modelDir | Out-Null
  $base = "https://huggingface.co/$($lock['repo'])/resolve/$($lock['revision'])"
  $headers = @{}
  if ($env:HF_TOKEN) { $headers["Authorization"] = "Bearer $($env:HF_TOKEN)" }
  $manifestPath = Join-Path $modelDir "amc-model.json"
  Invoke-WebRequest -Uri "$base/$($lock['manifest_file'])" -OutFile $manifestPath -Headers $headers
  $got = (Get-FileHash $manifestPath -Algorithm SHA256).Hash.ToLower()
  if ($got -ne $lock["manifest_sha256"]) { throw "amc-model.json sha256 $got does not match model.lock" }
  $m = Get-Content $manifestPath -Raw | ConvertFrom-Json
  $ggufPath = Join-Path $modelDir $m.gguf
  if (-not ((Test-Path $ggufPath) -and ((Get-FileHash $ggufPath -Algorithm SHA256).Hash.ToLower() -eq $m.gguf_sha256))) {
    Invoke-WebRequest -Uri "$base/$($m.gguf)" -OutFile $ggufPath -Headers $headers
  }
}
& $amc model verify $modelDir
if ($LASTEXITCODE -ne 0) { throw "the model is not compatible with this engine" }
$staged = Join-Path $resModels $name
New-Item -ItemType Directory -Force -Path $staged | Out-Null
$manifest = Get-Content (Join-Path $modelDir "amc-model.json") -Raw | ConvertFrom-Json
Copy-Item (Join-Path $modelDir "amc-model.json") $staged -Force
Copy-Item (Join-Path $modelDir $manifest.gguf) $staged -Force

# 4) grammar (engine-owned) + license notices
Write-Host "== [4/5] staging grammar + license notices =="
Copy-Item (Join-Path $repo "crates\amc-intent\contract\amc-ops.gbnf") (Join-Path $tauri "resources\amc-ops.gbnf") -Force
Copy-Item (Join-Path $repo "LICENSE"),(Join-Path $repo "NOTICE") $resLic -Force
Copy-Item (Join-Path $repo "apps\desktop\licenses\*") $resLic -Force

# 5) bundle the installer
Write-Host "== [5/5] cargo tauri build (msi + nsis) =="
Set-Location $tauri
cargo tauri build
Write-Host ""
Write-Host "DONE. Installers are in:"
Write-Host "  $tauri\target\release\bundle\msi\   (.msi)"
Write-Host "  $tauri\target\release\bundle\nsis\  (.exe)"
