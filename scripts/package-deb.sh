#!/usr/bin/env bash
# package-deb.sh — build susuki .deb packages in throwaway rootless Podman
# containers, one per target distribution (each builds its own dedicated FFmpeg
# so the shared-library Depends match that distribution).
# Usage: scripts/package-deb.sh [jammy|noble|resolute|trixie ...]   (default: all)
# Env:   RUST_VERSION (default: current stable, resolved once for all targets),
#        JOBS, OUT_ROOT (default: dist/deb)
# Builds the committed HEAD via git archive; a dirty worktree is rejected so the
# recorded SHA identifies the packaged sources.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

declare -A BASES=(
    [jammy]=ubuntu:22.04
    [noble]=ubuntu:24.04
    [resolute]=ubuntu:26.04
    [trixie]=debian:13
)
ALL=(jammy noble resolute trixie)
if [ "$#" -gt 0 ]; then DISTS=("$@"); else DISTS=("${ALL[@]}"); fi
for d in "${DISTS[@]}"; do
    [ -n "${BASES[$d]:-}" ] || { echo "error: unknown target '$d' (${ALL[*]})" >&2; exit 1; }
done

command -v podman >/dev/null || { echo "error: missing tool: podman" >&2; exit 1; }
if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
    echo "error: worktree has uncommitted changes; commit them first" >&2
    exit 1
fi
SOURCE_SHA="$(git -C "$ROOT" rev-parse HEAD)"

if [ -z "${RUST_VERSION:-}" ]; then
    # fetch fully before parsing: awk exits early, which would fail curl under pipefail
    channel="$(curl --proto '=https' -fsS https://static.rust-lang.org/dist/channel-rust-stable.toml)"
    RUST_VERSION="$(awk '/^\[pkg\.rust\]/ { f = 1; next } f && /^version = / { split($0, a, "\""); split(a[2], b, " "); print b[1]; exit }' <<<"$channel")"
    [ -n "$RUST_VERSION" ] || { echo "error: cannot resolve Rust stable version" >&2; exit 1; }
fi
OUT_ROOT="${OUT_ROOT:-$ROOT/dist/deb}"
echo "source $SOURCE_SHA, rust $RUST_VERSION, targets: ${DISTS[*]}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
git -C "$ROOT" archive --format=tar -o "$WORK/src.tar" "$SOURCE_SHA"

for dist in "${DISTS[@]}"; do
    base="${BASES[$dist]}"
    image="susuki-deb-build:$dist"
    out="$OUT_ROOT/$dist"
    rm -rf "$out"
    mkdir -p "$out"
    podman build -t "$image" \
        --build-arg "BASE=$base" --build-arg "RUST_VERSION=$RUST_VERSION" \
        "$ROOT/packaging/deb"
    digest="$(podman image inspect --format '{{index .RepoDigests 0}}' "$base")"
    # rootless: container root maps to the invoking user, so /out stays user-owned
    podman run --rm \
        -v "$WORK/src.tar:/in/src.tar:ro,Z" \
        -v "$out:/out:Z" \
        -e "DIST=$dist" -e "SOURCE_SHA=$SOURCE_SHA" \
        -e "BASE_IMAGE=$base" -e "BASE_DIGEST=$digest" \
        ${JOBS:+-e "JOBS=$JOBS"} \
        "$image" \
        bash -c 'mkdir -p /build/src && tar -xf /in/src.tar -C /build/src && exec bash /build/src/scripts/deb-build-in-container.sh' \
        2>&1 | tee "$out/build.log"
done
echo "packages:"
ls -1 "$OUT_ROOT"/*/*.deb
