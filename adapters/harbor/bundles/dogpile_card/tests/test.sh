#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/dogpile.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Dogpile source: ${candidate}" >&2
  exit 1
fi

echo "Dogpile source present; isolated Harbor scoring follows submission."
