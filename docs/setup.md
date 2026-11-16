# 開発環境

最終更新: 2026-11-16

開発は WSL2(Ubuntu)の中で行う。ゲームは Windows 用に作る。

## 整え方

```
./scripts/setup-dev.sh     # 整える。何度実行してもよい
./scripts/check-env.sh     # 点検する。何も変更しない
```

`setup-dev.sh` が管理者権限(sudo)を使うのは、OS の部品を入れる 1 か所だけ。パスワードを聞かれるので、
普段のターミナルで実行する(Claude Code の画面からは入力できない)。

リポジトリの外に置くものは一つだけ: WSL のとき、音の既定の出力先を WSLg に向ける `~/.asoundrc`
(既にあれば変更しない)。

## 何がどこで決まっているか

| 決めていること | 場所 |
|---|---|
| OS の部品の一覧と、それぞれの用途 | `scripts/packages.txt` |
| Rust の版、Windows 用の部品 | `rust-toolchain.toml`(このフォルダで `cargo` を使うと自動で入る) |
| Bevy の版、ビルドの設定 | `Cargo.toml`(作業場。実験とゲーム本体で共有する) |
| 依存する部品の正確な版 | `Cargo.lock`(git に入れる) |

版を上げるのは、段階の切れ目だけ(`decisions/0002`)。

## 動かし方

```
cargo run -p world-from-code                 # WSL の窓で動かす(確認用)
cargo build -p world-from-code --target x86_64-pc-windows-gnu   # Windows 用に作る
```

- WSL の窓は、GPU を使わずソフトウェアで描く。動くことの確認に使い、速さは測らない
- 速さと滑らかさは、Windows 用に作った実行ファイルを Windows で動かして測る

## 分かっている制約

- **この PC の Windows は、署名のない実行ファイルの起動を止める**(2026-11-16 に確認)。WSL で作った
  Windows 用の実行ファイルを起動すると「組織の Device Guard ポリシーによってブロックされました」と出る。
  Windows の保護機能(スマート アプリ コントロール、または管理者が入れたアプリ制御)によるもので、
  回避はしない。どう扱うかは本人が決める(下の「決めていないこと」)
- そのため、Windows での速さと滑らかさの計測は、まだできていない

## 決めていないこと

- **Windows 用の実行ファイルを、この PC でどう動かすか。** 候補: 保護機能の設定を本人が確認して見直す /
  別の Windows の PC で動かす / 実行ファイルに署名する。Steam で配る実行ファイルも、いずれ同じ種類の
  保護機能に当たるので、署名は発売までに必ず決める必要がある
- Windows 用のビルドは、今は WSL から作る方式(`x86_64-pc-windows-gnu`)。Steam の部品とつなぐ実験で
  うまくいかなければ、Windows に Microsoft のビルド道具を入れる方式に変える
