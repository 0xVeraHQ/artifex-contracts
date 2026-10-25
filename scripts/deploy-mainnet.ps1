# Deploy Artifex Soroban contracts to Stellar MAINNET.
# HARD STOP: refuses unless ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED.
# Does NOT Friendbot-fund. Deployer must already hold XLM on mainnet.
#
# Requires: stellar CLI, STELLAR_DEPLOYER_SECRET (S…) OR identity artifex-mn-deployer
# Optional: STELLAR_RPC_URL (mainnet Soroban RPC — required by stellar network config)
#
# After deploy: copy C… IDs into artifex-api registry.json → mainnet-v3, tag release.
$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if ($env:ARTIFEX_CONFIRM_MAINNET -ne "JOEL_CONFIRMED") {
  Write-Error "HARD STOP: set ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED after Joel explicitly confirms. No mainnet deploy."
  exit 2
}

$identity = "artifex-mn-deployer"
$keys = stellar keys ls 2>$null
if ($env:STELLAR_DEPLOYER_SECRET) {
  $env:STELLAR_DEPLOYER_SECRET | stellar keys add $identity --secret-key 2>$null
} elseif ($keys -notcontains $identity) {
  Write-Error "Missing identity '$identity'. Import STELLAR_DEPLOYER_SECRET or: stellar keys generate $identity --network mainnet (then fund the G… address)."
  exit 1
}

$addr = stellar keys address $identity
Write-Host "Deployer (mainnet): $addr"
Write-Host "NO Friendbot — account must already be funded."

& "$PSScriptRoot\build.ps1"

$wasmDir = Join-Path $root "target\wasm32v1-none\release"
$outDir = Join-Path $root "deployments"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function Deploy-Wasm([string]$name) {
  $wasm = Join-Path $wasmDir "$name.wasm"
  if (-not (Test-Path $wasm)) { throw "missing $wasm" }
  $id = stellar contract deploy --wasm $wasm --source $identity --network mainnet --alias $name
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

# Circle Mainnet USDC issuer (public) — NOT testnet GBBD…
$usdcIssuer = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
$usdcSac = stellar contract id asset --asset "USDC:$usdcIssuer" --network mainnet
$usdcSac = ($usdcSac | Select-Object -Last 1).ToString().Trim()
Write-Host "USDC SAC (mainnet): $usdcSac"

$xlmSac = stellar contract id asset --asset native --network mainnet
$xlmSac = ($xlmSac | Select-Object -Last 1).ToString().Trim()
Write-Host "XLM SAC (mainnet): $xlmSac"

# Init (5 XLM = 50000000 stroops, 1 USDC = 10000000 with 7 decimals)
stellar contract invoke --id $roles --source $identity --network mainnet -- init --platform $addr | Out-Null
stellar contract invoke --id $fee --source $identity --network mainnet -- init --platform $addr --usdc $usdcSac --xlm_fee 50000000 --usdc_fee 10000000 | Out-Null
stellar contract invoke --id $cnn --source $identity --network mainnet -- init --platform $addr | Out-Null
stellar contract invoke --id $cert --source $identity --network mainnet -- init --platform $addr --validator $addr --fee_collector $fee --connoisseur $cnn | Out-Null
stellar contract invoke --id $nft --source $identity --network mainnet -- init --admin $addr | Out-Null
stellar contract invoke --id $part --source $identity --network mainnet -- init --admin $addr | Out-Null
stellar contract invoke --id $mkt --source $identity --network mainnet -- init --admin $addr --participation $part | Out-Null
stellar contract invoke --id $dist --source $identity --network mainnet -- init --admin $addr --participation $part | Out-Null

$result = [ordered]@{
  network           = "mainnet"
  deployer          = $addr
  deployedAt        = (Get-Date).ToUniversalTime().ToString("o")
  usdcIssuer        = $usdcIssuer
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
  wholeNftSaleV3    = @{
    list_for_sale = $true
    buy_sale      = $true
    cancel_sale   = $true
  }
}
$jsonPath = Join-Path $outDir "mainnet.json"
$result | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonPath -Encoding utf8
Write-Host "Wrote $jsonPath"

$envSnippet = @"
# Contract IDs: see deployments/mainnet.json
# Copy into artifex-api registry.json versions.mainnet-v3 then set activeVersion.
STELLAR_NETWORK=mainnet
ARTIFEX_ALLOW_MAINNET=1
"@
Set-Content -Path (Join-Path $outDir "mainnet.api.env") -Value $envSnippet -Encoding utf8

Write-Host ""
Write-Host "=== After mainnet deploy (still NO prod VPS cutover until Joel) ==="
Write-Host "1. Commit deployments/mainnet.json (public IDs)."
Write-Host "2. Fill artifex-api registry mainnet-v3 contract IDs; set activeVersion=mainnet-v3 for mainnet image."
Write-Host "3. Tag contracts release; build Harbor API/web tags; then orch mainnet-cutover.sh"
