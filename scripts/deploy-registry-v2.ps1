# Deploy Artifex registry v2 to Testnet while keeping v1 IDs untouched.
# Redeploys: fee_collector, cert_registry, participation, marketplace, distribution
# Reuses from deployments/testnet.json (v1): roles, connoisseur, whole_nft, SAC IDs
#
# No secrets logged or committed. Requires stellar CLI + artifex-deployer identity
# (or STELLAR_DEPLOYER_SECRET).
$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$v1Path = Join-Path $root "deployments\testnet.json"
if (-not (Test-Path $v1Path)) { throw "missing $v1Path - need v1 deploy as baseline" }
$v1 = Get-Content $v1Path -Raw | ConvertFrom-Json

$identity = "artifex-deployer"
$keys = @(stellar keys ls 2>$null)
if ($env:STELLAR_DEPLOYER_SECRET) {
  $env:STELLAR_DEPLOYER_SECRET | stellar keys add $identity --secret-key 2>$null
} elseif ($keys -notcontains $identity) {
  Write-Host "Generating ephemeral deployer identity"
  stellar keys generate $identity --network testnet --fund | Out-Null
}

$addr = stellar keys address $identity
Write-Host "Deployer: $addr"
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
  $id = stellar contract deploy --wasm $wasm --source $identity --network testnet --alias "${name}_v2"
  $id = ($id | Select-Object -Last 1).ToString().Trim()
  Write-Host "Deployed $name (v2) -> $id"
  return $id
}

# Reuse v1
$roles = $v1.contracts.artifex_roles
$cnn = $v1.contracts.artifex_connoisseur
$nft = $v1.contracts.artifex_whole_nft
$usdcSac = $v1.usdcSac
$xlmSac = $v1.xlmSac

# Fresh v2 instances (ABI changes)
$fee = Deploy-Wasm "artifex_fee_collector"
$cert = Deploy-Wasm "artifex_cert_registry"
$part = Deploy-Wasm "artifex_participation"
$mkt = Deploy-Wasm "artifex_marketplace"
$dist = Deploy-Wasm "artifex_distribution"

stellar contract invoke --id $fee --source $identity --network testnet -- init --platform $addr --usdc $usdcSac --xlm_fee 50000000 --usdc_fee 10000000 | Out-Null
stellar contract invoke --id $cert --source $identity --network testnet -- init --platform $addr --validator $addr --fee_collector $fee --connoisseur $cnn | Out-Null
stellar contract invoke --id $fee --source $identity --network testnet -- set_cert_registry --caller $addr --cert_registry $cert | Out-Null
stellar contract invoke --id $part --source $identity --network testnet -- init --admin $addr | Out-Null
stellar contract invoke --id $mkt --source $identity --network testnet -- init --admin $addr --participation $part | Out-Null
stellar contract invoke --id $dist --source $identity --network testnet -- init --admin $addr --participation $part | Out-Null

$result = [ordered]@{
  network           = "testnet"
  version           = "v2"
  deployer          = $addr
  deployedAt        = (Get-Date).ToUniversalTime().ToString("o")
  keepsV1           = $true
  v1Source          = "deployments/testnet.json"
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
  apiWiringNotes    = @(
    "Copy contracts into artifex-api registry.json versions.v2 (keep v1 active until cutover)."
    "Wire POST /tx/cert/finalize-with-fee to finalize_with_fee(fee_asset, xlm_sac)."
    "Prefer finalize_with_fee over collect + mark_fee_paid + finalize."
    "create_offer takes payment_tokens[] + unit_prices[]; buy_primary requires payment_token."
    "Do not default paymentToken to USDC when XLM was selected."
  )
}
$jsonPath = Join-Path $outDir "testnet-v2.json"
$result | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding utf8
Write-Host "Wrote $jsonPath"
Write-Host "v1 IDs unchanged in deployments/testnet.json - lead activates registry v2 in API when ready."
