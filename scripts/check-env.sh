#!/usr/bin/env bash
# 開発環境が整っているかを点検する。何も変更しない。
#   ./scripts/check-env.sh
cd "$(dirname "$0")/.."
bad=0
row() { printf '  %-4s %-28s %s\n' "$1" "$2" "$3"; }
ok()  { row "OK" "$1" "$2"; }
ng()  { row "NG" "$1" "$2"; bad=1; }

echo "OS の部品"
while read -r p; do
  if dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q "install ok installed"; then
    ok "$p" "$(dpkg-query -W -f='${Version}' "$p")"
  else ng "$p" "入っていない"; fi
done < <(sed 's/#.*//' scripts/packages.txt | awk 'NF {print $1}')

echo "Rust"
want=$(awk -F'"' '/^channel/ {print $2}' rust-toolchain.toml)
have=$(rustc --version 2>/dev/null | awk '{print $2}')
[ "$have" = "$want" ] && ok "rustc" "$have" || ng "rustc" "今 ${have:-なし} / 必要 $want"
rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$' \
  && ok "Windows 用の部品" "x86_64-pc-windows-gnu" || ng "Windows 用の部品" "入っていない"

echo "表示と音(WSL)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && ok "窓を出す先" "DISPLAY=${DISPLAY:-} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-}" || ng "窓を出す先" "見つからない"
[ -n "${PULSE_SERVER:-}" ] && ok "音を出す先" "$PULSE_SERVER" || ng "音を出す先" "PULSE_SERVER がない"

if [ -n "${WSL_DISTRO_NAME:-}" ]; then
  grep -qs 'type pulse' "$HOME/.asoundrc" /etc/asound.conf && ok "音の既定の出力先" "PulseAudio(WSLg)" || ng "音の既定の出力先" "~/.asoundrc がない"
fi

echo "応急処置の残り"
[ -d "$HOME/.local/share/my-first-game-libs" ] && ng "手元に置いた部品" "~/.local/share/my-first-game-libs が残っている(消してよい)" || ok "手元に置いた部品" "ない"

echo
[ $bad = 0 ] && echo "整っている" || echo "NG がある。./scripts/setup-dev.sh を実行する"
exit $bad
