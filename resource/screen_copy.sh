#!/bin/bash

# default save path
SAVE_DIR="$HOME/Downloads"
DATE=$(date "+%Y-%m-%d_%H-%M-%S")

EXT="png"

case $1 in
  --full)
    grim "$SAVE_DIR/fullscreen_$DATE.$EXT"
    ;;
  --partial)
    AREA=$(slurp)
    grim -g "$AREA" "$SAVE_DIR/partial_$DATE.$EXT"
    ;;
  *)
    echo "Usage: $0 --full | --partial"
    exit 1
    ;;
esac

echo "Screenshot saved to $SAVE_DIR"