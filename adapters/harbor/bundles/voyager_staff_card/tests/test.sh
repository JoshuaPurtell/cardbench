#!/usr/bin/env bash
set -euo pipefail
python3 varieties/magic/scripts/materialize_voyager_staff_task.py --workspace /tmp/rav-voyager-staff-task-workspace --manifest /tmp/rav-voyager-staff-task-workspace-manifest.json
python3 varieties/magic/scripts/materialize_voyager_staff_task.py --workspace /tmp/rav-voyager-staff-task-workspace --manifest /tmp/rav-voyager-staff-task-workspace-manifest.json --verify
python3 varieties/magic/scripts/score_voyager_staff_pvp.py --help
