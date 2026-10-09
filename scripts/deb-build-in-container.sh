#!/usr/bin/env bash
# deb-build-in-container.sh — runs inside the packaging/deb/Containerfile image,
# started by scripts/package-deb.sh with the sources already extracted next to
# this script and /out mounted for results. Builds the dedicated FFmpeg and the
# release binary for this distribution, stages them with package-dist.sh and
# packs /usr/lib/susuki (binary + bundled FFmpeg) into a .deb.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-/out}"
: "${DIST:?}" "${SOURCE_SHA:?}" "${BASE_IMAGE:?}" "${BASE_DIGEST:?}"
export JOBS="${JOBS:-$(nproc)}"
cd "$ROOT"

# shellcheck disable=SC1091  # ffmpeg.lock is data-only kv
source "$ROOT/ffmpeg.lock"

bash scripts/ffmpeg-build.sh
SUSUKI_FFMPEG_PREFIX="$(echo "$ROOT"/dist/ffmpeg-*)"
export SUSUKI_FFMPEG_PREFIX
cargo build --release --locked -p susuki

STAGE="$ROOT/dist/pkg"
bash scripts/package-dist.sh "$STAGE"

VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[ -n "$VERSION" ] || { echo "error: workspace version not found" >&2; exit 1; }
DEB_VERSION="$VERSION-1~$DIST"

# dpkg-shlibdeps treats libraries found under debian/<pkg> as the package's own
PKGROOT="$ROOT/dist/debroot"
TREE="$PKGROOT/debian/susuki"
LIBDIR="$TREE/usr/lib/susuki/lib"
mkdir -p "$LIBDIR" "$TREE/usr/bin" "$TREE/usr/share/doc/susuki" "$TREE/DEBIAN"
install -m 0755 "$STAGE/susuki" "$TREE/usr/lib/susuki/susuki"
ln -s ../lib/susuki/susuki "$TREE/usr/bin/susuki"

# package-dist.sh dereferences every lib*.so* alias into a full copy; ship only
# the SONAMEs that are actually loaded (transitively from the binary).
needed() { readelf -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p'; }
queue=("$TREE/usr/lib/susuki/susuki")
while [ "${#queue[@]}" -gt 0 ]; do
    obj="${queue[0]}"
    queue=("${queue[@]:1}")
    while read -r so; do
        if [ -e "$STAGE/lib/$so" ] && [ ! -e "$LIBDIR/$so" ]; then
            install -m 0644 "$STAGE/lib/$so" "$LIBDIR/$so"
            queue+=("$LIBDIR/$so")
        fi
    done < <(needed "$obj")
done
strip --strip-unneeded "$TREE/usr/lib/susuki/susuki" "$LIBDIR"/*.so*

DOC="$TREE/usr/share/doc/susuki"
{
    echo "susuki is licensed under GPL-3.0-or-later; the full text follows."
    echo "Third-party components, provenance and their license texts are listed"
    echo "in NOTICE and LICENSES/ in this directory."
    echo
    cat LICENSE
} > "$DOC/copyright"
cp NOTICE Readme.md "$DOC/"
cp -r LICENSES "$DOC/LICENSES"
{
    echo "source_sha=$SOURCE_SHA"
    echo "dist=$DIST"
    echo "base_image=$BASE_IMAGE"
    echo "base_digest=$BASE_DIGEST"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "gcc=$(gcc --version | head -n1)"
    echo "libclang=$(dpkg-query -W -f '${Package} ${Version}\n' 'libclang-[0-9]*-dev' 2>/dev/null | paste -sd ' ')"
    echo "cmake=$(cmake --version | head -n1)"
    echo "nasm=$(nasm -v)"
    echo "dpkg=$(dpkg-query -W -f '${Version}' dpkg)"
    echo "ffmpeg=$FFMPEG_TAG $FFMPEG_COMMIT"
    echo "libaribtlv=$LIBARIBTLV_TAG $LIBARIBTLV_COMMIT"
    echo "nv-codec-headers=$NVCODEC_TAG $NVCODEC_COMMIT"
} > "$DOC/build-info.txt"
find "$DOC" -type d -exec chmod 0755 {} + && find "$DOC" -type f -exec chmod 0644 {} +

mkdir -p "$PKGROOT/debian"
printf 'Source: susuki\n\nPackage: susuki\nArchitecture: amd64\n' > "$PKGROOT/debian/control"
depends="$(cd "$PKGROOT" && dpkg-shlibdeps -O \
    -l"$LIBDIR" \
    -e"debian/susuki/usr/lib/susuki/susuki" debian/susuki/usr/lib/susuki/lib/*.so* \
    | sed -n 's/^shlibs:Depends=//p')"
[ -n "$depends" ] || { echo "error: dpkg-shlibdeps produced no Depends" >&2; exit 1; }

cat > "$TREE/DEBIAN/control" <<EOF
Package: susuki
Version: $DEB_VERSION
Architecture: amd64
Maintainer: MistVVK <jffyc82pzv@privaterelay.appleid.com>
Installed-Size: $(du -sk --exclude=DEBIAN "$TREE" | cut -f1)
Depends: $depends
Recommends: libvulkan1
Section: video
Priority: optional
Description: CM interval detection for broadcast recordings
 Susuki estimates commercial-break intervals in broadcast recordings
 (MPEG-TS / MXF / MKV / MP4 / OGG, including TLV/MMT) with a dedicated
 FFmpeg build bundled under /usr/lib/susuki.
EOF
(cd "$TREE" && find usr -type f -exec md5sum {} + | sort -k2 > DEBIAN/md5sums)

deb="$OUT/susuki_${DEB_VERSION}_amd64.deb"
dpkg-deb --root-owner-group --build "$TREE" "$deb"
cp "$DOC/build-info.txt" "$OUT/"
dpkg-deb --info "$deb"
echo "built: $deb"
