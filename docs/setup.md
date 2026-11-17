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
cargo run -p world-from-code                          # WSL の窓で動かす(手早い確認用)
./scripts/run-windows.sh world-from-code <引数...>     # Windows 用に作って、Windows で動かす
```

- WSL の窓は、GPU を使わずソフトウェアで描く。動くことの確認に使い、速さは測らない
- 速さと滑らかさは、Windows で動かして見る。`run-windows.sh` は実行ファイルを
  `%LOCALAPPDATA%\my-first-game\dev\` に置いて起動する
- 2026-11-16 に確認: Windows で起動し、GPU(NVIDIA GeForce RTX 5070 Laptop、Vulkan)で描けた。WSL の窓と同じ絵になった
- Windows 用の最初のビルドは 10 分ほど。作り直しは 10 秒ほど

## Windows の保護機能との関係

- Windows のスマート アプリ コントロールが「オン」だと、自作の実行ファイルは起動を止められる
  (「組織の Device Guard ポリシーによってブロックされました」と出る)。署名がなく、Microsoft が安全と
  判断できないため。作り方を変えても同じ
- **この PC では、本人がオフにした(2026-11-16)。** PC は本人の個人のもの。開発をやめるときはオンに戻せる
- この設定は本人が決めて本人が操作する。Claude は触らない

### スマート アプリ コントロールについて調べたこと(2026-11-16)

- 2026 年 4 月の更新(26200.8116 以降)から、オフにしても Windows を入れ直さずにオンへ戻せる。この PC は 26200.9974
- 2026 年 10 月の更新で「オンに戻せない」不具合が出たが、11 月初めに修正された。切り替える前に Windows Update を最新にする
- Microsoft の案内でも、開発中はオフにするのが正式な方法の一つ。ほかに、評価モードで試す、署名する、開発用の仮想マシンを使う
- 切り替える場所: 設定 → プライバシーとセキュリティ → Windows セキュリティ → アプリとブラウザーの制御 →
  スマート アプリ コントロールの設定
- オフにしても、ウイルス対策と SmartScreen の警告は残る

出典: <https://blog-en.topedia.com/2026/04/smart-app-control-in-windows-11-can-now-be-re-enabled-without-reinstalling/>、
<https://bleepingcomputer.com/news/microsoft/microsoft-fixes-smart-app-control-broken-by-october-windows-update>、
<https://learn.microsoft.com/en-ca/answers/questions/5872161/smart-app-control-blocks-winforms-wpf-sample-appli>

## 決めていないこと

- **発売する実行ファイルの署名。** 買った人の PC でも同じ保護機能が働くので、段階 6 より前に調査して決める
- Windows 用のビルドは、今は WSL から作る方式(`x86_64-pc-windows-gnu`)。Steam の部品とつなぐ実験で
  うまくいかなければ、Windows に Microsoft のビルド道具を入れる方式に変える

## 同梱する Linux を組み立てる(実験 5 から)

- **Docker が要る**(`docker` が管理者権限なしで動くこと)。Linux の組み立て(`experiments/06-linux/build.sh`)と、WSL 側での QEMU の測定に使う。
  QEMU そのものは WSL に入れていない(測定用の Docker イメージの中にある)
- **Windows で動かすには、Windows 用の QEMU が要る**。本人が入れる: `winget install SoftwareFreedomConservancy.QEMU`
  (入る場所は `C:\Program Files\qemu\`。ゲームのターミナルは、既定でここを探す)
