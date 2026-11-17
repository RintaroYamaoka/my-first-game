# 同梱する小さな Linux を組み立てる(実験 5)。
#   出力: /out/vmlinuz(カーネル)、/out/initramfs.gz(中身ぜんぶ。メモリの上で動く)
# 元にするのは Alpine Linux。各部品のライセンスとソースの入手先が整理されている(docs/decisions/0002)
ARG ALPINE=3.24.4

# カーネル
FROM alpine:${ALPINE} AS kernel
RUN apk add --no-cache linux-virt

# 中身: シェル、git、Node.js(TypeScript をそのまま実行できる)、エディタ
FROM alpine:${ALPINE} AS rootfs
# EXTRA: 追加で入れる部品(Rust モードを測るときは "rust cargo")
ARG EXTRA=""
RUN apk add --no-cache bash git nodejs vim nano less ${EXTRA}
COPY init /init
COPY inittab /etc/inittab
COPY boot.sh bench.sh bench2.sh bench3.sh runner.mjs /etc/game/
COPY profile.sh /etc/profile.d/game.sh
RUN chmod +x /init /etc/game/*.sh && rm -rf /var/cache/apk/*
COPY sample /root/sample

# 中身を一つのファイルにまとめる
FROM alpine:${ALPINE} AS pack
RUN apk add --no-cache cpio
COPY --from=rootfs / /rootfs
COPY --from=kernel /boot/vmlinuz-virt /out/vmlinuz
RUN cd /rootfs && find . -xdev -path ./proc -prune -o -path ./sys -prune -o -path ./dev -prune -o -print \
      | cpio -o -H newc --quiet | gzip -1 > /out/initramfs.gz \
 && ls -la /out

# 測るための道具: QEMU(WSL 側での測定に使う。製品には、Windows 用の QEMU を同梱する)
FROM alpine:${ALPINE} AS bench
RUN apk add --no-cache qemu-system-x86_64 coreutils
COPY --from=pack /out /out
