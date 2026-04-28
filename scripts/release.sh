#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <version>   (e.g. $0 0.0.3)" >&2
  exit 1
fi

VERSION="$1"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: version must be semver X.Y.Z (got: $VERSION)" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: working tree is not clean. Commit or stash changes first." >&2
  exit 1
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
  echo "Error: must release from main (current: $BRANCH)" >&2
  exit 1
fi

TAG="v$VERSION"
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag $TAG already exists" >&2
  exit 1
fi

git fetch --tags origin

CURRENT="$(awk -F'"' '/^version[[:space:]]*=/ {print $2; exit}' extension.toml)"
echo "Bumping extension.toml: $CURRENT -> $VERSION"

# macOS sed
sed -i '' -E "s/^version = \"[^\"]+\"/version = \"$VERSION\"/" extension.toml

if ! grep -q "^version = \"$VERSION\"$" extension.toml; then
  echo "Error: failed to update extension.toml" >&2
  git checkout -- extension.toml
  exit 1
fi

git add extension.toml
git commit -m "Release v$VERSION"
git tag "$TAG"

echo
echo "Ready to push:"
echo "  commit: $(git rev-parse --short HEAD)"
echo "  tag:    $TAG"
echo
read -r -p "Push to origin now? [y/N] " confirm
if [[ "$confirm" =~ ^[Yy]$ ]]; then
  git push origin main
  git push origin "$TAG"
  echo "Pushed. GitHub Action will open a PR to Damecek/zed-extensions."
else
  echo "Skipped push. Run manually:"
  echo "  git push origin main && git push origin $TAG"
fi
