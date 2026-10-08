#!/bin/sh
set -eu

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/blockbuster.rs}"
if [ ! -f "$candidate" ]; then
  echo "missing Blockbuster candidate source: $candidate" >&2
  exit 1
fi
echo "Blockbuster candidate module present; isolated Harbor scoring follows submission."
