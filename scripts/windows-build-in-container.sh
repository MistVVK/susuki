#!/usr/bin/env bash
# windows-build-in-container.sh — runs inside the packaging/windows/Containerfile
# image, started by scripts/package-windows.sh with the sources already
# extracted next to this script and /out mounted for results. Cross-builds the
# dedicated FFmpeg and susuki.exe for x86_64-pc-windows-gnu, smoke-tests the
# exe under Wine and packs it with the DLLs it loads into a 7z (BZip2) archive.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-/out}"
: "${SOURCE_SHA:?}" "${BASE_IMAGE:?}" "${BASE_DIGEST:?}"
export JOBS="${JOBS:-$(nproc)}"
TARGET=x86_64-pc-windows-gnu
CROSS=x86_64-w64-mingw32-
cd "$ROOT"

# shellcheck disable=SC1091  # ffmpeg.lock is data-only kv
source "$ROOT/ffmpeg.lock"

FFPREFIX="$ROOT/dist/ffmpeg-windows-$RELEASE_TAG"
TARGET_OS=windows PREFIX="$FFPREFIX" bash scripts/ffmpeg-build.sh
export SUSUKI_FFMPEG_PREFIX="$FFPREFIX"
cargo build --release --locked --target "$TARGET" -p susuki

VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[ -n "$VERSION" ] || { echo "error: workspace version not found" >&2; exit 1; }
NAME="susuki-$VERSION-windows-x86_64"
STAGE="$ROOT/dist/$NAME"
mkdir -p "$STAGE"
install -m 0755 "target/$TARGET/release/susuki.exe" "$STAGE/susuki.exe"

# The loader searches the exe's directory first, so the DLLs sit next to it.
# Ship only the FFmpeg DLLs that are actually loaded (transitively from the exe).
imports() { "${CROSS}objdump" -p "$1" | sed -n 's/^[[:space:]]*DLL Name: //p'; }
queue=("$STAGE/susuki.exe")
while [ "${#queue[@]}" -gt 0 ]; do
    obj="${queue[0]}"
    queue=("${queue[@]:1}")
    while read -r dll; do
        if [ -e "$FFPREFIX/bin/$dll" ] && [ ! -e "$STAGE/$dll" ]; then
            install -m 0644 "$FFPREFIX/bin/$dll" "$STAGE/$dll"
            queue+=("$STAGE/$dll")
        fi
    done < <(imports "$obj")
done
"${CROSS}strip" --strip-unneeded "$STAGE"/*.exe "$STAGE"/*.dll

# Every import must be bundled or a Windows system DLL; system DLLs never start
# with lib/zlib, so such a name is a MinGW runtime or library DLL left behind.
for obj in "$STAGE"/*.exe "$STAGE"/*.dll; do
    while read -r dll; do
        if [ ! -e "$STAGE/$dll" ] && grep -Eiq '^(lib|zlib)' <<<"$dll"; then
            echo "error: $(basename "$obj") imports unbundled $dll" >&2
            exit 1
        fi
    done < <(imports "$obj")
done
for lib in avcodec avformat avutil swresample; do
    compgen -G "$STAGE/$lib-*.dll" >/dev/null || { echo "error: $lib DLL not bundled" >&2; exit 1; }
done

cp LICENSE NOTICE Readme.md "$STAGE/"
cp -r LICENSES "$STAGE/LICENSES"
{
    echo "source_sha=$SOURCE_SHA"
    echo "target=$TARGET"
    echo "base_image=$BASE_IMAGE"
    echo "base_digest=$BASE_DIGEST"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "mingw_gcc=$("${CROSS}gcc" --version | head -n1)"
    echo "libclang=$(dpkg-query -W -f '${Package} ${Version}\n' 'libclang-[0-9]*-dev' 2>/dev/null | paste -sd ' ')"
    echo "cmake=$(cmake --version | head -n1)"
    echo "nasm=$(nasm -v)"
    echo "wine=$(wine --version)"
    echo "7zip=$(7z i | sed -n 's/^7-Zip[^0-9]*\([0-9.]*\).*/\1/p' | head -n1)"
    echo "ffmpeg=$FFMPEG_TAG $FFMPEG_COMMIT"
    echo "libaribtlv=$LIBARIBTLV_TAG $LIBARIBTLV_COMMIT"
    echo "zlib=$ZLIB_TAG $ZLIB_COMMIT"
    echo "bzip2=$BZIP2_TAG $BZIP2_COMMIT"
    echo "xz=$XZ_TAG $XZ_COMMIT"
} > "$STAGE/build-info.txt"
(cd "$STAGE" && sha256sum ./*.exe ./*.dll > SHA256SUMS)

# Smoke test under Wine: the exe must load every bundled DLL and run.
export WINEDEBUG=-all WINEPREFIX="$ROOT/dist/wineprefix"
wine "$STAGE/susuki.exe" --version
wine "$STAGE/susuki.exe" --help >/dev/null

# -mf=off: 7z would otherwise add the BCJ2 filter to exe/dll, whose side
# streams are LZMA; keep every stream BZip2
(cd "$ROOT/dist" && 7z a -t7z -m0=BZip2 -mx=9 -mf=off "$OUT/$NAME.7z" "$NAME")
cp "$STAGE/build-info.txt" "$OUT/"
echo "built: $OUT/$NAME.7z"
