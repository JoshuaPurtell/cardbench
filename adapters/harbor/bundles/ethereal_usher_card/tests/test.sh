#!/bin/sh
set -eu
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs}"
if [ ! -f "$candidate" ]; then
  echo "missing Ethereal Usher candidate source: ${candidate}" >&2
  exit 1
fi
echo "Ethereal Usher source present; isolated Harbor scoring follows submission."
