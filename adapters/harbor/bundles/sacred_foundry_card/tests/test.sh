#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-}"
if [[ -z "$candidate" ]]; then
  candidate="varieties/magic/sets/ravnica_city_of_guilds/src/cards/sacred_foundry.rs"
fi
if [[ ! -f "$candidate" ]]; then
  echo "missing Sacred Foundry source: $candidate" >&2
  exit 1
fi

echo "Sacred Foundry source present; isolated Harbor scoring follows submission."
