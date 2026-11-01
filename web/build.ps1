# Baut die Browser-Version nach web/dist.
#   Einmalig nötig:  rustup target add wasm32-unknown-unknown
#                    cargo install wasm-bindgen-cli --version <Version aus Cargo.lock> --locked
# Ausprobieren:      python -m http.server 8123 --directory web/dist
#                    und dann http://localhost:8123 öffnen.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$env:CARGO_TARGET_DIR = Join-Path $root 'target/web'
cargo build --profile web --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$dist = Join-Path $root 'web/dist'
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $dist | Out-Null

wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section --out-dir $dist (Join-Path $env:CARGO_TARGET_DIR 'wasm32-unknown-unknown/web/nova_numerix.wasm')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item (Join-Path $root 'web/index.html') $dist
Copy-Item -Recurse (Join-Path $root 'assets') (Join-Path $dist 'assets')
Write-Host "Fertig: $dist"
