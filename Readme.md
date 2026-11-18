# Susuki

Susuki は、放送録画ファイル (MPEG-TS / MXF / MKV / MP4 / OGG、TLV/MMT 対応) から
CM 区間を推定するコマンドラインツールです。旧運用チェーン
(m2ts-tv-trim → chapter_exe / logoframe / join_logo_scp) の判定部を、
FFmpeg をデコード基盤として単一バイナリに統合・再実装したものです。
判定ロジックは設計規範书および参照実装の固定 commit に厳密に準拠しています
(`--help` 出力と挙動が規範、本 README は概要のみ)。

## インストール (Debian / Ubuntu)

[リリース](https://github.com/MistVVK/susuki/releases)に、対応ディストリビューションごとの deb パッケージ (amd64) を置いています。
使っているディストリビューションに合うものを選んでください。

| ディストリビューション | パッケージ名の末尾 |
|------------------------|--------------------|
| Ubuntu 22.04 LTS | `.jammy_amd64.deb` |
| Ubuntu 24.04 LTS | `.noble_amd64.deb` |
| Ubuntu 26.04 LTS | `.resolute_amd64.deb` |
| Debian 13 | `.trixie_amd64.deb` |

ダウンロードしたファイルを `apt` でインストールします (例は Ubuntu 24.04)。
パス指定の `./` を付けると、必要な依存パッケージ (libva など) も一緒に入ります。

```sh
sudo apt install ./susuki_<バージョン>-1.noble_amd64.deb
susuki --help
```

- `/usr/bin/susuki` から起動できます。本体は `/usr/lib/susuki/` にあり、専用 FFmpeg を
  `/usr/lib/susuki/lib/` に同梱しています。システムの FFmpeg は使わず、競合もしません。
- GPU 解析 (`--analysis-backend auto|gpu`) には Vulkan ローダ (`libvulkan1`、推奨依存として
  自動で入ります) と GPU ドライバが必要です。使えない場合は CPU で解析します。
- GPU デコード (`--backend vaapi|nvdec`) には、それぞれ VA-API ドライバまたは NVIDIA
  ドライバが別途必要です。
- アンインストールは `sudo apt remove susuki` です。

上記以外のディストリビューションやアーキテクチャでは、次の「ビルド」に従ってソースから
ビルドしてください。

## インストール (Windows)

[リリース](https://github.com/MistVVK/susuki/releases)の `susuki-<バージョン>-windows-x86_64.7z` (Windows 10 以降の x64) を任意の
フォルダに展開し (7-Zip などで展開できます)、その中の `susuki.exe` を実行します。インストーラはありません。

```bat
susuki.exe --help
```

- 専用 FFmpeg の DLL を `susuki.exe` と同じフォルダに同梱しています。DLL は exe と
  同じフォルダに置いたままにしてください。
- GPU デコードは `--backend d3d11va` (Intel / AMD / NVIDIA 共通) です。`nvdec` と
  `vaapi` は Windows 版にはありません。`--device` には DXGI アダプタ番号 (0 始まり)
  を指定します。
- GPU 解析 (`--analysis-backend auto|gpu`) には Vulkan 対応の GPU ドライバが必要です。
  使えない場合は CPU で解析します。
- Ctrl+C で中断できます (終了コード 130)。ただし `--output-stdout` の出力先が読み取りを
  止めてパイプが詰まっている間の書き込みは中断できません。

## ビルド

FFmpeg のビルドと Rust のビルドは分離されています (設計 §10.4)。

```sh
# 1) 専用 FFmpeg (LGPL 構成・TLV パッチ適用済み) を dist/ に構築
#    ffmpeg.lock / vendor/patches の SHA256 を検証してからビルドする
./scripts/ffmpeg-build.sh

# 2) Rust ワークスペース (susuki CLI と各 crate)
SUSUKI_FFMPEG_PREFIX=/path/to/ffmpeg-prefix cargo build --release
```

実行時、バイナリは `RPATH=$ORIGIN/lib` で同梱共有ライブラリを参照します
(アーカイブ配布想定)。ソースツリーから直接実行する場合は
`LD_LIBRARY_PATH=<ffmpeg-prefix>/lib` を指定してください。

Release ビルドは `crates/susuki/build.rs` が埋め込み標準スクリプト
(JL_標準.txt) の SHA256 を検証し、REF-JLS 固定 commit とバイト一致しない
場合はビルドが失敗します。

### deb パッケージの作成

リリース用の deb は `scripts/package-deb.sh` で作ります。対応ディストリビューションごとに
使い捨ての rootless Podman コンテナを起動し、その中で専用 FFmpeg と susuki をビルドします。
コミット済みの HEAD が対象で、未コミットの変更があると止まります。
コンテナからネットワーク (apt ミラー・GitHub・crates.io・rustup) に接続できる必要があります。

```sh
./scripts/package-deb.sh                # 4 ディストリビューションすべて
./scripts/package-deb.sh jammy trixie   # 一部だけ
```

成果物は `dist/deb/<ディストリビューション>/` に、deb・`build.log`・`build-info.txt`
(ソースの SHA とツールのバージョン) として出力されます。

### Windows 版の作成 (クロスコンパイル)

Windows 版は Linux 上で MinGW-w64 を使ってクロスコンパイルします (Rust のターゲットは
`x86_64-pc-windows-gnu`)。`scripts/package-windows.sh` が使い捨ての rootless Podman
コンテナ (Debian 13) を起動し、その中で専用 FFmpeg の DLL と `susuki.exe` をビルドして、
Wine で起動確認をしてから 7z (圧縮方式 BZip2) にまとめます。deb と同様に、コミット済みの HEAD が
対象で、コンテナからネットワークに接続できる必要があります。

```sh
./scripts/package-windows.sh
```

成果物は `dist/windows/` に、7z・`build.log`・`build-info.txt` として出力されます。
手動でビルドする場合は、`TARGET_OS=windows` で専用 FFmpeg を構築してから、
ターゲットを指定して cargo を実行します。

```sh
TARGET_OS=windows PREFIX="$PWD/dist/ffmpeg-windows" ./scripts/ffmpeg-build.sh
SUSUKI_FFMPEG_PREFIX="$PWD/dist/ffmpeg-windows" \
    cargo build --release --target x86_64-pc-windows-gnu
```

Windows 用の専用 FFmpeg は、GPU デコードを D3D11VA のみにし、zlib・bzip2・xz を
`ffmpeg.lock` の固定 commit から静的に組み込みます。

### リリース

リリースの手順は `.agents/skills/release/SKILL.md` にまとめています。配布物をビルドしたあと、
次のスクリプトで公開します (`main` ブランチで実行します)。

```sh
./scripts/release-assets.sh        # 配布物を dist/release/v<バージョン>/ にまとめ、同じコミットからのビルドか検査
./scripts/release-push.sh          # タグを作り、公開するコミットを検査 (--push で main とタグを push)
./scripts/release.sh               # gh で GitHub Releases に公開
```

## 使い方

```sh
susuki INPUT.ts --logo logo.lgd --output-json out.json --output-chapter out.chapter.txt
susuki INPUT.ts --no-logo --output-stdout json
```

- 入力動画は位置引数 1 個。
- 候補ロゴは `--logo` (1 個以上、AviUtl v0.1 または Amatsukaze 拡張 v1
  のロゴファイル) で指定する。すべての候補を同じデコード済み輝度で検出し、
  統合した表示区間を判定器へ渡す。ロゴ検出を省略する場合は `--no-logo`。
- 無音判定は選択した1本の音声を元サンプルレートで解析し、全チャンネルの
  全サンプルの s16 換算最大振幅を `--mute-threshold` と比較する。
  ダウンミックスやリサンプルは行わない。
- 可変フレームレートの映像は、先頭映像の表示時刻を原点とする公称 fps の格子へ、
  実際の表示時刻に応じて複製・間引きして解析する。音声とロゴも同じ実時間軸に揃う。
  FFmpeg が PTS と DTS から選んだ表示時刻を優先する。JSON の `input.total_frames` と
  各 `*_frame` はこの解析格子の番号であり、可変フレームレート入力の元フレーム番号ではない。
  秒数は先頭映像基準で、格子への丸め誤差を含む。コンテナの開始時刻との関係は
  `format_start_us` と `video_start_us` で確認する。表示時刻の欠落・逆行・重複や
  5 秒超の間隔は拒否する。
- `--tolerate-audio-decode-errors` は破損した音声 packet を読み飛ばす。欠落区間は
  無音と判定せず、映像やメモリ不足などの異常は引き続き失敗する。読み飛ばしがあれば
  JSON の `warnings` に `decode_errors_skipped` を記録する。
- `--tolerate-video-gaps` は 5 秒超 30 秒以内の映像欠落を不明として扱う。
  欠落中を静止画やロゴなしとは判定せず、欠落に触れる CM 候補は全体を本編として保護する。
  JSON の `analysis.missing_video_ranges` と `video_frames_missing` を確認する。
  保護は `result.keep_ranges` と chapter に反映する。`result.segments` と div は保護前の判定情報。
  このオプション単独では逆行・重複を許容しない。30 秒超の欠落も失敗する。
- --tolerate-video-timestamp-errors は復号後の短い表示時刻の逆行を最大 8 フレーム先まで並べ替える。
  同じ時刻の複数画像は正しい画像を確定できないため、その期間を不明として保護する。
  不明範囲と警告の出力は映像欠落と同じ。窓より深い逆行や時刻のない映像は失敗する。
- 色記述だけが変化した場合、各フレームの native luma で解析し、境界のフレーム同士を
  比較しない。結果には `video_color_configuration_changed` を記録する。ロゴの判定は
  色記述が変化した区間でも同じ候補ロゴを使うため、結果の確認が必要。
- 出力は `--output-json` / `--output-chapter` / `--output-div` /
  `--output-stdout json|chapter` のいずれか 1 つ以上。既存ファイルへの
  上書きは `--overwrite` を除き終了コード 2。
- 判定スクリプトは既定でバイナリ埋め込みの標準スクリプト (§2.4 / §7.5)。
  `--script PATH` で JLS 形式のカスタムスクリプトに差し替え可能。
- 解析計算の実行先は `--analysis-backend auto|cpu|gpu` (既定 `auto`)。
  `auto`/`gpu` は Vulkan compute デバイスがあればシーン特徴量 (mvec) と
  ロゴ特徴量 (LogoCalc pix/areasum) を GPU で計算し、利用不能・失敗時は
  CPU へ自動フォールバックする (明示 `gpu` でフォールバック時は JSON の
  `warnings` に `analysis_gpu_fallback` を記録)。CPU/GPU 間の結果は設計書
  §6.5 の許容誤差規範に従う (現実装はビット一致)。GPU デコード
  (`--backend nvdec|vaapi`) とは独立の経路。
- 詳細な挙動・全オプションは `susuki --help` と設計規範书が正本。

## 安全性 (v1)

- 入力経路は CLI オプションに限定。スクリプトの `FileOpen` / `FileAppend` /
  `FileClose` / `ReadData` / `ReadTrim` / `ReadString` / `EnvGet` は
  静的検査で検出し、終了コード 6 で拒否 (設計 §7.5)。
- 出力ファイルは `--output-*` で明示したもののみ。ECHO は stderr に退避。

## crates

| crate | 役割 |
|-------|------|
| `susuki` | CLI とパイプライン配線 (デコード→解析→スクリプト実行→出力) |
| `susuki-core` | 時間軸・無音/シーンチェンジ検出・ロゴ検出数式・JLS エンジン (join_logo_scp 移植) |
| `susuki-decode` | FFmpeg 読み込み (demux/decode、probe、バックエンド選択) |
| `susuki-logo-file` | logoframe 形式ロゴファイルの読込・検証 |

## ライセンス・派生

本ツールは GPL-3.0-or-later です (`LICENSE`)。判定部・検出部の多くは
join_logo_scp / chapter_exe / logoframe (いずれも GPL-2.0-or-later) の
固定 commit からの翻案であり、移植ファイルには出処を明記しています。
FFmpeg (LGPL-2.1 ビルド)、libaribtlv (MIT) 等をリンクします。
条文・出処一覧は `LICENSES/README.md`、謝辞は `NOTICE` を参照してください。

検証用試料・録画データは公開しません (設計 §10.4)。
