#!/bin/bash
# 多層構造を確かめる: Linux の中でコンテナを動かし、その中でさらにコンテナを動かす(rootfs-layers の中身で動かす)
now() { echo "${EPOCHREALTIME/./}"; }
t() { # t <名前> <コマンド…>: 時間を測って 1 行で出す
  local label="$1"; shift
  local s; s=$(now); "$@" > /tmp/out 2>&1; local rc=$?; local us=$(( $(now) - s ))
  printf '%-46s %4d.%02d s  %s\n' "$label" $((us / 1000000)) $((us % 1000000 / 10000)) "$([ $rc = 0 ] && echo ok || echo "失敗($rc): $(tail -12 /tmp/out | tr '\n' ' ')")"
}
wait_docker() { for _ in $(seq 200); do "$@" docker info > /dev/null 2>&1 && return 0; sleep 0.1; done; return 1; }
# 組み立てるときに入れておいた小さな中身(box.tar)を、コンテナの元(イメージ)にする。外から取ってこない
box_image() { "$@" docker import - box < /etc/game/box.tar > /dev/null; }

echo "== 準備 =="
mount -t cgroup2 none /sys/fs/cgroup
mkdir -p /var/lib/docker /run && mount -t tmpfs none /var/lib/docker
# 中身がメモリの上(initramfs)にあるので、Docker に「根を入れ替える方法を変えて」と伝える
export DOCKER_RAMDISK=true
dockerd > /var/log/dockerd.log 2>&1 &
t "dockerd が使えるようになるまで" wait_docker
docker info 2>/dev/null | grep -E "Storage Driver|Cgroup|Server Version" | sed 's/^ */  /'
t "小さな中身をイメージにする" box_image
echo "  イメージ: $(docker images --format '{{.Repository}} {{.Size}}' | tr '\n' ' ')"

echo "== 2 層目(Linux の中のコンテナ)=="
t "docker run(コンテナを起こす)" docker run -d --name server --hostname server box sleep 100000
t "docker exec … hostname" docker exec server hostname; echo "  → $(cat /tmp/out)"
t "docker exec … ls /" docker exec server ls /
t "docker exec … node -e" docker exec server node -e 'console.log(1+1)'
t "docker exec … bash -c(何もしない)" docker exec server bash -c true

echo "== 3 層目(コンテナの中のコンテナ)=="
t "docker run --privileged(中で dockerd)" docker run -d --privileged --name outer --hostname outer --tmpfs /var/lib/docker --tmpfs /run -e DOCKER_RAMDISK=true -v /usr:/usr:ro -v /lib:/lib:ro -v /bin:/bin:ro -v /sbin:/sbin:ro -v /etc/game:/etc/game:ro box dockerd
t "中の dockerd が使えるようになるまで" wait_docker docker exec outer
t "中で、イメージを作る" box_image docker exec -i outer
t "中で docker run" docker exec outer docker run -d --name inner --hostname inner box sleep 100000
t "外から 2 段で入る(exec → exec)hostname" docker exec outer docker exec inner hostname; echo "  → $(cat /tmp/out)"
t "3 層目で node -e" docker exec outer docker exec inner node -e 'console.log(1+1)'

echo "== メモリ =="
free -m | sed 's/^/  /'
echo "== dockerd の記録(最後の 5 行)=="
tail -5 /var/log/dockerd.log | cut -c1-200
