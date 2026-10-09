# 開発時の lint・検証規則

## 実行環境と前提

- 検証は作業用 Incus コンテナ内で行う。対象ソースをコンテナへ配置し、以下のコマンドはリポジトリ直下（Cargo.toml があるディレクトリ）から実行する。
- Cargo.toml の宣言は edition 2021、rust-version 1.85。リポジトリに Rust toolchain の固定ファイルはない。確認済みの環境は stable の rustc/Cargo 1.99.0、rustfmt 1.10.0-stable、Clippy 0.1.99、ShellCheck 0.10.0。これは実測値であり、最低対応版の検証結果ではない。検証時には実際のバージョンを記録する。
- 使用する Rust toolchain に rustfmt と Clippy を用意する（rustup を使う場合は `rustup component add rustfmt clippy`）。ShellCheck もコンテナ内に用意する。
- Clippy は build.rs も実行するため、bindgen 用の Clang ライブラリ（libclang と開発パッケージ）と C コンパイラが必要。確認済みの環境は libclang 19.1.7 と GCC 14.2.0。Clang のコマンドラインドライバは当該環境に未導入であり、libclang と区別する。
- 専用 FFmpeg の構築には Bash、Git、C/C++ コンパイラ、CMake、Make、pkg-config、アセンブラ（NASM/Yasm）、および zlib・bzip2・xz/liblzma・libva の開発パッケージを用意する。配布物の取得に必要なネットワークと CA 証明書もコンテナ内で利用可能にする。

## 専用 FFmpeg

Clippy の前に `scripts/ffmpeg-build.sh` で専用 FFmpeg を構築する。
このスクリプトは ffmpeg.lock の固定 commit と vendor/patches の SHA256 を検証する。
システム FFmpeg への置き換えは禁止する。
`SUSUKI_FFMPEG_PREFIX` は専用 FFmpeg の include/ と lib/ がある prefix を指す必要がある。

作業用コンテナ内での配置例:

```bash
PREFIX="$PWD/dist/ffmpeg-lint" bash scripts/ffmpeg-build.sh
export SUSUKI_FFMPEG_PREFIX="$PWD/dist/ffmpeg-lint"
```

ビルド用ディレクトリの既定値は `.ffmpeg-build`。
既に存在するとスクリプトは失敗するため、再構築時には未使用の `BUILD_DIR` を指定する。
既存の専用 FFmpeg を使う場合は、そのビルド元と固定参照が検証対象に適合していることを確認し、実際の prefix を `SUSUKI_FFMPEG_PREFIX` に設定する。

### Windows 版の依存ライブラリの更新確認

Windows 版をビルドする前に、専用 FFmpeg に静的に組み込む依存ライブラリ (ffmpeg.lock の `ZLIB_*`・`BZIP2_*`・`XZ_*`) が上流の最新安定版か確認する。
Windows 版にはこれらがそのまま同梱されるため、古い版のままだと修正済みの脆弱性を配布することになる。

```bash
source ffmpeg.lock
for repo in "$ZLIB_REPO" "$BZIP2_REPO" "$XZ_REPO"; do
    git ls-remote --tags --refs "$repo" | sed 's#.*refs/tags/##' | grep -Eiv 'rc|alpha|beta|pre' | sort -V | tail -n1
done
```

- 最新の安定版タグが `*_TAG` より新しい場合は、上流のリリースノートで変更点 (セキュリティ修正・ビルド方法の変更) を確認し、`*_TAG` と `*_COMMIT` を更新してから Windows 版をビルドし直す。
  `*_COMMIT` にはタグが指すコミット (`git ls-remote` の `refs/tags/<タグ>^{}` の値) を記録する。
- 確認時点の最新版と採用した版を検証証拠に記録する。
- FFmpeg・libaribtlv・nv-codec-headers と vendor/patches は TLV パッチの固定 commit に合わせているため、この確認の対象外とする。

## 必須チェック

コード変更後、およびレビューへ渡す前に、次のチェックを実行する。
Clippy には上記の `SUSUKI_FFMPEG_PREFIX` を設定した環境を使用する。

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
shellcheck scripts/ffmpeg-build.sh
shellcheck scripts/package-dist.sh
shellcheck scripts/package-deb.sh
shellcheck scripts/deb-build-in-container.sh
shellcheck scripts/package-windows.sh
shellcheck scripts/windows-build-in-container.sh
shellcheck scripts/release-assets.sh
shellcheck scripts/release-push.sh
shellcheck scripts/release.sh
```

Windows 向けのコード (`cfg(windows)`) を変更した場合は、Windows ターゲットの Clippy も実行する。
`packaging/windows/Containerfile` のイメージ内で `TARGET_OS=windows` の専用 FFmpeg を構築し、
その prefix を `SUSUKI_FFMPEG_PREFIX` に設定する。

```bash
cargo clippy --locked --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

`--locked` と `-D warnings` を外して合格の代わりにしない。
pedantic/nursery の一括必須化は行わない。

## 警告・修正の扱い

- 警告はまず原因を修正する。FFI や参照実装の忠実な移植など、抑制の根拠がある場合だけ、対象 lint を最小範囲で allow/disable し、理由をその箇所のコメントに残す。
- 生成物や外部ソースを lint 対応のために手編集しない。所有するソースや生成元で対応する。
- 参照移植の評価順序、丸め、状態遷移を lint 都合で変えない。自動修正も差分を確認し、既存の検証で挙動を確認する。

## 検証証拠

- 対象の完全 Git SHA、未コミット差分の有無とその範囲、コンテナ内で実際に検査したソースを記録する。未コミットの変更がある場合、SHA だけでは検査対象を特定できない。
- 使用した Rust/Cargo/rustfmt/Clippy/ShellCheck、libclang・C コンパイラのバージョン、専用 FFmpeg のビルド元・prefix、実行ディレクトリを証拠に記録する。
- 各コマンドと終了コードを個別に保存する。失敗時は原因と代表箇所を記録し、修正した場合は修正後の結果も残す。
- ツール不足や専用 FFmpeg などの前提不足は実行不能として報告する。未実行を合格にせず、未検証の主張は `unverified` と明記する。
- 公開する文書にはマシン固有の絶対パスや既存コンテナ名への依存を持ち込まない。検証証拠に認証情報や録画データを含めない。

## ブランチとリリース

- 開発は `dev` ブランチで行う。リモート (GitHub) へ push するのは `main` とリリースタグ `v<version>` だけで、`dev` やその他のブランチは push しない。
- リリース (配布物のビルド・`main` とタグの push・GitHub Releases への公開) を行うときは、`.agents/skills/release/SKILL.md` を読み、その手順に従う。
- リリースのための `main` とタグの push、および `gh release create` は、この手順で行う場合に限って許可する。それ以外の push や gh 操作、force push、公開済みの履歴やタグの書き換えは行わない。
