#!/usr/bin/env bash
set -euo pipefail

WORKDIR="/app"

cp /solution/import_specs.rs "$WORKDIR/scaffold/src/df/import_specs.rs"
cp /solution/runtime.rs "$WORKDIR/scaffold/src/df/runtime.rs"

echo "SetBench oracle applied: Dragon Frontiers import_specs.rs + runtime.rs restored"
