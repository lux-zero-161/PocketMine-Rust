#!/usr/bin/env bash
set -e

echo "=== Building PocketMine-Rust Ecosystem ==="

echo "[1/4] Building C# SDK (PocketMine.Sdk)..."
XDG_RUNTIME_DIR=/tmp dotnet build csharp/PocketMine.Sdk/PocketMine.Sdk.csproj -c Release

echo "[2/4] Building C# Host (PocketMine.Host)..."
XDG_RUNTIME_DIR=/tmp dotnet build csharp/PocketMine.Host/PocketMine.Host.csproj -c Release

echo "[3/4] Building Sample C# Plugin (ExamplePlugin)..."
XDG_RUNTIME_DIR=/tmp dotnet build csharp/plugins/ExamplePlugin/ExamplePlugin.csproj -c Release
mkdir -p plugins
cp csharp/plugins/ExamplePlugin/bin/Release/net10.0/ExamplePlugin.dll plugins/

echo "[4/4] Building Rust Server Core (PocketMine)..."
cargo build --release

# Copy binaries
cp target/release/PocketMine ./pocketmine
cp target/release/PocketMine ./pmmp
cp target/release/PocketMine ./PocketMine

echo "=== Build Complete! ==="
echo "Run './start.sh' to start the server on Linux."
echo "Run 'start.cmd' or 'start.ps1' on Windows."
