$ErrorActionPreference = "Stop"
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
& "$PSScriptRoot\set-crate-type.ps1" -Type rlib
Set-Location (Split-Path -Parent $PSScriptRoot)
cargo +stable-x86_64-pc-windows-gnu test
