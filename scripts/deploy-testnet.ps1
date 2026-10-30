# Deploy Artshio Soroban contracts to Testnet. No secrets logged or committed.
# Crate dirs remain artifex_* (legacy; do not redeploy for brand).
# Requires: stellar CLI on PATH, network testnet, and either:
#   STELLAR_DEPLOYER_SECRET  (S...)  OR an existing `stellar keys` identity named artifex-deployer
# Funds via Friendbot when balance is zero.
$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$identity = "artifex-deployer"
$keys = stellar keys ls 2>$null
if ($env:STELLAR_DEPLOYER_SECRET) {
  # Import without echoing secret
  $env:STELLAR_DEPLOYER_SECRET | stellar keys add $identity --secret-key 2>$null
  if ($LASTEXITCODE -ne 0) {
    # may already exist
  }
} elseif ($keys -notcontains $identity) {
  Write-Host "Generating ephemeral deployer identity (save secret yourself if needed)"
  stellar keys generate $identity --network testnet --fund | Out-Null
}

$addr = stellar keys address $identity
Write-Host "Deployer: $addr"
# Friendbot fund (idempotent)
try {
  Invoke-RestMethod -Uri "https://friendbot.stellar.org/?addr=$addr" -Method Get | Out-Null
} catch {
  Write-Host "Friendbot skip/fail (may already funded)"
}

& "$PSScriptRoot\build.ps1"

$wasmDir = Join-Path $root "target\wasm32v1-none\release"
$outDir = Join-Path $root "deployments"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function Deploy-Wasm([string]$name) {
  $wasm = Join-Path $wasmDir "$name.wasm"
  if (-not (Test-Path $wasm)) { throw "missing $wasm" }
  $id = stellar contract deploy --wasm $wasm --source $identity --network testnet --alias $name
  $id = ($id | Select-Object -Last 1).ToString().Trim()
  Write-Host "Deployed $name -> $id"
  return $id
}

$roles = Deploy-Wasm "artifex_roles"
$fee = Deploy-Wasm "artifex_fee_collector"
$cnn = Deploy-Wasm "artifex_connoisseur"
$cert = Deploy-Wasm "artifex_cert_registry"
$nft = Deploy-Wasm "artifex_whole_nft"
$part = Deploy-Wasm "artifex_participation"
$mkt = Deploy-Wasm "artifex_marketplace"
$dist = Deploy-Wasm "artifex_distribution"

# Circle Testnet USDC issuer (public)
$usdcIssuer = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
# Resolve USDC SAC id on testnet
$usdcSac = stellar contract id asset --asset "USDC:$usdcIssuer" --network testnet
$usdcSac = ($usdcSac | Select-Object -Last 1).ToString().Trim()
Write-Host "USDC SAC: $usdcSac"

# Native XLM SAC
$xlmSac = stellar contract id asset --asset native --network testnet
$xlmSac = ($xlmSac | Select-Object -Last 1).ToString().Trim()

# Init (5 XLM = 50000000 stroops, 1 USDC = 10000000 with 7 decimals)
stellar contract invoke --id $roles --source $identity --network testnet -- init --platform $addr | Out-Null
stellar contract invoke --id $fee --source $identity --network testnet -- init --platform $addr --usdc $usdcSac --xlm_fee 50000000 --usdc_fee 10000000 | Out-Null
stellar contract invoke --id $cnn --source $identity --network testnet -- init --platform $addr | Out-Null
stellar contract invoke --id $cert --source $identity --network testnet -- init --platform $addr --validator $addr --fee_collector $fee --connoisseur $cnn | Out-Null
# Required: fee_collector.consume_deposit only accepts this consumer. Skipping leaves finalize broken.
stellar contract invoke --id $fee --source $identity --network testnet -- set_consumer --caller $addr --consumer $cert | Out-Null
stellar contract invoke --id $nft --source $identity --network testnet -- init --admin $addr | Out-Null
stellar contract invoke --id $part --source $identity --network testnet -- init --admin $addr | Out-Null
stellar contract invoke --id $mkt --source $identity --network testnet -- init --admin $addr --participation $part | Out-Null
stellar contract invoke --id $dist --source $identity --network testnet -- init --admin $addr --participation $part | Out-Null

$result = [ordered]@{
  network           = "testnet"
  deployer          = $addr
  deployedAt        = (Get-Date).ToUniversalTime().ToString("o")
  usdcSac           = $usdcSac
  xlmSac            = $xlmSac
  contracts         = [ordered]@{
    artifex_roles           = $roles
    artifex_fee_collector   = $fee
    artifex_connoisseur     = $cnn
    artifex_cert_registry   = $cert
    artifex_whole_nft       = $nft
    artifex_participation   = $part
    artifex_marketplace     = $mkt
    artifex_distribution    = $dist
  }
}
$jsonPath = Join-Path $outDir "testnet.json"
$result | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonPath -Encoding utf8
Write-Host "Wrote $jsonPath"

# API loads contract IDs from its registry; do not duplicate IDs into env.
# testnet.json is the source of truth for this deploy. Pointer only:
$envSnippet = @"
# Contract IDs: see deployments/testnet.json (source of truth).
# Runtime registry lives in artifex-api — copy IDs there after deploy; do not commit secrets.
STELLAR_NETWORK=testnet
"@
Set-Content -Path (Join-Path $outDir "testnet.api.env") -Value $envSnippet -Encoding utf8
Write-Host "Wrote deployments/testnet.api.env (STELLAR_NETWORK + pointer to testnet.json)"

Write-Host ""
Write-Host "=== After deploy ==="
Write-Host "1. Commit/update deployments/testnet.json (public IDs only)."
Write-Host "2. Register IDs in artifex-api (registry is source of truth at runtime)."
Write-Host "3. Tag main with SemVer vX.Y.Z (e.g. v0.1.0). contracts release.yml validates the tag - no Harbor image."
