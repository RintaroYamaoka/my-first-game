#!/bin/sh
# 起動のたびに 1 回だけ動く
hostname world
arg() { tr ' ' '\n' < /proc/cmdline | sed -n "s/^$1=//p"; }
# 直列の線(シリアル)は画面の大きさを伝えないので、起動時の引数で受け取る
stty -F /dev/ttyS0 rows "$(arg rows || echo 24)" cols "$(arg cols || echo 80)" 2>/dev/null
echo "[boot] kernel -> ready: $(cut -d' ' -f1 /proc/uptime) s" > /dev/ttyS0
# 見張り番を、裏で動かす(2 本目の直列の線がなければ、すぐ終わる)
(setsid /etc/game/agent.sh > /dev/null 2>&1 &)
case "$(arg bench)" in
  1) /etc/game/bench.sh  > /dev/ttyS0 2>&1; poweroff -f ;;
  2) /etc/game/bench2.sh > /dev/ttyS0 2>&1; poweroff -f ;;
  3) /etc/game/bench3.sh > /dev/ttyS0 2>&1; poweroff -f ;;
  4) /etc/game/bench4.sh > /dev/ttyS0 2>&1; poweroff -f ;;
  5) /etc/game/bench5.sh > /dev/ttyS0 2>&1; poweroff -f ;;
esac
