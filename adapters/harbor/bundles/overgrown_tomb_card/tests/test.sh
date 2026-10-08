#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/overgrown_tomb.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Overgrown Tomb source: ${candidate}" >&2
  exit 1
fi

echo "Overgrown Tomb source present; isolated Harbor scoring follows submission."
