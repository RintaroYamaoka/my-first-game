#!/bin/bash
# 見張り番: /root/sample のファイルの変化を、2 本目の直列の線(ttyS1)でゲームへ知らせる。
#   F <中身(base64。空なら -)> <道筋>   ファイルができた・変わった
#   D <道筋>                              消えた(フォルダなら、中身ごと)
#   S                                     ここまでで一区切り(ゲームは、これを見て世界を作り直す)
root=/root/sample; out=/dev/ttyS1
stty -F "$out" raw -echo 2>/dev/null || exit 0
exec 3> "$out"
skip='(^|/)(\.git|node_modules)(/|$)|\.sw[a-z]$|~$|(^|/)4913$'
send_file() {
  [[ "$1" =~ $skip ]] && return
  local b; b=$(base64 -w0 "$root/$1" 2>/dev/null) || return
  printf 'F %s %s\n' "${b:--}" "$1" >&3
}
send_tree() { (cd "$root" && find "${1:-.}" -type f | sed 's|^\./||') | while read -r f; do send_file "$f"; done; }
send_tree; echo S >&3
inotifywait -m -r -q -e close_write,create,delete,moved_from,moved_to --exclude '/\.git(/|$)' --format '%w%f' "$root" | while read -r p; do
  rel=${p#"$root"/}
  [[ "$rel" =~ $skip ]] && continue
  if [ -f "$p" ]; then send_file "$rel"; elif [ -d "$p" ]; then send_tree "$rel"; else printf 'D %s\n' "$rel" >&3; fi
  echo S >&3
done
