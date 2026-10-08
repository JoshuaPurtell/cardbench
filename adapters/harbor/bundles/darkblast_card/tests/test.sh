#!/bin/sh
set -eu
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/darkblast.rs}"
if [ ! -f "$candidate" ]; then
  echo "missing Darkblast source: ${candidate}" >&2
  exit 1
fi
echo "Darkblast source present; isolated Harbor scoring follows submission."
