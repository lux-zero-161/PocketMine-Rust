# PocketMine-Rust Windows PowerShell Launcher
$ErrorActionPreference = "Stop"

Set-Location $PSScriptRoot
$host.UI.RawUI.WindowTitle = "PocketMine-Rust Server"

Write-Host "=== Starting PocketMine-Rust ===" -ForegroundColor Cyan

if (Test-Path "PocketMine.exe") {
    & ".\PocketMine.exe" @args
} elseif (Test-Path "target\release\PocketMine.exe") {
    & ".\target\release\PocketMine.exe" @args
} elseif (Test-Path "target\debug\PocketMine.exe") {
    & ".\target\debug\PocketMine.exe" @args
} else {
    Write-Host "Binary not found, compiling release build..." -ForegroundColor Yellow
    cargo run --bin PocketMine --release -- @args
}
