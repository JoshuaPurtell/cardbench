#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sparkmage_apprentice.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Sparkmage Apprentice source: ${candidate}" >&2
  exit 1
fi

echo "Sparkmage Apprentice module present; isolated Harbor scoring follows submission."
