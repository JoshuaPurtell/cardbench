#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/dromad_purebred.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Dromad Purebred source: ${candidate}" >&2
  exit 1
fi

echo "Dromad Purebred source present; isolated Harbor scoring follows submission."
