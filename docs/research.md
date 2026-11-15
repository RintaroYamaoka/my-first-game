# 調べたこと

2026-11-15 時点。第三者サイトの記載を含むので、採用前に公式で確かめる。

## PC Building Simulator

- 個人開発者 Claudiu Kiss が itch.io に出した無料デモが始まり。2017 年に The Irregular Corporation が拾った
- 2018 年 3 月に Steam 早期アクセス、最初の 1 か月で 10 万本超。エンジンは Unity
- キャリアモード: 依頼 → 作業 → 現金と経験値 → レベルで新しいパーツと高報酬の仕事が解放。
  家賃と電気代が毎月かかる。後半は期限つきの仕事が増える
- 続編は、ジャンク PC を直して売る店頭販売と、作業台の切り替えを足した
- 評価: 学べる・くつろげる。フリービルドが魅力
- 批判: **キャリアが単調で同じ作業の繰り返し**(1作目・続編とも)

## 先行する作品とサービス

| 名前 | 内容 |
|---|---|
| Bitburner | JavaScript を書いて進めるハッカー RPG。ブラウザと Steam の両方 |
| SadServers | 壊れたサーバーを直す練習サイト。本物のクラウド VM を渡す |
| iximiuz Labs | Firecracker の小型 VM を自前サーバーで動かす学習サイト |
| Killercoda | シナリオを GitHub リポジトリで管理する学習環境 |
| Rustlings / 100 Exercises to Learn Rust | 手元のターミナルで解く演習 |

SadServers と Killercoda は、ミッションを「準備・依頼文・判定」のデータとして持つ。

## 本物の Linux を動かす方法

| 方式 | 例 | 特徴 |
|---|---|---|
| ブラウザ内で動かす | v86 / CheerpX / container2wasm | サーバー代なし。遅め。v86 は 64 ビット不可。CheerpX は商用有料 |
| サーバーで動かす | Vercel Sandbox / E2B | 本物そのもの。利用時間で課金 |
| プレイヤーの PC で動かす | QEMU を同梱 | サーバー代なし。動作の確認が要る |

- WebContainers は Node.js と WebAssembly だけ。営利の本番利用はライセンスが要る
- E2B: 1 vCPU あたり時間 約 0.05 ドル。Vercel Sandbox: CPU が動いた時間で課金

## 100-cord-knocks(本人の既存プロジェクト)から活かせること

場所: `~/dev/my-projects/rust-100-knocks`

- ミッションをデータで持ち、検証ハーネスで全件を確かめる形
- 判定の偽装対策(終了コードだけで正解にしない、区切りに実行ごとの乱数を使う、後勝ちで読む)
- 検証と本番で同じ環境・同じコマンド定義を使う

## Web の技術で作ったゲームを Steam に出す

- Electron に包む。Steam の機能は steamworks.js(Electron 向け)。Tauri は自前でつなぐ必要がある
- オーバーレイが Electron で動かない、Windows だけ、という報告がある(開発者の体験談)
