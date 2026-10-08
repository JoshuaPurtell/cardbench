#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/devouring_light.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Devouring Light source: ${candidate}" >&2
  exit 1
fi

echo "Devouring Light source present; isolated Harbor scoring follows submission."
