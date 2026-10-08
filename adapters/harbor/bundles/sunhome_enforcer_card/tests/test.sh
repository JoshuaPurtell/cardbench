#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sunhome_enforcer.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Sunhome Enforcer source: ${candidate}" >&2
  exit 1
fi

echo "Sunhome Enforcer source present; isolated Harbor scoring follows submission."
