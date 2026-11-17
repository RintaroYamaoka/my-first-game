#!/bin/bash
# Node.js を、遅い方式でも待たずに使う方法を比べる
export HOME=/root
ip link set lo up
t() { local s=$EPOCHREALTIME; "$@" > /tmp/out 2>&1; local rc=$?; printf '[bench] %-44s %6.2f s\n' "$L" "$(echo "$EPOCHREALTIME $s" | awk '{print $1-$2}')"; [ $rc = 0 ] || echo "[bench]   失敗(exit $rc): $(head -c 200 /tmp/out | tr '\n' ' ')"; }
ask() { exec 3<>/dev/tcp/127.0.0.1/7000; echo "$1" >&3; read -r r <&3; exec 3<&-; [ "$r" = ok ] || { echo "$r"; return 1; }; }
cd /root/sample
L="毎回起動: node src/log.ts";                 t node src/log.ts
L="毎回起動(2 回目)";                         t node src/log.ts
L="毎回起動 --jitless";                        t node --jitless src/log.ts
L="毎回起動 --max-lazy --no-opt";              t node --max-lazy --no-opt src/log.ts
s=$EPOCHREALTIME; node /etc/game/runner.mjs > /tmp/ready 2>&1 &
until grep -q ready /tmp/ready 2>/dev/null; do sleep 0.05; done
printf '[bench] %-44s %6.2f s\n' "常駐の起動(起動時に 1 回だけ)" "$(echo "$EPOCHREALTIME $s" | awk '{print $1-$2}')"
L="常駐に頼む: 同じプロセスで読み込む(1 回目)";  t ask "import /root/sample/src/log.ts"
L="常駐に頼む: 同じプロセスで読み込む(2 回目)";  t ask "import /root/sample/src/log.ts"
L="常駐に頼む: 別スレッドで実行(1 回目)";        t ask "worker /root/sample/src/log.ts"
L="常駐に頼む: 別スレッドで実行(2 回目)";        t ask "worker /root/sample/src/log.ts"
L="常駐に頼む: 別スレッドで実行(3 回目)";        t ask "worker /root/sample/src/log.ts"
