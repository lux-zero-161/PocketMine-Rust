#!/usr/bin/env bash
# Builds the whole ecosystem: C# SDK, C# host, example plugin, Rust server.
set -euo pipefail

cd "$(dirname "$0")"

export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1

echo "=== Building PocketMine-Rust ==="

echo "[1/4] C# SDK (PocketMine.Sdk)"
dotnet build csharp/PocketMine.Sdk/PocketMine.Sdk.csproj -c Release -v q

echo "[2/4] C# plugin host (PocketMine.Host)"
dotnet build csharp/PocketMine.Host/PocketMine.Host.csproj -c Release -v q

echo "[3/4] Example C# plugin"
dotnet build csharp/plugins/ExamplePlugin/ExamplePlugin.csproj -c Release -v q
mkdir -p plugins
cp csharp/plugins/ExamplePlugin/bin/Release/net10.0/ExamplePlugin.dll plugins/

echo "[4/4] Rust server"
cargo build --release

for name in pocketmine PocketMine pmmp; do
    cp target/release/PocketMine "./$name"
done

echo
echo "=== Build complete ==="
echo "Start with: ./start.sh   (Linux)"
echo "            start.cmd    (Windows)"
echo "Docs:       README.md, PORTING.md"
