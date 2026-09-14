#!/usr/bin/env python3
"""Build the public Pokémon catalog without embedding private effect ASTs."""
import argparse, json, sqlite3
from pathlib import Path

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--implemented-set", action="append", default=["CG", "DF"])
    args = parser.parse_args()
    conn = sqlite3.connect(args.db)
    conn.row_factory = sqlite3.Row
    rows = conn.execute("""SELECT c.card_def_id,c.name,c.supertype,c.stage,c.hp,c.types_json,c.trainer_kind,c.energy_kind,s.code AS set_code FROM cards c JOIN sets s ON s.set_id=c.set_id ORDER BY c.card_def_id""").fetchall()
    cards = []
    for row in rows:
        implemented = row["set_code"] in args.implemented_set or row["card_def_id"].startswith("ENERGY-")
        cards.append({"card_def_id": row["card_def_id"], "name": row["name"], "supertype": row["supertype"], "stage": row["stage"], "hp": row["hp"], "types": json.loads(row["types_json"] or "[]"), "trainer_kind": row["trainer_kind"], "energy_kind": row["energy_kind"], "implemented": implemented and row["card_def_id"] != "ENERGY-COLORLESS"})
    conn.close()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"schema_version": "cardbench.pokemon-catalog.v1", "cards": cards}, indent=2) + "\n")
    print(f"wrote {len(cards)} cards: {args.output}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
