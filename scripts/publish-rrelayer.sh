#!/bin/bash
# Usage: ./scripts/publish-rrelayer.sh
# Publishes rrelayer_core and rrelayer to crates.io

set -euo pipefail

SCRIPT_DIR="$(dirname "${BASH_SOURCE[0]}")"
cd "$SCRIPT_DIR/.."

echo "🚀 Publishing rrelayer to crates.io..."
echo

echo "🔍 Checking cargo login..."
if ! cargo search --limit 1 serde &>/dev/null; then
    echo "❌ Not logged into cargo. Run 'cargo login' first."
    exit 1
fi
echo "✅ Cargo login verified"

echo
echo "📦 Step 1: Publishing rrelayer_core..."
cd "crates/core"

CORE_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Version: $CORE_VERSION"

cargo check || exit 1
cargo publish --dry-run --allow-dirty || exit 1

echo
read -p "Publish rrelayer_core v$CORE_VERSION? (y/N): " -n 1 -r
echo

if [[ $REPLY =~ ^[Yy]$ ]]; then
    cargo publish --allow-dirty || exit 1
    echo "✅ Published rrelayer_core v$CORE_VERSION"
    echo "⏳ Waiting 30s for crates.io..."
    sleep 30
else
    echo "❌ Cancelled"
    exit 1
fi

echo
echo "📦 Step 2: Publishing rrelayer..."
cd "../sdk"

SDK_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Version: $SDK_VERSION"

cp Cargo.toml Cargo.toml.bak

sed -i.tmp "s|rrelayer_core = { path = \"../core\", version = \".*\" }|rrelayer_core = \"$CORE_VERSION\"|" Cargo.toml

cargo check || {
    echo "❌ Check failed with published core"
    mv Cargo.toml.bak Cargo.toml
    exit 1
}

cargo publish --dry-run --allow-dirty || {
    echo "❌ Dry run failed"
    mv Cargo.toml.bak Cargo.toml
    exit 1
}

echo
read -p "Publish rrelayer v$SDK_VERSION? (y/N): " -n 1 -r
echo

if [[ $REPLY =~ ^[Yy]$ ]]; then
    cargo publish --allow-dirty || {
        echo "❌ Publish failed"
        mv Cargo.toml.bak Cargo.toml
        exit 1
    }
    
    echo
    echo "🎉 Success! Both packages published:"
    echo "📦 rrelayer_core v$CORE_VERSION"
    echo "📦 rrelayer v$SDK_VERSION"
    echo
    echo "Users install with: cargo add rrelayer"
    echo "📚 https://docs.rs/rrelayer"
    
    mv Cargo.toml.bak Cargo.toml
    echo "🔄 Restored local development setup"
else
    echo "❌ Cancelled"
    mv Cargo.toml.bak Cargo.toml
fi