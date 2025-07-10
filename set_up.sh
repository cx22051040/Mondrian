#!/usr/bin/env sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/configs"

TARGET_DIR="$HOME/.config/Mondrian"

echo "Source: $SOURCE_DIR"
echo "Target: $TARGET_DIR"

mkdir -p "$TARGET_DIR"

cp -r "$SOURCE_DIR/"* "$TARGET_DIR/"

echo "configs copy to $TARGET_DIR"
