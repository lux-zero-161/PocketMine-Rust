#!/usr/bin/env bash
# End-to-end smoke test: builds everything, runs unit tests and drives the
# server (including the C# plugin) with a scripted console session.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
PLUGIN_DLL="csharp/plugins/ExamplePlugin/bin/Release/net10.0/ExamplePlugin.dll"

echo "== 1/5 Rust unit tests =="
cargo test --workspace --quiet

echo "== 2/5 Build C# SDK + host + example plugin =="
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1
dotnet build csharp/PocketMine.Host/PocketMine.Host.csproj -c Release -v q
dotnet build csharp/plugins/ExamplePlugin/ExamplePlugin.csproj -c Release -v q

echo "== 3/5 Build Rust server =="
cargo build --quiet

echo "== 4/5 Scripted server session =="
mkdir -p plugins
cp "$PLUGIN_DLL" plugins/
OUTPUT="$(mktemp)"
printf '%s\n' \
  'info' \
  'plugins' \
  'event join SmokeTester' \
  'hello' \
  'csharp' \
  'event chat SmokeTester !ping' \
  'world getblock 0 3 0' \
  'world setblock 12 6 12 minecraft:diamond_block' \
  'world getblock 12 6 12' \
  'event break SmokeTester' \
  | ./target/debug/PocketMine --no-network --ticks 40 >"$OUTPUT" 2>&1

grep -q "Loaded C# plugin ExamplePlugin" "$OUTPUT"
grep -q "connected\|initialized" "$OUTPUT"
grep -q "Pong from C#" "$OUTPUT"
grep -q "minecraft:diamond_block" "$OUTPUT"
grep -q "Server stopped after" "$OUTPUT"
echo "   scripted session OK ($(wc -l <"$OUTPUT") log lines)"

echo "== 5/5 UDP LAN ping =="
./target/debug/PocketMine --no-plugins --port 19201 --ticks 80 >/tmp/pm_smoke_net.log 2>&1 &
SERVER_PID=$!
sleep 0.7
python3 tools/ping.py 19201
wait "$SERVER_PID" || true

echo
echo "All smoke tests passed."
