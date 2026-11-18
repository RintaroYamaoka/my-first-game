#!/usr/bin/env bash
# 同梱する Linux を組み立てて、target/linux-image/ に置く。Docker が要る
#   ./experiments/06-linux/build.sh
#   EXTRA="rust cargo" TAG=-rust ./experiments/06-linux/build.sh   # Rust 入り(target/linux-image-rust/)
#   ROOT=rootfs-layers TAG=-layers ./experiments/06-linux/build.sh   # コンテナを動かす道具つき(target/linux-image-layers/)
#   DISK=1 を足すと、中身をディスクに置く形も作る(target/linux-image<TAG>-disk/)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$(cd "$here/../.." && pwd)"
out="$root/target/linux-image${TAG:-}"
# 見本のプロジェクト(node_modules を除く)を、Linux の中に入れる
rm -rf "$here/image/sample" && mkdir -p "$here/image/sample"
( cd "$root/experiments/01-world-from-code/sample" && tar cf - --exclude node_modules . ) | tar xf - -C "$here/image/sample"
docker build -q --target pack  -t my-first-game-linux:pack${TAG:-}  --build-arg EXTRA="${EXTRA:-}" --build-arg ROOT="${ROOT:-rootfs}" "$here/image" > /dev/null
docker build -q --target bench -t my-first-game-linux:bench${TAG:-} --build-arg EXTRA="${EXTRA:-}" --build-arg ROOT="${ROOT:-rootfs}" "$here/image" > /dev/null
[ -z "${DISK:-}" ] || docker build -q --target disk -t my-first-game-linux:disk${TAG:-} --build-arg EXTRA="${EXTRA:-}" --build-arg ROOT="${ROOT:-rootfs}" "$here/image" > /dev/null
rm -rf "$here/image/sample"
mkdir -p "$out"
id=$(docker create my-first-game-linux:pack${TAG:-} sh)
docker cp -q "$id:/out/vmlinuz" "$out/vmlinuz"; docker cp -q "$id:/out/initramfs.gz" "$out/initramfs.gz"
docker rm "$id" > /dev/null
ls -la "$out"
# DISK=1: 中身をディスクに置く形も作る(target/linux-image<TAG>-disk/)
if [ -n "${DISK:-}" ]; then
  mkdir -p "$out-disk"
  id=$(docker create my-first-game-linux:disk${TAG:-} sh)
  for f in vmlinuz initramfs.gz root.squashfs; do docker cp -q "$id:/out/$f" "$out-disk/$f"; done
  docker rm "$id" > /dev/null
  ls -la "$out-disk"
fi
