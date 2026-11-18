# 同梱の Linux に入っている部品(コンテナを動かす道具つき)

`scripts/licenses.sh` が作る。手で書き換えない。元は Alpine Linux の管理簿。

- 部品の数: 62
- GPL / LGPL の部品: 27 個。ゲーム本体とは別のプログラムとして同梱し、ソースコードの入手先とライセンス文を添える

カーネル(linux-virt、GPL-2.0-only)は、この一覧とは別に同梱する。

| 部品 | 版 | ライセンス | 元になるパッケージ | 入手先 |
|---|---|---|---|---|
| ada-libs | 4.0.0-r0 | ( Apache-2.0 OR MIT ) AND MPL-2.0 | ada | https://ada-url.github.io/ada |
| alpine-baselayout | 3.7.2-r1 | GPL-2.0-only | alpine-baselayout | https://gitlab.alpinelinux.org/alpine/aports/-/tree/master/main/alpine-baselayout |
| alpine-baselayout-data | 3.7.2-r1 | GPL-2.0-only | alpine-baselayout | https://gitlab.alpinelinux.org/alpine/aports/-/tree/master/main/alpine-baselayout |
| alpine-keys | 2.6-r0 | MIT | alpine-keys | https://alpinelinux.org |
| alpine-release | 3.24.4-r0 | MIT | alpine-base | https://alpinelinux.org |
| apk-tools | 3.0.9-r0 | GPL-2.0-only | apk-tools | https://gitlab.alpinelinux.org/alpine/apk-tools |
| bash | 5.3.9-r1 | GPL-3.0-or-later | bash | https://www.gnu.org/software/bash/bash.html |
| brotli-libs | 1.2.1-r0 | MIT | brotli | https://github.com/google/brotli |
| busybox | 1.37.0-r31 | GPL-2.0-only | busybox | https://busybox.net/ |
| busybox-binsh | 1.37.0-r31 | GPL-2.0-only | busybox | https://busybox.net/ |
| c-ares | 1.34.9-r0 | MIT | c-ares | https://c-ares.org/ |
| ca-certificates | 20261020-r0 | MPL-2.0 AND MIT | ca-certificates | https://www.mozilla.org/en-US/about/governance/policies/security-group/certs/ |
| ca-certificates-bundle | 20261020-r0 | MPL-2.0 AND MIT | ca-certificates | https://www.mozilla.org/en-US/about/governance/policies/security-group/certs/ |
| containerd | 2.3.7-r0 | Apache-2.0 | containerd | https://containerd.io/ |
| docker | 29.9.3-r1 | Apache-2.0 | docker | https://www.docker.io/ |
| docker-cli | 29.9.3-r1 | Apache-2.0 | docker | https://www.docker.io/ |
| docker-cli-buildx | 0.38.1-r0 | Apache-2.0 | docker-cli-buildx | https://docs.docker.com/engine/reference/commandline/buildx_build |
| docker-engine | 29.9.3-r1 | Apache-2.0 | docker | https://www.docker.io/ |
| git | 2.54.1-r0 | GPL-2.0-only | git | https://www.git-scm.com/ |
| git-init-template | 2.54.1-r0 | GPL-2.0-only | git | https://www.git-scm.com/ |
| icu-data-en | 78.1-r0 | ICU | icu | https://icu.unicode.org/ |
| icu-libs | 78.1-r0 | ICU | icu | https://icu.unicode.org/ |
| inotify-tools | 4.23.9.0-r0 | GPL-2.0-only | inotify-tools | https://github.com/inotify-tools/inotify-tools |
| inotify-tools-libs | 4.23.9.0-r0 | GPL-2.0-only | inotify-tools | https://github.com/inotify-tools/inotify-tools |
| iptables | 1.8.13-r0 | GPL-2.0-or-later | iptables | https://www.netfilter.org/projects/iptables/index.html |
| less | 702-r0 | GPL-3.0-or-later OR BSD-2-Clause | less | https://www.greenwoodsoftware.com/less/ |
| libapk | 3.0.9-r0 | GPL-2.0-only | apk-tools | https://gitlab.alpinelinux.org/alpine/apk-tools |
| libcrypto3 | 3.5.9-r0 | Apache-2.0 | openssl | https://www.openssl.org/ |
| libcurl | 8.23.0-r0 | curl | curl | https://curl.se/ |
| libexpat | 2.10.0-r0 | MIT | expat | https://libexpat.github.io/ |
| libgcc | 15.2.0-r5 | GPL-2.0-or-later AND LGPL-2.1-or-later | gcc | https://gcc.gnu.org |
| libidn2 | 2.3.8-r0 | GPL-2.0-or-later OR LGPL-3.0-or-later | libidn2 | https://www.gnu.org/software/libidn#libidn2 |
| libmnl | 1.0.5-r2 | LGPL-2.1-or-later | libmnl | https://www.netfilter.org/projects/libmnl/ |
| libncursesw | 6.6_p20260516-r0 | X11 | ncurses | https://invisible-island.net/ncurses/ |
| libnftnl | 1.3.1-r0 | GPL-2.0-or-later | libnftnl | https://netfilter.org/projects/libnftnl |
| libpsl | 0.21.5-r3 | MIT | libpsl | https://rockdaboot.github.io/libpsl |
| libseccomp | 2.6.1-r0 | LGPL-2.1-or-later | libseccomp | https://github.com/seccomp/libseccomp |
| libssl3 | 3.5.9-r0 | Apache-2.0 | openssl | https://www.openssl.org/ |
| libstdc++ | 15.2.0-r5 | GPL-2.0-or-later AND LGPL-2.1-or-later | gcc | https://gcc.gnu.org |
| libunistring | 1.4.2-r0 | GPL-2.0-or-later OR LGPL-3.0-or-later | libunistring | https://www.gnu.org/software/libunistring/ |
| libxtables | 1.8.13-r0 | GPL-2.0-or-later | iptables | https://www.netfilter.org/projects/iptables/index.html |
| musl | 1.2.6-r3 | MIT | musl | https://musl.libc.org/ |
| musl-utils | 1.2.6-r3 | MIT AND BSD-2-Clause AND GPL-2.0-or-later | musl | https://musl.libc.org/ |
| nano | 9.3-r0 | GPL-3.0-or-later | nano | https://www.nano-editor.org/ |
| ncurses-terminfo-base | 6.6_p20260516-r0 | X11 | ncurses | https://invisible-island.net/ncurses/ |
| nghttp2-libs | 1.70.0-r0 | MIT | nghttp2 | https://nghttp2.org |
| nodejs | 24.22.3-r0 | MIT | nodejs | https://nodejs.org/ |
| pcre2 | 10.49-r0 | BSD-3-Clause | pcre2 | https://pcre.org/ |
| pixman | 0.46.4-r0 | MIT | pixman | https://gitlab.freedesktop.org/pixman |
| readline | 8.3.3-r1 | GPL-3.0-or-later | readline | https://tiswww.cwru.edu/php/chet/readline/rltop.html |
| runc | 1.4.4-r0 | Apache-2.0 | runc | https://www.opencontainers.org |
| scanelf | 1.3.9-r1 | GPL-2.0-only | pax-utils | https://wiki.gentoo.org/wiki/Hardened/PaX_Utilities |
| simdjson | 4.2.4-r0 | Apache-2.0 OR MIT | simdjson | https://simdjson.org/ |
| simdutf | 9.0.0-r0 | Apache-2.0 OR MIT | simdutf | https://simdutf.github.io/simdutf/ |
| sqlite-libs | 3.53.4-r0 | blessing | sqlite | https://www.sqlite.org/ |
| ssl_client | 1.37.0-r31 | GPL-2.0-only | busybox | https://busybox.net/ |
| tini-static | 0.19.0-r3 | MIT | tini | https://github.com/krallin/tini |
| vim | 9.2.1352-r0 | Vim | vim | https://www.vim.org/ |
| vim-common | 9.2.1352-r0 | Vim | vim | https://www.vim.org/ |
| xxd | 9.2.1352-r0 | Vim | vim | https://www.vim.org/ |
| zlib | 1.3.2-r1 | Zlib | zlib | https://zlib.net/ |
| zstd-libs | 1.5.7-r2 | BSD-3-Clause OR GPL-2.0-or-later | zstd | https://facebook.github.io/zstd/ |
