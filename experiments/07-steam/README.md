# 実験 6(Steam との連携): Bevy の窓の中から、Steam の機能を呼べるか

問い(`docs/decisions/0002`): Steam との連携(実績、オーバーレイ)が Bevy で動くか。

## 仕組み

| 部分 | 担当 |
|---|---|
| Steam の機能を Rust から呼ぶ | `steamworks` 0.13.1(MIT OR Apache-2.0)。Valve の SDK の配布用ファイル(`steam_api64.dll`)を含む |
| 試す相手 | Valve が試験用に公開しているアプリ(Spacewar、番号 480)。自分のアプリの番号は、Steamworks に登録してから |

Bevy 用の部品(`bevy-steamworks`)は、対応する Bevy の版が合うか分からないので使わず、`steamworks` を直接呼んでいる。
必要なのは「毎コマ、Steam からの知らせを受け取る」1 行だけだった。

## 動かし方

Steam を起動して、ログインした状態で:

```sh
cargo build -p steam --target x86_64-pc-windows-gnu
# steam.exe と steam_api64.dll を Windows 側の同じフォルダに置いて、steam.exe を起動する
```

**起動中は、Steam の上で「Spacewar をプレイ中」と表示される(フレンドにも見える)。**

| 操作 | 動き |
|---|---|
| O / Shift+Tab | オーバーレイを開く |
| A | Spacewar の実績を 1 つ解除する(自分の Steam のアカウントに記録される) |
| C | その実績を取り消す |

## 分かったこと

- Windows 用にビルドできた(`x86_64-pc-windows-gnu`)。
- **Steam につながった**(2026-11-18、本人の了解を得て、Windows で起動)。自動で読んだ状態:
  つながった / ログイン中 / オーバーレイは有効 / 実績の状態を読めた(未解除)。
- オーバーレイが実際に画面に出るか: 本人が操作して「出来た 多分」(2026-11-18)。はっきりした確認ではないので、段階 2 で自分のアプリの番号を取ったあとに、もう一度確かめる。
- 実績の解除の通知は、試していない。
- `steam.exe` の隣に `steam_api64.dll` を置く必要がある(`steamworks-sys` の中の `lib/steam/redistributable_bin/win64/`)。

## 守ること

- Steam の SDK は、GPL などのコピーレフトと両立しない(`docs/decisions/0002`)。ゲーム本体に GPL の部品を入れない。
  `scripts/licenses.sh` の「要確認」で確かめる。
- Steam の鍵や、発売用の署名の情報を、リポジトリに入れない。
