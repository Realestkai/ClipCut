#!/usr/bin/env bash
# Publishes the bundles in ./bundles to the release $TAG, creating it on the
# commit $SHA when it does not exist yet. release.yml runs this once for
# Windows and once for the Macs, each as soon as its build is done, so
# whichever is first creates the release and the second adds to it:
# manifest.json and SHA256SUMS keep the first one's rows.
#
# Needs: GH_TOKEN, GITHUB_REPOSITORY, TAG, VERSION, SHA, RUNNER_TEMP, and
# the release's tree checked out in ./source with its history.
set -euo pipefail

repo="$GITHUB_REPOSITORY"
earlier="$RUNNER_TEMP/earlier"
mkdir -p "$earlier"

exists=false
if gh release view "$TAG" -R "$repo" >/dev/null 2>&1; then
  exists=true
  gh release download "$TAG" -R "$repo" -D "$earlier" -p manifest.json -p SHA256SUMS --clobber || true
fi

# The manifest: a row per bundle with its size and digest, and a row per
# model with its mirror. See scripts/models.py.
python3 source/scripts/models.py release-manifest \
  --version "$VERSION" --tag "$TAG" --bundles bundles \
  --merge "$earlier/manifest.json" --out bundles/manifest.json
cat bundles/manifest.json

# The checksums: this platform's, then any the release has already.
(cd bundles && sha256sum -- ClipCut-*) > "$RUNNER_TEMP/sums"
if [ -f "$earlier/SHA256SUMS" ]; then cat "$earlier/SHA256SUMS" >> "$RUNNER_TEMP/sums"; fi
awk '!seen[$2]++' "$RUNNER_TEMP/sums" | sort -k2 > bundles/SHA256SUMS
cat bundles/SHA256SUMS

if $exists; then
  gh release upload "$TAG" -R "$repo" --clobber bundles/*
  echo "Added to $TAG: $(cd bundles && ls | tr '\n' ' ')"
  exit 0
fi

# The notes are the commits since the previous release; see the script. The
# tag does not exist yet - creating the release makes it - so they run from
# the last release's tag to this commit.
previous=$(git -C source describe --tags --abbrev=0 --match 'v*' HEAD 2>/dev/null \
  || git -C source rev-list --max-parents=0 HEAD | tail -1)
(cd source && .github/release-notes.sh "$previous" HEAD "$VERSION") > "$RUNNER_TEMP/notes.md"
cat "$RUNNER_TEMP/notes.md"

gh release create "$TAG" -R "$repo" --target "$SHA" --title "ClipCut $TAG" \
  --notes-file "$RUNNER_TEMP/notes.md" --latest bundles/*
echo "Created $TAG"
