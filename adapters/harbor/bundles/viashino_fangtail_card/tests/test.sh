#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/viashino_fangtail.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Viashino Fangtail source: ${candidate}" >&2
  exit 1
fi
echo "Viashino Fangtail module present; isolated Harbor scoring follows submission."
