#!/usr/bin/env bash
set -euo pipefail
python3 varieties/magic/scripts/materialize_indentured_oaf_task.py --workspace /tmp/rav-indentured-oaf-task-workspace --manifest /tmp/rav-indentured-oaf-task-workspace-manifest.json
python3 varieties/magic/scripts/materialize_indentured_oaf_task.py --workspace /tmp/rav-indentured-oaf-task-workspace --manifest /tmp/rav-indentured-oaf-task-workspace-manifest.json --verify
python3 varieties/magic/scripts/score_indentured_oaf_pvp.py --help
