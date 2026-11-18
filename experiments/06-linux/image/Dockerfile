# 同梱する小さな Linux を組み立てる(実験 5)。
#   出力: /out/vmlinuz(カーネル)、/out/initramfs.gz(中身ぜんぶ。メモリの上で動く)
# 元にするのは Alpine Linux。各部品のライセンスとソースの入手先が整理されている(docs/decisions/0002)
ARG ALPINE=3.24.4
# ROOT: どの中身を詰めるか(rootfs = いつもの / rootfs-layers = コンテナを動かす道具つき)
ARG ROOT=rootfs

# カーネル
FROM alpine:${ALPINE} AS kernel
RUN apk add --no-cache linux-virt

# 中身: シェル、git、Node.js(TypeScript をそのまま実行できる)、エディタ
FROM alpine:${ALPINE} AS rootfs
# EXTRA: 追加で入れる部品(Rust モードを測るときは "rust cargo")
ARG EXTRA=""
RUN apk add --no-cache bash git nodejs vim nano less inotify-tools ${EXTRA}
COPY init /init
COPY inittab /etc/inittab
COPY boot.sh agent.sh bench.sh bench2.sh bench3.sh bench4.sh runner.mjs /etc/game/
COPY profile.sh /etc/profile.d/game.sh
RUN chmod +x /init /etc/game/*.sh && rm -rf /var/cache/apk/*
COPY sample /root/sample

# コンテナを動かす道具つき(多層構造を確かめる)。Docker と、カーネルの部品(重ねるファイルシステム、仮想のネットワークなど)
# コンテナの元(イメージ)にする小さな中身。外から取ってこられないので、組み立てるときに用意して、中に入れておく
FROM alpine:${ALPINE} AS box
RUN apk add --no-cache bash nodejs && rm -rf /var/cache/apk/*

FROM rootfs AS rootfs-layers
RUN apk add --no-cache docker && rm -rf /var/cache/apk/*
RUN --mount=type=bind,from=box,source=/,target=/box tar -C /box -cf /etc/game/box.tar bin etc home lib root sbin usr var tmp
COPY --from=kernel /lib/modules /lib/modules

FROM ${ROOT} AS chosen

# 中身を一つのファイルにまとめる
FROM alpine:${ALPINE} AS pack
RUN apk add --no-cache cpio
COPY --from=chosen / /rootfs
COPY --from=kernel /boot/vmlinuz-virt /out/vmlinuz
RUN cd /rootfs && find . -xdev -path ./proc -prune -o -path ./sys -prune -o -path ./dev -prune -o -print \
      | cpio -o -H newc --quiet | gzip -1 > /out/initramfs.gz \
 && ls -la /out

# 測るための道具: QEMU(WSL 側での測定に使う。製品には、Windows 用の QEMU を同梱する)
FROM alpine:${ALPINE} AS bench
RUN apk add --no-cache qemu-system-x86_64 coreutils
COPY --from=pack /out /out
