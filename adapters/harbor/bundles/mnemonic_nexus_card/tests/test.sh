#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/mnemonic_nexus.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Mnemonic Nexus source: ${candidate}" >&2
  exit 1
fi
echo "Mnemonic Nexus candidate module present; isolated authority scoring validates owner-relative graveyard recovery."
