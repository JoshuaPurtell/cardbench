#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Halcyon Glaze source: ${candidate}" >&2
  exit 1
fi

echo "Halcyon Glaze source present; isolated scorer runs after submission."
