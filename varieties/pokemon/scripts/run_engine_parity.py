#!/usr/bin/env python3
"""P1 engine substrate check; parity promotion remains a separate gate."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

POKEMON_ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()
    args.output_root.mkdir(parents=True, exist_ok=True)
    completed = subprocess.run(
        ["cargo", "check", "--workspace"],
        cwd=POKEMON_ROOT / "engine",
        text=True,
        capture_output=True,
    )
    result = {
        "schema_version": "cardbench.engine.check.v1",
        "task_id": "cardbench/pokemon/engine",
        "compile_passed": completed.returncode == 0,
        "event_log_parity_run": False,
        "passed": completed.returncode == 0,
        "promotion_status": "scaffold",
        "stderr": completed.stderr[-4000:],
    }
    (args.output_root / "engine-check.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
