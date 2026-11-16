#!/usr/bin/env bash
# 開発環境を整える。何度実行してもよい。
#   ./scripts/setup-dev.sh
# 管理者権限(sudo)を使うのは、OS の部品を入れる 1 か所だけ。
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== 1/4 OS の部品 =="
mapfile -t pkgs < <(sed 's/#.*//' scripts/packages.txt | awk 'NF {print $1}')
missing=()
for p in "${pkgs[@]}"; do
  dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q "install ok installed" || missing+=("$p")
done
if ((${#missing[@]})); then
  echo "入れるもの: ${missing[*]}"
  sudo apt-get update
  sudo apt-get install -y "${missing[@]}"
else
  echo "すべて入っている"
fi

echo "== 2/4 音の出力先(WSL のときだけ) =="
# WSL には音の装置がない。音は WSLg(PulseAudio)経由で Windows から出るので、ALSA の既定をそこへ向ける
if [ -n "${WSL_DISTRO_NAME:-}" ]; then
  if [ -e "$HOME/.asoundrc" ]; then
    echo "~/.asoundrc は既にある(変更しない)"
  else
    printf 'pcm.!default { type pulse }\nctl.!default { type pulse }\n' > "$HOME/.asoundrc"
    echo "~/.asoundrc を作った"
  fi
else
  echo "WSL ではないので何もしない"
fi

echo "== 3/4 Rust =="
command -v rustup >/dev/null || { echo "rustup がない。https://rustup.rs の手順で入れてから、もう一度実行する"; exit 1; }
rustup show active-toolchain   # rust-toolchain.toml の版と、Windows 用の部品が入る

echo "== 4/4 点検 =="
./scripts/check-env.sh
