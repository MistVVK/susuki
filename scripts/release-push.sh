#!/usr/bin/env bash
# release-push.sh — tag main for release, scan every commit about to be
# published and push only main and that tag (never dev or any other branch).
# Usage: scripts/release-push.sh [--scan-only | --push]
#   (none)       create/verify tag v<version> on HEAD, scan, do not push
#   --push       same, then push main and the tag (fast-forward only)
#   --scan-only  scan HEAD's unpublished history only; needs no tag or origin
# The scan covers each commit's whole tree plus author/committer and message,
# so content added in one commit and removed later is still caught.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-}"
case "$MODE" in ""|--push|--scan-only) ;; *) echo "usage: $0 [--scan-only | --push]" >&2; exit 2 ;; esac
cd "$ROOT"

# Addresses that are public by decision (git@github.com is the SSH remote); anything else is reported.
ALLOWED_EMAILS='^(jffyc82pzv@privaterelay\.appleid\.com|git@github\.com)$'
SECRET_RE='-----BEGIN [A-Z ]*PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[abprs]-[A-Za-z0-9-]{10,}|sk-[A-Za-z0-9_-]{20,}'
LOCAL_RE='/home/[a-z_][a-z0-9_-]*|/Users/[A-Za-z]|[Cc]:\\Users\\|\b10\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\b|\b192\.168\.[0-9]{1,3}\.[0-9]{1,3}\b|\b172\.(1[6-9]|2[0-9]|3[01])\.[0-9]{1,3}\.[0-9]{1,3}\b'
EMAIL_RE='[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
MEDIA_RE='\.(ts|m2ts|mts|mxf|mkv|mp4|m4v|mp3|aac|wav|flac)$'
MAX_BLOB=5000000
# Upstream patches are vendored verbatim (their From: lines name the upstream author).
EMAIL_EXCLUDE=':(exclude)vendor/patches'

[ -z "$(git status --porcelain)" ] || { echo "error: worktree has uncommitted changes" >&2; exit 1; }

if [ "$MODE" != --scan-only ]; then
    [ "$(git symbolic-ref --short HEAD)" = main ] || { echo "error: release from main (git switch main && git merge --ff-only dev)" >&2; exit 1; }
    git remote get-url origin >/dev/null 2>&1 || { echo "error: no origin remote" >&2; exit 1; }
    git fetch --quiet origin
    if git rev-parse -q --verify refs/remotes/origin/main >/dev/null; then
        git merge-base --is-ancestor origin/main HEAD \
            || { echo "error: origin/main is not an ancestor of HEAD; refusing to rewrite published history" >&2; exit 1; }
    fi
    VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
    TAG="v$VERSION"
    if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
        [ "$(git rev-parse "$TAG^{commit}")" = "$(git rev-parse HEAD)" ] \
            || { echo "error: tag $TAG exists but does not point to HEAD" >&2; exit 1; }
    else
        git tag -a "$TAG" -m "susuki $VERSION"
        echo "created tag $TAG"
    fi
    if remote_tag="$(git ls-remote --tags origin "refs/tags/$TAG^{}" | cut -f1)" && [ -n "$remote_tag" ] \
        && [ "$remote_tag" != "$(git rev-parse HEAD)" ]; then
        echo "error: origin already has $TAG on a different commit" >&2
        exit 1
    fi
fi

if git rev-parse -q --verify refs/remotes/origin/main >/dev/null; then
    range="origin/main..HEAD"
else
    range="HEAD"
fi
commits="$(git rev-list --reverse "$range")"
findings=0
report() { echo "  $*"; findings=$((findings + 1)); }
for c in $commits; do
    short="$(git rev-parse --short "$c")"
    while IFS= read -r hit; do report "$short ${hit#"$c":}"; done < <(
        git grep -I -n -E -e "$SECRET_RE" -e "$LOCAL_RE" "$c" -- . || true)
    while IFS= read -r hit; do
        addrs="$(grep -oE "$EMAIL_RE" <<<"${hit#"$c":}" | grep -vE "$ALLOWED_EMAILS" || true)"
        [ -z "$addrs" ] || report "$short ${hit#"$c":}"
    done < <(git grep -I -n -E -e "$EMAIL_RE" "$c" -- . "$EMAIL_EXCLUDE" || true)
    while read -r _ type _ size path; do
        [ "$type" = blob ] || continue
        if [ "$size" -gt "$MAX_BLOB" ] || grep -Eiq "$MEDIA_RE" <<<"$path"; then
            report "$short $path: large or media file ($size bytes)"
        fi
    done < <(git ls-tree -r -l "$c")
    for who in "$(git log -1 --format=%ae "$c")" "$(git log -1 --format=%ce "$c")"; do
        grep -Eq "$ALLOWED_EMAILS" <<<"$who" || report "$short author/committer email: $who"
    done
    msg="$(git log -1 --format=%B "$c")"
    while IFS= read -r line; do report "$short message: $line"; done < <(
        grep -E -e "$SECRET_RE" -e "$LOCAL_RE" <<<"$msg" || true)
    while IFS= read -r line; do
        addrs="$(grep -oE "$EMAIL_RE" <<<"$line" | grep -vE "$ALLOWED_EMAILS" || true)"
        [ -z "$addrs" ] || report "$short message: $line"
    done < <(grep -E -e "$EMAIL_RE" <<<"$msg" || true)
done
count="$(wc -w <<<"$commits")"
if [ "$findings" -gt 0 ]; then
    echo "error: $findings finding(s) in $count commit(s) ($range); nothing pushed" >&2
    exit 1
fi
echo "scan passed: $count commit(s) ($range)"

[ "$MODE" = --push ] || { [ "$MODE" = --scan-only ] || echo "rerun with --push to publish main and $TAG"; exit 0; }
git push origin refs/heads/main:refs/heads/main
git push origin "refs/tags/$TAG"
echo "pushed main and $TAG"
