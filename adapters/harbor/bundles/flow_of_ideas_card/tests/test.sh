#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Flow of Ideas source: ${candidate}" >&2
  exit 1
fi

echo "Flow of Ideas source present; isolated Harbor scoring follows submission."
