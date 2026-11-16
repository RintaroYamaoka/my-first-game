#!/usr/bin/env bash
# Windows 用に作って、Windows で動かす(GPU を使う。速さと滑らかさを見るのはこちら)。
#   ./scripts/run-windows.sh <パッケージ名> [プログラムへの引数...]
#   例: ./scripts/run-windows.sh world-from-code experiments/01-world-from-code/sample
# 引数のうち、実在するファイルやフォルダは Windows から見える書き方に直して渡す。
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
pkg="${1:?パッケージ名を指定する}"; shift
cargo build --manifest-path "$root/Cargo.toml" -p "$pkg" --target x86_64-pc-windows-gnu

# 実行ファイルは Windows 側のフォルダに置いてから動かす
win_local="$(wslpath "$(cmd.exe /c 'echo %LOCALAPPDATA%' 2>/dev/null | tr -d '\r')")"
out="$win_local/my-first-game/dev"
mkdir -p "$out"
cp "$root/target/x86_64-pc-windows-gnu/debug/$pkg.exe" "$out/"

args=()
for a in "$@"; do
  if [ -e "$a" ]; then args+=("$(wslpath -w "$a")"); else args+=("$a"); fi
done
exec "$out/$pkg.exe" "${args[@]}"
