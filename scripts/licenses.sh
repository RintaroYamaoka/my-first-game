#!/usr/bin/env bash
# 同梱する部品のライセンスの一覧を作る(docs/decisions/0002)。出力: docs/licenses/
#   ./scripts/licenses.sh            # ゲーム本体(Rust)と、同梱の Linux(先に experiments/06-linux/build.sh で組み立てておく)
# 何かを足したら、これを走らせて、差分を見る。「要確認」が出たら、使う前に調べる
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"; out="$root/docs/licenses"; mkdir -p "$out"
PKG="${PKG:-terminal}"   # ゲーム本体になる部品(今は実験 4 の terminal で代用)

# 1. ゲーム本体に組み込まれる Rust の部品(Windows 用。作るときだけ使う部品は数えない)
( cd "$root" && cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc ) | PKG="$PKG" python3 -c '
import json, os, sys, collections
m = json.load(sys.stdin)
ids = {p["id"]: p for p in m["packages"]}
ws = set(m["workspace_members"])
root = next(p["id"] for p in m["packages"] if p["name"] == os.environ["PKG"])
dep = {n["id"]: [d["pkg"] for d in n["deps"] if any(k["kind"] is None for k in d["dep_kinds"])] for n in m["resolve"]["nodes"]}
seen, todo = set(), [root]
while todo:
    i = todo.pop()
    if i not in seen:
        seen.add(i); todo += dep[i]
parts = sorted((ids[i] for i in seen if i not in ws), key=lambda p: (p["name"], p["version"]))
lic = lambda p: (p.get("license") or "(ファイルで指定: %s)" % p.get("license_file")).replace("/", " OR ")
ok = ("MIT", "Apache", "BSD", "Zlib", "ISC", "Unicode", "CC0", "Unlicense", "BSL", "0BSD")
bad = ("GPL", "MPL", "CDDL", "EPL", "SSPL")
odd = [p for p in parts if not any(k in lic(p) for k in ok) or any(k in lic(p) for k in bad)]
count = collections.Counter(lic(p) for p in parts)
print("# ゲーム本体に組み込まれる Rust の部品\n")
print("`scripts/licenses.sh` が作る。手で書き換えない。\n")
print("- 部品の数: %d(Windows 用の実行ファイルに入るもの)" % len(parts))
print("- 要確認(コピーレフトなど、Steam の SDK と両立しないおそれがあるもの): %s\n" % (", ".join("%s %s (%s)" % (p["name"], p["version"], lic(p)) for p in odd) or "なし"))
print("| ライセンス | 数 |\n|---|---|")
for l, n in count.most_common():
    print("| %s | %d |" % (l, n))
print("\n| 部品 | 版 | ライセンス | 入手先 |\n|---|---|---|---|")
for p in parts:
    print("| %s | %s | %s | %s |" % (p["name"], p["version"], lic(p), p.get("repository") or "crates.io"))
' > "$out/rust.md"
sed -n 5,6p "$out/rust.md"

# 2. 同梱の Linux に入っている部品(Alpine の管理簿から読む)
for tag in "" "-layers"; do
  docker image inspect "my-first-game-linux:pack$tag" > /dev/null 2>&1 || { echo "(my-first-game-linux:pack$tag がない。飛ばす)"; continue; }
  docker run --rm "my-first-game-linux:pack$tag" cat /rootfs/lib/apk/db/installed | TAG="$tag" python3 -c '
import os, sys, collections
pk, cur = [], {}
for line in sys.stdin.read().split("\n"):
    if not line:
        if cur: pk.append(cur); cur = {}
    elif line[1] == ":" and line[0] in "PVLUo":
        cur[line[0]] = line[2:]
pk.sort(key=lambda p: p["P"])
name = "コンテナを動かす道具つき" if os.environ["TAG"] else "いつもの中身"
copyleft = [p for p in pk if "GPL" in p.get("L", "")]
print("# 同梱の Linux に入っている部品(%s)\n" % name)
print("`scripts/licenses.sh` が作る。手で書き換えない。元は Alpine Linux の管理簿。\n")
print("- 部品の数: %d" % len(pk))
print("- GPL / LGPL の部品: %d 個。ゲーム本体とは別のプログラムとして同梱し、ソースコードの入手先とライセンス文を添える\n" % len(copyleft))
print("カーネル(linux-virt、GPL-2.0-only)は、この一覧とは別に同梱する。\n")
print("| 部品 | 版 | ライセンス | 元になるパッケージ | 入手先 |\n|---|---|---|---|---|")
for p in pk:
    print("| %s | %s | %s | %s | %s |" % (p["P"], p.get("V", ""), p.get("L", "?"), p.get("o", ""), p.get("U", "")))
' > "$out/linux$tag.md"
  sed -n 5,6p "$out/linux$tag.md"
done
