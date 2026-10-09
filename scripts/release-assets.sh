#!/usr/bin/env bash
# release-assets.sh — collect the deb and Windows packages of one commit into
# dist/release/v<version>/ with SHA256SUMS, per-target build info and release
# notes. Run after scripts/package-deb.sh and scripts/package-windows.sh.
# The commit is the tag v<version> when it exists, otherwise HEAD; every
# package must have been built from it (build-info.txt source_sha).
# Usage: scripts/release-assets.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISTS=(jammy noble resolute trixie)
declare -A DIST_NAMES=(
    [jammy]="Ubuntu 22.04 LTS"
    [noble]="Ubuntu 24.04 LTS"
    [resolute]="Ubuntu 26.04 LTS"
    [trixie]="Debian 13"
)

VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml")"
[ -n "$VERSION" ] || { echo "error: workspace version not found" >&2; exit 1; }
TAG="v$VERSION"
if git -C "$ROOT" rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    COMMIT="$(git -C "$ROOT" rev-parse "$TAG^{commit}")"
else
    COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
fi

errors=0
fail() { echo "error: $*" >&2; errors=$((errors + 1)); }
info_value() { sed -n "s/^$2=//p" "$1"; }
check_built() { # package build-info
    [ -f "$1" ] || { fail "missing $1"; return; }
    [ -f "$2" ] || { fail "missing $2"; return; }
    local sha
    sha="$(info_value "$2" source_sha)"
    [ "$sha" = "$COMMIT" ] || fail "$1 was built from ${sha:-unknown}, expected $COMMIT"
}
for d in "${DISTS[@]}"; do
    check_built "$ROOT/dist/deb/$d/susuki_${VERSION}-1~${d}_amd64.deb" "$ROOT/dist/deb/$d/build-info.txt"
done
WIN_NAME="susuki-$VERSION-windows-x86_64.7z"
check_built "$ROOT/dist/windows/$WIN_NAME" "$ROOT/dist/windows/build-info.txt"
if [ -f "$ROOT/dist/windows/build-info.txt" ] \
    && [ "$(info_value "$ROOT/dist/windows/build-info.txt" target)" != x86_64-pc-windows-gnu ]; then
    fail "dist/windows was not built for x86_64-pc-windows-gnu"
fi
[ "$errors" -eq 0 ] || { echo "error: $errors problem(s); rebuild the packages from $COMMIT" >&2; exit 1; }

OUT="$ROOT/dist/release/$TAG"
rm -rf "$OUT"
mkdir -p "$OUT"
for d in "${DISTS[@]}"; do
    cp "$ROOT/dist/deb/$d/susuki_${VERSION}-1~${d}_amd64.deb" "$OUT/"
    cp "$ROOT/dist/deb/$d/build-info.txt" "$OUT/build-info-$d.txt"
done
cp "$ROOT/dist/windows/$WIN_NAME" "$OUT/"
cp "$ROOT/dist/windows/build-info.txt" "$OUT/build-info-windows.txt"
(cd "$OUT" && sha256sum ./*.deb ./*.7z | sed 's# \./# #' > SHA256SUMS)
# read by scripts/release.sh; not uploaded
echo "$COMMIT" > "$OUT/source-sha.txt"

win_info="$OUT/build-info-windows.txt"
{
    echo "## ダウンロード"
    echo
    echo "| 対象 | ファイル |"
    echo "|------|----------|"
    for d in "${DISTS[@]}"; do
        echo "| ${DIST_NAMES[$d]} (amd64) | \`susuki_${VERSION}-1~${d}_amd64.deb\` |"
    done
    echo "| Windows 10 以降 (x64) | \`$WIN_NAME\` |"
    echo
    echo "## インストール"
    echo
    echo "- Debian / Ubuntu: \`sudo apt install ./susuki_${VERSION}-1~<ディストリ>_amd64.deb\`"
    echo "- Windows: 7z を任意のフォルダに展開し、\`susuki.exe\` を実行する (DLL は exe と同じフォルダに置いたままにする)"
    echo
    echo "詳しくは [Readme](https://github.com/MistVVK/susuki/blob/$TAG/Readme.md) を参照してください。"
    echo
    echo "## ビルド情報"
    echo
    echo "- ソース: $COMMIT"
    echo "- Rust: $(info_value "$win_info" rustc)"
    echo "- FFmpeg: $(info_value "$win_info" ffmpeg) (libaribtlv $(info_value "$win_info" libaribtlv))"
    echo "- 各配布物のツールの版は \`build-info-*.txt\` に記録しています。"
    echo
    echo "## SHA256"
    echo
    echo '```'
    cat "$OUT/SHA256SUMS"
    echo '```'
} > "$OUT/release-notes.md"

echo "release assets for $TAG ($COMMIT) in $OUT:"
ls -1 "$OUT"
