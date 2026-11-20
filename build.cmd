@echo off
TITLE Build PocketMine-Rust
cd /d "%~dp0"

echo === Building PocketMine-Rust Ecosystem for Windows ===

echo [1/4] Building C# SDK (PocketMine.Sdk)...
dotnet build csharp\PocketMine.Sdk\PocketMine.Sdk.csproj -c Release
if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%

echo [2/4] Building C# Host (PocketMine.Host)...
dotnet build csharp\PocketMine.Host\PocketMine.Host.csproj -c Release
if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%

echo [3/4] Building Sample C# Plugin (ExamplePlugin)...
dotnet build csharp\plugins\ExamplePlugin\ExamplePlugin.csproj -c Release
if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%

if not exist "plugins" mkdir plugins
copy /y csharp\plugins\ExamplePlugin\bin\Release\net10.0\ExamplePlugin.dll plugins\

echo [4/4] Building Rust Server Core (PocketMine.exe)...
cargo build --release --bin PocketMine
if %ERRORLEVEL% NEQ 0 exit /b %ERRORLEVEL%

copy /y target\release\PocketMine.exe .\PocketMine.exe

echo === Build Complete! ===
echo Run 'start.cmd' or 'start.ps1' to start PocketMine-Rust.
pause
