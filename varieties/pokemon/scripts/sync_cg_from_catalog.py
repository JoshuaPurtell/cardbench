#!/usr/bin/env python3
"""Build a private cards.sqlite with CG metadata and gold ASTs from a private catalog."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any


POKEMON = Path(__file__).resolve().parents[1]
ROOT = POKEMON.parents[1]
ENGINE = POKEMON / "engine"
PUBLIC_DB = POKEMON / "policies" / "data" / "cards.sqlite"


def _tags(meta: dict[str, Any]) -> list[str]:
    tags = []
    if meta.get("is_delta") or meta.get("delta_species"):
        tags.append("DeltaSpecies")
    if meta.get("is_ex"):
        tags.append("PokemonEx")
    if meta.get("is_star"):
        tags.append("PokemonStar")
    return tags


def _weakness(value: dict[str, Any] | None) -> list[dict[str, str]]:
    if not value:
        return []
    return [{"type": value["type_"], "value": f"x{value['multiplier']}"}]


def _resistance(value: dict[str, Any] | None) -> list[dict[str, str]]:
    if not value:
        return []
    amount = int(value["value"])
    return [{"type": value["type_"], "value": str(-abs(amount))}]


def normalize(card_id: str, meta: dict[str, Any]) -> dict[str, Any]:
    number = card_id.split("-", 1)[1]
    name = meta["name"]
    if (meta.get("is_delta") or meta.get("delta_species")) and name.endswith(" Delta"):
        name = name.removesuffix(" Delta") + " δ"

    common: dict[str, Any] = {
        "set": "CG",
        "number": number,
        "name": name,
        "supertype": meta["card_type"],
        "tags": _tags(meta),
    }
    if meta["card_type"] == "Pokemon":
        common.update(
            stage=meta["stage"],
            evolves_from=meta.get("evolves_from"),
            hp=meta["hp"],
            types=meta["types"],
            weakness=_weakness(meta.get("weakness")),
            resistance=_resistance(meta.get("resistance")),
            retreat=meta["retreat_cost"],
            attacks=[
                {
                    "name": attack["name"],
                    "cost": attack.get("cost", {}).get("types", []),
                    "damage": attack.get("damage", 0),
                    "effect_ast": attack.get("effect_ast") or {},
                }
                for attack in meta.get("attacks", [])
            ],
        )
        if common["evolves_from"] is None:
            common.pop("evolves_from")
    elif meta["card_type"] == "Trainer":
        common.update(
            trainer_kind=meta["trainer_kind"],
            effect_ast=meta.get("trainer_effect") or {},
        )
    elif meta["card_type"] == "Energy":
        common.update(
            energy_kind=meta["energy_kind"],
            provides=meta.get("provides", []),
            effect_ast=meta.get("trainer_effect") or {},
        )
    else:
        raise ValueError(f"unsupported card type for {card_id}: {meta['card_type']}")
    return common


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("catalog", type=Path, help="private CG catalog.json")
    parser.add_argument("output", type=Path, help="private output cards.sqlite")
    parser.add_argument("--base-db", type=Path, default=PUBLIC_DB)
    args = parser.parse_args()

    base = args.base_db.resolve()
    output = args.output.resolve()
    if output == base:
        raise SystemExit("refusing to overwrite the public cards.sqlite; choose a private output")

    catalog = json.loads(args.catalog.read_text())
    cards = [normalize(card_id, catalog[card_id]) for card_id in sorted(catalog) if card_id.startswith("CG-")]
    if len(cards) != 100:
        raise SystemExit(f"expected 100 CG cards, found {len(cards)}")

    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(base, output)
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", prefix="cg-normalized-", dir=output.parent, delete=False
    ) as handle:
        normalized_path = Path(handle.name)
        json.dump(cards, handle)

    try:
        subprocess.run(
            [
                "cargo",
                "run",
                "--manifest-path",
                str(ENGINE / "Cargo.toml"),
                "--package",
                "tcg_db",
                "--bin",
                "sync_cg_catalog",
                "--",
                str(normalized_path),
                str(output),
            ],
            check=True,
        )
    finally:
        normalized_path.unlink(missing_ok=True)

    tracked = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--error-unmatch", str(output)],
        capture_output=True,
    )
    if tracked.returncode == 0:
        output.unlink(missing_ok=True)
        raise SystemExit("refusing to leave a private synchronized DB at a tracked path")
    print(f"wrote private CG database: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
