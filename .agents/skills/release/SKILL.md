---
name: release
description: susuki のリリース手順。バージョン更新から deb (Ubuntu 22.04/24.04/26.04・Debian 13) と Windows 7z のビルド、push 前検査、main とタグの push、GitHub Releases への公開までを行う。リリース・公開・タグ付け・push を頼まれたときに使う。
---

# susuki のリリース

susuki の配布物 (deb 4 種と Windows 7z) を GitHub Releases (MistVVK/susuki) に公開する。
エージェントでも人間でも同じ手順で行う。判断の要らない検査と操作は `scripts/` にあり、ここには順序と判断基準を書く。

## 許可される操作 (例外の範囲)

リリースのために、次の操作だけを許可する。

- `scripts/release-push.sh --push` による `main` とリリースタグ `v<version>` の push
- `scripts/release.sh` による `gh release create`

次の操作はしない。

- `dev`、`backup/*` など `main` 以外のブランチの push
- force push、公開済みの履歴やタグの書き換え
- 上記以外の gh 操作 (公開済みリリースの修正・削除は、ユーザーに確認してから行う)

## ブランチ

- 開発は `dev` で行う。`dev` は push しない。
- リリースのときだけ、`main` を `dev` へ fast-forward でそろえて push する。

## 前提 (初回だけ。人間が行う)

- `gh` を導入し、`gh auth login` でログインする (git の通信は SSH)
- `git remote get-url origin` が `git@github.com:MistVVK/susuki.git` を返す
- rootless Podman が使え、コンテナからネットワーク (apt・GitHub・crates.io・rustup) に接続できる

## 手順

1. **バージョンを上げる** (`dev` で行う)
   - `Cargo.toml` の `[workspace.package] version` を上げ、`cargo update --workspace` で `Cargo.lock` を更新する。
   - AGENTS.md の「Windows 版の依存ライブラリの更新確認」を行う。
   - AGENTS.md の必須チェックを実行し、論理単位でコミットする。
2. **main をそろえる**
   - `git switch main && git merge --ff-only dev`
   - fast-forward できない場合は止めて、ユーザーに報告する。
3. **配布物をビルドする** (`main` で行う。どちらもコミット済みの HEAD をビルドする)
   - `./scripts/package-deb.sh`
   - `./scripts/package-windows.sh`
   - deb は、素のベースイメージ (`ubuntu:22.04` など) に `apt install ./<deb>` で入れ、`susuki --help` が終了コード 0 になること、`ldd /usr/lib/susuki/susuki` に not found が無いことを確かめる。Windows 版は package-windows.sh が Wine で起動を確かめる。
4. **配布物をまとめる**
   - `./scripts/release-assets.sh`
   - すべての配布物が同じコミットからビルドされていることを確かめ、`dist/release/v<version>/` に配布物・`SHA256SUMS`・`build-info-*.txt`・`release-notes.md` を作る。
   - 失敗した場合は、指示に従って配布物をビルドし直す。
5. **push 前の検査と push**
   - `./scripts/release-push.sh` を実行する。タグを作り (既にあれば HEAD を指すか確かめ)、push する各コミットのファイル全体・作者・メッセージを検査する。
   - 検出があれば **push せずに止め**、該当箇所をユーザーに報告して判断を仰ぐ。履歴の書き換えはユーザーの指示があるまでしない。
   - 検出が無ければ `./scripts/release-push.sh --push` で `main` とタグを push する。
6. **リリースを公開する**
   - `./scripts/release.sh` を実行し、表示されたリリースの URL を報告する。
   - 公開後に `gh release download v<version> -D <空のディレクトリ>` で取得し、`sha256sum -c SHA256SUMS` で照合する。
7. **dev を戻す**
   - `git switch dev` で開発ブランチに戻る。

## 失敗したとき

- 手順 5 より前の失敗は、外部に何も出ていない。原因を直して該当の手順からやり直す。
- push 後に問題が見つかった場合、公開済みのタグや履歴は書き換えず、修正版を次のバージョンとして出す。
- `release.sh` が途中で失敗し、リリースが中途半端に作られた場合は、`gh release view v<version>` で状態を確かめてユーザーに報告する。
