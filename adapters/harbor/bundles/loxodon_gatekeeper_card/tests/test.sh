#!/bin/sh
set -eu
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_gatekeeper.rs}"
if [ ! -f "$candidate" ]; then
  echo "missing Loxodon Gatekeeper source: ${candidate}" >&2
  exit 1
fi
echo "Loxodon Gatekeeper source present; isolated Harbor scoring follows submission."
