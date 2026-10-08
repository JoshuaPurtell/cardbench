#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/induce_paranoia.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Induce Paranoia source: ${candidate}" >&2
  exit 1
fi
echo "Induce Paranoia module present; isolated Harbor scoring follows submission."
