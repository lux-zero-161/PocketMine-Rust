#!/usr/bin/env bash
set -e

# Ensure C# Host is built
if [ ! -f "csharp/PocketMine.Host/bin/Debug/net10.0/PocketMine.Host.dll" ] && [ ! -f "csharp/PocketMine.Host/bin/Release/net10.0/PocketMine.Host.dll" ]; then
    echo "Host binary not found. Running build.sh..."
    ./build.sh
fi

if [ -f "./pocketmine" ]; then
    exec ./pocketmine "$@"
elif [ -f "./target/release/PocketMine" ]; then
    exec ./target/release/PocketMine "$@"
elif [ -f "./target/debug/PocketMine" ]; then
    exec ./target/debug/PocketMine "$@"
else
    cargo run --bin PocketMine -- "$@"
fi
