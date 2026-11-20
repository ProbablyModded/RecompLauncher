#!/bin/sh
# Builds the Playnite add-on behind "Add to Playnite" (src-tauri/playnite/src) and puts the DLL
# next to its extension.yaml, where the launcher picks it up with include_bytes!. Needs the .NET
# SDK (6 or newer); it runs on Windows, Linux or macOS. The build is deterministic, so running this
# on an unchanged source leaves the committed DLL as it is (CI checks that).
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
src="$here/src-tauri/playnite/src"
out="$(mktemp -d)"
trap 'rm -rf "$out" "$src/bin" "$src/obj"' EXIT
dotnet build "$src/RecompLauncherLink.csproj" -c Release -o "$out" --nologo -v quiet
cp "$out/RecompLauncherLink.dll" "$here/src-tauri/playnite/RecompLauncherLink.dll"
echo "src-tauri/playnite/RecompLauncherLink.dll updated"
