#!/bin/bash
# 中身の置き場所(メモリ / ディスク)を比べるための測定: メモリと、最初の 1 回・2 回目の速さ
now() { echo "${EPOCHREALTIME/./}"; }
t() { local label="$1"; shift; local s; s=$(now); "$@" > /dev/null 2>&1; local us=$(( $(now) - s )); printf '%-34s %3d.%02d s\n' "$label" $((us / 1000000)) $((us % 1000000 / 10000)); }
mem() { awk -v l="$1" '/MemTotal/{t=$2}/MemAvailable/{a=$2}END{printf "%-34s %4d MB\n", l, (t-a)/1024}' /proc/meminfo; }
echo "root: $(awk '$2=="/"{print $3}' /proc/mounts | tail -1)"
mem "使っているメモリ(起動直後)"
cd /root/sample
t "node(最初の 1 回)" node -e 'console.log(1+1)'
t "node(2 回目)" node -e 'console.log(1+1)'
t "git status(最初の 1 回)" git status
t "git status(2 回目)" git status
command -v cargo > /dev/null && { cd /tmp && cargo new -q hello 2>/dev/null; cd hello; t "cargo build(最初の 1 回)" cargo build; echo '// x' >> src/main.rs; t "cargo build(書き換えたあと)" cargo build; }
mem "使っているメモリ(動かしたあと)"
