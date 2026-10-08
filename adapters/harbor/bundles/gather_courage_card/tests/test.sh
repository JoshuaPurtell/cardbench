#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/gather_courage.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Gather Courage source: ${candidate}" >&2
  exit 1
fi

echo "Gather Courage module present; isolated Harbor scoring follows submission."
