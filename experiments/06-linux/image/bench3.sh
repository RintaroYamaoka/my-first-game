#!/bin/bash
# Rust モード: コンパイルにかかる時間を測る(EXTRA="rust cargo" で組み立てたイメージで動かす)
export HOME=/root
t() { local s=$EPOCHREALTIME; "$@" > /tmp/out 2>&1; local rc=$?; printf '[bench] %-44s %6.2f s\n' "$L" "$(echo "$EPOCHREALTIME $s" | awk '{print $1-$2}')"; [ $rc = 0 ] || echo "[bench]   失敗(exit $rc): $(tail -c 300 /tmp/out | tr '\n' ' ')"; }
mkdir -p /root/rs && cd /root/rs
L="rustc --version";                            t rustc --version
L="cargo new hello";                            t cargo new -q hello
cd hello
L="cargo build(最初の 1 回)";                  t cargo build -q --offline
L="cargo run(作り直しなし)";                   t cargo run -q --offline
cat > src/main.rs <<'R'
use std::collections::HashMap;
#[derive(Debug)]
struct Fragment { id: String, body: String }
fn main() {
    let mut store: HashMap<String, Fragment> = HashMap::new();
    for i in 0..3 { let id = format!("f{i}"); store.insert(id.clone(), Fragment { id, body: "x".repeat(i) }); }
    let mut keys: Vec<_> = store.keys().cloned().collect(); keys.sort();
    println!("{keys:?} {:?}", store["f2"]);
}
R
L="cargo build(コードを書き換えたあと)";       t cargo build -q --offline
L="cargo run";                                  t cargo run -q --offline
echo "// 1 行だけ変える" >> src/main.rs
L="cargo build(1 行だけ変えたあと)";           t cargo build -q --offline
L="cargo check(型の確認だけ)";                 t cargo check -q --offline
echo "[bench] $(rustc --version)  メモリ: $(free -m | awk '/Mem/ {print $3 " MB 使用 / " $2 " MB"}')"
