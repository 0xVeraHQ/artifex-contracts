$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
& "$PSScriptRoot\set-crate-type.ps1" -Type cdylib
Set-Location (Split-Path -Parent $PSScriptRoot)
stellar contract build
