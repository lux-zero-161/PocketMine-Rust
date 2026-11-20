@echo off
TITLE PocketMine-Rust Server
cd /d "%~dp0"

echo Starting PocketMine-Rust...

if exist "PocketMine.exe" (
    PocketMine.exe %*
) else if exist "target\release\PocketMine.exe" (
    target\release\PocketMine.exe %*
) else if exist "target\debug\PocketMine.exe" (
    target\debug\PocketMine.exe %*
) else (
    echo PocketMine.exe not found. Building via Cargo...
    cargo run --bin PocketMine --release -- %*
)

pause
