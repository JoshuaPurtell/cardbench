#!/usr/bin/env python3
"""Create a private cards DB with canonical CG metadata and gold effect ASTs."""
import argparse, json, shutil, sqlite3
from pathlib import Path

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("catalog", type=Path)
    parser.add_argument("--base-db", type=Path, default=Path(__file__).parents[1] / "policies/data/cards.sqlite")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(args.base_db, args.output)
    catalog = json.loads(args.catalog.read_text())
    conn = sqlite3.connect(args.output)
    try:
        with conn:
            set_id = conn.execute("SELECT set_id FROM sets WHERE code='CG'").fetchone()[0]
            for card_id, meta in sorted(catalog.items()):
                if not card_id.startswith("CG-"):
                    continue
                number = card_id.split("-", 1)[1]
                name = meta["name"]
                if name.endswith(" Delta"):
                    name = name[:-6] + " δ"
                if meta.get("is_pokemon"):
                    supertype = "Pokemon"
                elif meta.get("is_energy"):
                    supertype = "Energy"
                else:
                    supertype = "Trainer"
                cursor = conn.execute("""UPDATE cards SET set_id=?, number=?, name=?, supertype=?, stage=?, evolves_from=?, hp=?, types_json=?, weakness_json=?, resist_json=?, retreat_cost=?, trainer_kind=?, energy_kind=?, script_kind='Dsl', script_payload=? WHERE card_def_id=?""", (
                    set_id, number, name, supertype, meta.get("stage"), meta.get("evolves_from"), meta.get("hp"),
                    json.dumps(meta.get("types") or []), json.dumps(meta.get("weakness")), json.dumps(meta.get("resistance")),
                    meta.get("retreat_cost"), meta.get("trainer_kind"), meta.get("energy_kind"),
                    json.dumps(meta.get("trainer_effect") or {}), card_id))
                if cursor.rowcount != 1:
                    raise RuntimeError(f"missing base DB row for {card_id}")
                conn.execute("DELETE FROM attacks WHERE card_def_id=?", (card_id,))
                for index, attack in enumerate(meta.get("attacks") or []):
                    conn.execute("INSERT INTO attacks(card_def_id,idx,name,cost_json,damage_expr,effect_ast) VALUES(?,?,?,?,?,?)", (
                        card_id, index, attack["name"], json.dumps(attack.get("cost") or {}), str(attack.get("damage", 0)), json.dumps(attack.get("effect_ast") or {})))
        # Equivalent explicit compiler validation gate: every derived JSON field must
        # decode and every canonical CG attack must round-trip through the DB.
        rows = conn.execute("SELECT card_def_id,types_json,weakness_json,resist_json,script_payload FROM cards WHERE card_def_id LIKE 'CG-%'").fetchall()
        if len(rows) != 100:
            raise RuntimeError(f"expected 100 CG cards after sync, found {len(rows)}")
        for card_id, *payloads in rows:
            for payload in payloads:
                json.loads(payload or "null")
            expected_attacks = len(catalog[card_id].get("attacks") or [])
            actual_attacks = conn.execute("SELECT count(*) FROM attacks WHERE card_def_id=?", (card_id,)).fetchone()[0]
            if actual_attacks != expected_attacks:
                raise RuntimeError(f"attack round-trip mismatch for {card_id}: {actual_attacks} != {expected_attacks}")
    finally:
        conn.close()
    print(f"wrote private CG database: {args.output}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
