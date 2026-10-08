#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/spectral_searchlight.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Spectral Searchlight source: ${candidate}" >&2
  exit 1
fi

echo "Spectral Searchlight source present; isolated Harbor scoring follows submission."
