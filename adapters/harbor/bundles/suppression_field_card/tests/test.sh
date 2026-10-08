#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/suppression_field.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Suppression Field source: ${candidate}" >&2
  exit 1
fi

echo "Suppression Field module present; isolated Harbor scoring follows submission."
