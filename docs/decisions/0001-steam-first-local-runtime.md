# 0001 — Steam での配信を最初から目指し、本物の環境はプレイヤーの PC で動かす

- **Status**: Accepted(技術の中身は未検証。下の「未検証」を参照)
- **Date**: 2026-11-15
- **Deciders**: RintaroYamaoka (本人決定) / Claude

## Context (背景)

- 当初は「Web で公開し、育ったら Steam へ」だったが、本人が**最初から Steam での配信を目指す**と決めた
- コンセプト(`docs/concept.md`)の条件は「作業する瞬間は本物の Linux」
- 見た目の試作(`prototypes/dive/`)は Three.js で作り、本人が方向を確定した

## Decision (決定)

1. **配信先は Steam。** Web の技術で作り、デスクトップアプリの枠に包んで出す
2. **画面は TypeScript + Three.js。** 3D の世界が主役のため
3. **枠は Electron を第一候補にする。** Steam の機能をつなぐ steamworks.js が Electron 向けのため
4. **本物の環境は、ゲームに同梱した小さな Linux をプレイヤーの PC 上で動かす**

### 4 の理由

Steam は買い切りが基本。サーバー側のサンドボックス(Vercel Sandbox など)は、こちらが実行費用を
払い続けるので、売れるほど費用が増える。PC の中で動かせば、サーバー代も悪用対策も要らず、
オフラインで遊べる。

## 採用しなかった案

- **Vercel Sandbox(100-cord-knocks の基盤)**: 上の費用の理由。判定の考え方(隠しテスト、
  偽装対策)は引き継ぐ
- **Rust + Leptos(100-cord-knocks と同じ画面構成)**: Three.js を Rust から使うのは回り道
- **Unity / Godot**: 本物のターミナル画面をゲーム内に映すのが難しい。試作の資産も使えない
- **プレイヤーに Docker を入れてもらう**: 一般のプレイヤーに求められない
- **WebContainers / CheerpX**: 営利利用に有料ライセンスが要る。WebContainers は Linux が動かない

## 未検証(次の試作で確かめる)

- 同梱する Linux を Windows 上で、速く・確実に動かせるか。候補は QEMU と v86
  - v86 は 64 ビットのカーネルが動かない
  - コンテナの中にコンテナを入れた多層構造が、その上で動くか
- 同梱する部品のライセンスが、有料配信で問題ないか
- 本物の `ls` や `ps` の出力から、世界を組み立てられるか
- Electron で Steam のオーバーレイが動くか(動かない、Windows だけ、という報告がある)

## Steam の手続き(2026-11-15 に調べた内容)

- 1作品 100 ドル。返金なし、売上 1,000 ドル超で戻る
- 料金を払ってから発売まで 21 日。「近日登場」のストアページを 2 週間以上公開する
- ストアページと本体は別々に審査。ストアページの審査は通常 3〜5 営業日

出典: Steamworks Documentation(appfee / releasing / review_process)
