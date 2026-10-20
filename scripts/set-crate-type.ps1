param(
  [Parameter(Mandatory = $true)][ValidateSet("rlib", "cdylib")][string]$Type
)
$root = Split-Path -Parent $PSScriptRoot
Get-ChildItem (Join-Path $root "contracts") -Directory | ForEach-Object {
  $p = Join-Path $_.FullName "Cargo.toml"
  $c = [System.IO.File]::ReadAllText($p)
  $c = [regex]::Replace($c, 'crate-type = \["(?:rlib|cdylib|lib)"\]', "crate-type = [`"$Type`"]")
  [System.IO.File]::WriteAllText($p, $c)
}
Write-Host "crate-type set to $Type"
