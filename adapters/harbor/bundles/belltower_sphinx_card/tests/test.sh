#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/belltower_sphinx.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Belltower Sphinx source: ${candidate}" >&2
  exit 1
fi

echo "Belltower Sphinx module present; isolated Harbor scoring follows submission."
