# Bind fee_collector consumer to a cert_registry (platform-signed).
# Use after deploying a new cert_registry that reuses an existing fee_collector.
# Example (Testnet v4):
#   .\scripts\bind-fee-consumer.ps1 `
#     -FeeCollector CATWKQFEW2CGBPDVEUYFMJAC27NRNTLDGAMNQHKVMU3OZ54XHHHRMJMS `
#     -CertRegistry CDD2BYHRY7ZCMMHBRETM4X2EGVCTKMAGKNRWSCJYHOTLLCWYK2UZY4CC `
#     -Network testnet -Identity artifex-deployer
param(
  [Parameter(Mandatory = $true)][string]$FeeCollector,
  [Parameter(Mandatory = $true)][string]$CertRegistry,
  [ValidateSet("testnet", "mainnet")][string]$Network = "testnet",
  [string]$Identity = "artifex-deployer"
)
$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"

if ($Network -eq "mainnet" -and $env:ARTIFEX_CONFIRM_MAINNET -ne "JOEL_CONFIRMED") {
  throw "Refusing mainnet bind without ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED"
}

$platform = stellar keys address $Identity
Write-Host "Platform: $platform"
Write-Host "Fee:      $FeeCollector"
Write-Host "Consumer: $CertRegistry ($Network)"

stellar contract invoke `
  --id $FeeCollector `
  --source $Identity `
  --network $Network `
  -- set_consumer --caller $platform --consumer $CertRegistry

Write-Host "Bound. Verify finalize_cert against $CertRegistry now succeeds when deposit+sigs are ready."
