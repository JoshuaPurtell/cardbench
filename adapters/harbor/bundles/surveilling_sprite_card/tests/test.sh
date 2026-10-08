#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/surveilling_sprite.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Surveilling Sprite source: ${candidate}" >&2
  exit 1
fi
echo "Surveilling Sprite module present; isolated Harbor scoring follows submission."
