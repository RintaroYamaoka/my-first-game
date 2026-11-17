#!/bin/bash
# よく使う操作にかかる時間を測る
export HOME=/root
t() { local s=$EPOCHREALTIME; "$@" > /tmp/out 2>&1; local rc=$?; printf '[bench] %-34s %6.2f s\n' "$L" "$(echo "$EPOCHREALTIME $s" | awk '{print $1-$2}')"; [ $rc = 0 ] || { echo "[bench]   失敗(exit $rc): $(head -c 300 /tmp/out | tr '\n' ' ')"; }; }
cd /root/sample
L="ls -la";                          t ls -la
L="bash 起動";                        t bash -c true
L="node 起動(console.log)";          t node -e 'console.log(1)'
L="node 起動 2 回目";                 t node -e 'console.log(1)'
L="TypeScript を実行(src/log.ts)";   t node src/log.ts
L="TypeScript(import あり server.ts)"; t node --check src/server.ts
L="git init";                        t git init -q
git config --global user.email a@b; git config --global user.name a
L="git add .";                       t git add .
L="git commit";                      t git commit -q -m first
L="git log";                         t git log --oneline
L="grep -r(全ファイル)";              t grep -r import .
echo "[bench] メモリ: $(free -m | awk '/Mem/ {print $3 " MB 使用 / " $2 " MB"}')"
echo "[bench] node $(node --version)  git $(git --version | cut -d' ' -f3)  kernel $(uname -r)"
