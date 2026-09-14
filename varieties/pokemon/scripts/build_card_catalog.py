#!/usr/bin/env python3
"""Build the public card-suite catalog using hashes of sealed evaluator assets."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

POKEMON = Path(__file__).resolve().parents[1]
CARDS = POKEMON / "cards"


def digest(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sealed-root", type=Path, required=True)
    args = parser.parse_args()
    entries = []
    for path in sorted((CARDS / "instances").glob("*.json")):
        data = json.loads(path.read_text())
        module = Path(data["card_file"]).stem
        stub = CARDS / "stubs" / f"{module}.rs"
        gold = args.sealed_root / "pokemon" / "card" / "implementations" / f"{module}.rs"
        tests = args.sealed_root / "pokemon" / "card" / "tests" / f"{module}_eval.rs"
        entries.append(
            {
                "instance_id": data["id"],
                "expansion_id": data["expansion"],
                "card_id": data["cards"][0]["id"],
                "module": module,
                "target": f"scaffold/src/{'df' if data['expansion'] == 'dragon_frontiers' else 'hp'}/cards/{module}.rs",
                "stub_sha256": digest(stub),
                "sealed_gold_sha256": digest(gold),
                "sealed_tests_sha256": digest(tests),
                "visible_test_count": len(data.get("tests", [])),
                "status": "verified_assets" if gold.is_file() and tests.is_file() else "scaffold",
            }
        )
    payload = {
        "schema_version": "cardbench.card-suite.v1",
        "suite_id": "pokemon-ex-era-single-card-v1",
        "task_id": "cardbench/pokemon/card",
        "engine_pin": "821680873a607ad6dfbac9749f15a7c7505ff822",
        "sealed_namespace": "pokemon/card",
        "instances": entries,
    }
    (CARDS / "catalog.json").write_text(json.dumps(payload, indent=2) + "\n")
    print(f"wrote {len(entries)} card entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
