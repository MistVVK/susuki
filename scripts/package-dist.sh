#!/usr/bin/env bash
# package-dist.sh — §10.4 binary distribution staging (run after
# scripts/ffmpeg-build.sh && cargo build --release inside the source tree).
# Layout: <out>/susuki, <out>/lib/*.so (dedicated FFmpeg + deps),
#         <out>/LICENSE, <out>/NOTICE, <out>/LICENSES/, <out>/Readme.md
# The binary's only RPATH is $ORIGIN/lib (crates/susuki/build.rs), which
# resolves to <out>/lib without touching system FFmpeg (§10.2/REQ-BD-001/003).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="$(echo "$ROOT"/dist/ffmpeg-*)"
[ -d "$PREFIX" ] || { echo "error: run scripts/ffmpeg-build.sh first (no dist/ffmpeg-*)" >&2; exit 1; }
BIN="$ROOT/target/release/susuki"
[ -x "$BIN" ] || { echo "error: missing release binary; run cargo build --release" >&2; exit 1; }
OUT="${1:-$ROOT/dist/pkg}"
rm -rf "$OUT"
mkdir -p "$OUT/lib"
cp "$BIN" "$OUT/susuki"
cp -L "$PREFIX"/lib/lib*.so* "$OUT/lib/" 2>/dev/null || cp -L "$PREFIX"/lib/lib*.so "$OUT/lib/"
for f in LICENSE NOTICE Readme.md; do cp "$ROOT/$f" "$OUT/"; done
cp -r "$ROOT/LICENSES" "$OUT/LICENSES"
# integrity of the bundled decoder libraries
(cd "$OUT/lib" && sha256sum lib* > SHA256SUMS.lib)

# the staged binary must resolve every FFmpeg library from <out>/lib
runpath="$(readelf -d "$OUT/susuki" | sed -n 's/.*(\(RUNPATH\|RPATH\)).*\[\(.*\)\]/\2/p')"
[ "$runpath" = "\$ORIGIN/lib" ] || { echo "error: unexpected RUNPATH '$runpath'" >&2; exit 1; }
deps="$(env -u LD_LIBRARY_PATH ldd "$OUT/susuki")"
echo "$deps" | grep -E "libav|libsw|libarib" || true
if echo "$deps" | grep -q "not found"; then
    echo "error: unresolved libraries in staged binary" >&2
    exit 1
fi
lib_real="$(cd "$OUT/lib" && pwd -P)"
for l in libavformat libavcodec libavutil libswresample; do
    path="$(echo "$deps" | sed -n "s/^[[:space:]]*$l\.so[^ ]* => \([^ ]*\) .*/\1/p")"
    [ "$(dirname "$(readlink -f "$path")")" = "$lib_real" ] || {
        echo "error: $l resolved outside $OUT/lib: $path" >&2
        exit 1
    }
done
echo "staged: $OUT"
