#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/ordruun_commando.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Ordruun Commando source: ${candidate}" >&2
  exit 1
fi
echo "Ordruun Commando module present; isolated Harbor scoring follows submission."
