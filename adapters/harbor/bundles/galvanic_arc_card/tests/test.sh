#!/usr/bin/env bash
set -euo pipefail
python3 varieties/magic/scripts/materialize_galvanic_arc_task.py --workspace /tmp/rav-galvanic-arc-task-workspace --manifest /tmp/rav-galvanic-arc-task-workspace-manifest.json
python3 varieties/magic/scripts/materialize_galvanic_arc_task.py --workspace /tmp/rav-galvanic-arc-task-workspace --manifest /tmp/rav-galvanic-arc-task-workspace-manifest.json --verify
python3 varieties/magic/scripts/score_galvanic_arc_pvp.py --help
