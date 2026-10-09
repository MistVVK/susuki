#!/usr/bin/env bash
# release.sh — publish dist/release/v<version>/ (made by release-assets.sh) as
# a GitHub release with gh. The tag must already be on origin
# (release-push.sh --push); an existing release is never overwritten.
# Usage: scripts/release.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v gh >/dev/null || { echo "error: missing tool: gh" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "error: gh is not logged in (gh auth login)" >&2; exit 1; }
VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
TAG="v$VERSION"
DIR="$ROOT/dist/release/$TAG"

git rev-parse -q --verify "refs/tags/$TAG" >/dev/null || { echo "error: no local tag $TAG" >&2; exit 1; }
COMMIT="$(git rev-parse "$TAG^{commit}")"
remote_tag="$(git ls-remote --tags origin "refs/tags/$TAG^{}" | cut -f1)"
[ "$remote_tag" = "$COMMIT" ] || { echo "error: origin does not have $TAG at $COMMIT; run release-push.sh --push" >&2; exit 1; }
[ -d "$DIR" ] || { echo "error: missing $DIR; run release-assets.sh" >&2; exit 1; }
[ "$(cat "$DIR/source-sha.txt")" = "$COMMIT" ] || { echo "error: $DIR was assembled for another commit; rerun release-assets.sh" >&2; exit 1; }
(cd "$DIR" && sha256sum --quiet -c SHA256SUMS)
if gh release view "$TAG" >/dev/null 2>&1; then
    echo "error: release $TAG already exists; edit it with gh release upload/edit instead" >&2
    exit 1
fi

gh release create "$TAG" --verify-tag \
    --title "susuki $VERSION" --notes-file "$DIR/release-notes.md" \
    "$DIR"/*.deb "$DIR"/*.7z "$DIR/SHA256SUMS" "$DIR"/build-info-*.txt
gh release view "$TAG" --json url --jq .url
