#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/concerted_effort.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Concerted Effort source: ${candidate}" >&2
  exit 1
fi

echo "Concerted Effort candidate module present; isolated task scoring follows submission."
