#!/usr/bin/env bash
# release.sh - Bumps the minor version in the workspace Cargo.toml and commits it.
set -euo pipefail

# Ensure we are in the root directory
cd "$(dirname "$0")"

# 1. Extract current version from [workspace.package]
# We look for the line 'version = "X.Y.Z"' following '[workspace.package]'
CURRENT_VERSION=$(sed -n '/\[workspace\.package\]/,/\[/p' Cargo.toml | grep '^version =' | cut -d '"' -f 2)

if [[ -z "$CURRENT_VERSION" ]]; then
    echo "Error: Could not find version in [workspace.package] in Cargo.toml"
    exit 1
fi

# 2. Bump minor version (X.Y.Z -> X.Y+1.0)
IFS='.' read -r major minor patch <<< "$CURRENT_VERSION"
NEW_MINOR=$((minor + 1))
NEW_VERSION="$major.$NEW_MINOR.0"

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

# 3. Update Cargo.toml and Cargo.lock
# Use sed to replace the version line within the [workspace.package] section
# We use a temp file for cross-platform compatibility with sed -i
sed "/\[workspace\.package\]/,/\[/ s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml
cargo check --workspace --quiet

# 4. Commit the change
git add Cargo.toml Cargo.lock
git commit -m "release: v$NEW_VERSION"

echo "Successfully bumped version to $NEW_VERSION and committed Cargo.toml."
echo "Push this change to trigger a new build: git push"
