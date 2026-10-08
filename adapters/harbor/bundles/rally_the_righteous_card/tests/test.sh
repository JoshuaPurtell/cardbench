#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/rally_the_righteous.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Rally the Righteous source: ${candidate}" >&2
  exit 1
fi

echo "Rally the Righteous module present; isolated Harbor scoring follows submission."
