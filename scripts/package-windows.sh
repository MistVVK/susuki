#!/usr/bin/env bash
# package-windows.sh — cross-build the Windows x86_64 distribution 7z in a
# throwaway rootless Podman container (Debian + MinGW-w64; no Windows host).
# Usage: scripts/package-windows.sh
# Env:   RUST_VERSION (default: current stable), JOBS, OUT (default: dist/windows)
# Builds the committed HEAD via git archive; a dirty worktree is rejected so the
# recorded SHA identifies the packaged sources.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE=debian:13

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
OUT="${OUT:-$ROOT/dist/windows}"
echo "source $SOURCE_SHA, rust $RUST_VERSION, target x86_64-pc-windows-gnu"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
git -C "$ROOT" archive --format=tar -o "$WORK/src.tar" "$SOURCE_SHA"

image="susuki-windows-build:latest"
rm -rf "$OUT"
mkdir -p "$OUT"
podman build -t "$image" --build-arg "RUST_VERSION=$RUST_VERSION" "$ROOT/packaging/windows"
digest="$(podman image inspect --format '{{index .RepoDigests 0}}' "$BASE")"
# rootless: container root maps to the invoking user, so /out stays user-owned
podman run --rm \
    -v "$WORK/src.tar:/in/src.tar:ro,Z" \
    -v "$OUT:/out:Z" \
    -e "SOURCE_SHA=$SOURCE_SHA" -e "BASE_IMAGE=$BASE" -e "BASE_DIGEST=$digest" \
    ${JOBS:+-e "JOBS=$JOBS"} \
    "$image" \
    bash -c 'mkdir -p /build/src && tar -xf /in/src.tar -C /build/src && exec bash /build/src/scripts/windows-build-in-container.sh' \
    2>&1 | tee "$OUT/build.log"
echo "packages:"
ls -1 "$OUT"/*.7z
