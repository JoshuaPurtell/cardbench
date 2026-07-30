use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

const SET_CODE: &str = "CG";
const CRITICAL_TRAINERS: &[&str] = &[
    "CG-71",
    "CG-73",
    "CG-77",
    "CG-78",
    "CG-82",
    "CG-84", // Warp Point
    "CG-86",
    "CG-87",
];
// Critical attacks that must never be NoOp - these have complex effects that must be properly implemented
const CRITICAL_ATTACKS: &[(&str, &str)] = &[
    ("CG-28", "Toxic Sleep"), // Venusaur: Must apply Asleep+Poisoned and add marker for extra poison damage
];

fn load_jsonl(path: &str) -> Result<HashMap<String, String>, String> {
    let file = File::open(path).map_err(|err| format!("Failed to open {path}: {err}"))?;
    let reader = BufReader::new(file);
    let mut cards = HashMap::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|err| format!("Failed to read line {index}: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|err| format!("Invalid JSON line {index}: {err}"))?;
        let number = value
            .get("number")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Missing number in line {index}"))?;
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Missing name in line {index}"))?;
        let name = normalize_name(name);
        let def_id = format!("{SET_CODE}-{number}");
        cards.insert(def_id, name.to_string());
    }
    Ok(cards)
}

fn load_jsonl_cards(path: &str) -> Result<Vec<Value>, String> {
    let file = File::open(path).map_err(|err| format!("Failed to open {path}: {err}"))?;
    let reader = BufReader::new(file);
    let mut cards = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|err| format!("Failed to read line {index}: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|err| format!("Invalid JSON line {index}: {err}"))?;
        cards.push(value);
    }
    Ok(cards)
}

fn effect_is_noop(effect: &str) -> bool {
    let value: Value = match serde_json::from_str(effect) {
        Ok(value) => value,
        Err(_) => return false,
    };
    value
        .get("op")
        .and_then(Value::as_str)
        .map(|op| op == "NoOp")
        .unwrap_or(false)
}

fn validate_non_noop_effects(conn: &Connection, cards: &[Value]) -> Result<(), String> {
    for card in cards {
        let number = card
            .get("number")
            .and_then(Value::as_str)
            .ok_or("Missing number in card JSON")?;
        let def_id = format!("{SET_CODE}-{number}");
        let supertype = card
            .get("supertype")
            .and_then(Value::as_str)
            .unwrap_or("Trainer");

        if supertype == "Pokémon" || supertype == "Pokemon" {
            if let Some(attacks) = card.get("attacks").and_then(Value::as_array) {
                for attack in attacks {
                    let text = attack.get("text").and_then(Value::as_str).unwrap_or("").trim();
                    if text.is_empty() {
                        continue;
                    }
                    let name = attack
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or("Missing attack name")?;
                    let effect: String = conn
                        .query_row(
                            "SELECT effect_ast FROM attacks WHERE card_def_id = ? AND name = ?",
                            params![def_id, name],
                            |row| row.get(0),
                        )
                        .map_err(|err| format!("Missing attack {def_id} {name}: {err}"))?;
                    if effect_is_noop(&effect) {
                        return Err(format!(
                            "Attack {def_id} {name} has text but still NoOp effect_ast."
                        ));
                    }
                }
            }
            if let Some(powers) = card.get("abilities").and_then(Value::as_array) {
                for power in powers {
                    let name = power
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or("Missing power name")?;
                    let effect: String = conn
                        .query_row(
                            "SELECT effect_ast FROM powers WHERE card_def_id = ? AND name = ?",
                            params![def_id, name],
                            |row| row.get(0),
                        )
                        .map_err(|err| format!("Missing power {def_id} {name}: {err}"))?;
                    if effect_is_noop(&effect) {
                        return Err(format!(
                            "Power {def_id} {name} has text but still NoOp effect_ast."
                        ));
                    }
                }
            }
        } else if supertype == "Trainer" {
            let effect: String = conn
                .query_row(
                    "SELECT script_payload FROM cards WHERE card_def_id = ?",
                    params![def_id],
                    |row| row.get(0),
                )
                .map_err(|err| format!("Missing trainer {def_id}: {err}"))?;
            if effect_is_noop(&effect) {
                return Err(format!(
                    "Trainer {def_id} has rules text but still NoOp effect_ast."
                ));
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err(format!("Usage: {} <db_path> <cg_jsonl>", args[0]));
    }
    let db_path = &args[1];
    let jsonl_path = &args[2];

    let conn = Connection::open(db_path).map_err(|err| err.to_string())?;
    let expected = load_jsonl(jsonl_path)?;
    let cards = load_jsonl_cards(jsonl_path)?;

    let mut stmt = conn
        .prepare("SELECT card_def_id, name FROM cards WHERE card_def_id LIKE ?")
        .map_err(|err| err.to_string())?;
    let mut rows = stmt
        .query(params![format!("{SET_CODE}-%")])
        .map_err(|err| err.to_string())?;
    let mut found = HashMap::new();
    while let Some(row) = rows.next().map_err(|err| err.to_string())? {
        let def_id: String = row.get(0).map_err(|err| err.to_string())?;
        let name: String = row.get(1).map_err(|err| err.to_string())?;
        found.insert(def_id, normalize_name(&name));
    }

    let mut missing = Vec::new();
    let mut name_mismatch = Vec::new();
    for (def_id, name) in expected.iter() {
        match found.get(def_id) {
            None => missing.push(def_id.clone()),
            Some(db_name) if db_name != name => {
                name_mismatch.push((def_id.clone(), name.clone(), db_name.clone()));
            }
            _ => {}
        }
    }

    if !missing.is_empty() {
        return Err(format!("Missing {SET_CODE} cards in DB: {missing:?}"));
    }
    if !name_mismatch.is_empty() {
        return Err(format!(
            "Name mismatches for {SET_CODE}: {name_mismatch:?}"
        ));
    }

    for def_id in CRITICAL_TRAINERS {
        let payload: String = conn
            .query_row(
                "SELECT script_payload FROM cards WHERE card_def_id = ?",
                params![def_id],
                |row| row.get(0),
            )
            .map_err(|err| format!("Missing {def_id}: {err}"))?;
        if payload.contains("NoOp") {
            return Err(format!("Trainer {def_id} still has NoOp script_payload."));
        }
    }

    validate_non_noop_effects(&conn, &cards)?;

    // Validate critical attacks are never NoOp
    for (def_id, attack_name) in CRITICAL_ATTACKS {
        let effect: String = conn
            .query_row(
                "SELECT effect_ast FROM attacks WHERE card_def_id = ? AND name = ?",
                params![def_id, attack_name],
                |row| row.get(0),
            )
            .map_err(|err| format!("Missing attack {def_id} {attack_name}: {err}"))?;
        if effect.contains("\"op\":\"NoOp\"") || effect == "{\"op\":\"NoOp\"}" {
            return Err(format!(
                "Critical attack {def_id} {attack_name} still has NoOp effect_ast. This attack must have proper effect implementation."
            ));
        }
    }

    expect_attack_effect(
        &conn,
        "CG-23",
        "Surprise",
        serde_json::json!({
            "op": "ReturnRandomCardFromHandToDeck",
            "player": "Opponent",
            "reveal": true,
            "shuffle": true,
            "notes": "CG-23:Surprise"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-23",
        "Bass Control",
        serde_json::json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "DealDamage",
                "target": "Selected",
                "amount": 40
            },
            "notes": "CG-23:Bass Control"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-28",
        "Green Blast",
        serde_json::json!({
            "op": "DealDamageByEnergyInPlay",
            "player": "Current",
            "energy_type": "Grass",
            "target": "OppActive",
            "per_energy": 10,
            "notes": "CG-28:Green Blast"
        }),
    )?;
    // Venusaur (CG-28) "Toxic Sleep": Critical attack that must apply Asleep+Poisoned and add marker
    // for extra poison damage between turns. This is validated both here and in CRITICAL_ATTACKS above.
    expect_attack_effect(
        &conn,
        "CG-28",
        "Toxic Sleep",
        serde_json::json!({
            "op": "Sequence",
            "effects": [
                {
                    "op": "ApplySpecialCondition",
                    "target": "OppActive",
                    "condition": "Asleep"
                },
                {
                    "op": "ApplySpecialCondition",
                    "target": "OppActive",
                    "condition": "Poisoned"
                },
                {
                    "op": "AddMarker",
                    "target": "OppActive",
                    "name": "Toxic Sleep"
                }
            ],
            "notes": "CG-28:Toxic Sleep"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-32",
        "Detect",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "AddMarker",
                "target": "SelfActive",
                "name": "Detect",
                "expires_after_turns": 1
            },
            "on_tails": { "op": "NoOp" },
            "notes": "CG-32:Detect"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-33",
        "Amnesia",
        serde_json::json!({
            "op": "ChooseDefenderAttack",
            "target": "OppActive",
            "notes": "CG-33:Amnesia"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-34",
        "Sleep Powder",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": "CG-34:Sleep Powder"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-45",
        "Poisonpowder",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Poisoned",
            "notes": "CG-45:Poisonpowder"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-53",
        "Hypnoblast",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": "CG-53:Hypnoblast"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-67",
        "Paralyzing Gaze",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "ApplySpecialCondition",
                "target": "OppActive",
                "condition": "Paralyzed"
            },
            "on_tails": { "op": "NoOp" },
            "notes": "CG-67:Paralyzing Gaze"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-69",
        "Supersonic",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "ApplySpecialCondition",
                "target": "OppActive",
                "condition": "Confused"
            },
            "on_tails": { "op": "NoOp" },
            "notes": "CG-69:Supersonic"
        }),
    )?;

    // Sceptile deck attacks
    expect_attack_effect(
        &conn,
        "CG-68",
        "Shining Claws",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "on_tails": { "op": "NoOp" },
            "notes": "CG-68:Shining Claws"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-19",
        "Agility",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "AddMarker", "target": "SelfActive", "name": "PreventAllDamageAndEffects", "expires_after_turns": 1 },
            "on_tails": { "op": "NoOp" },
            "notes": "CG-19:Agility"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "CG-96",
        "Power Revenge",
        serde_json::json!({
            "op": "DealDamageByOpponentPrizesTaken",
            "target": "OppActive",
            "base": 60,
            "per_prize": 10,
            "notes": "CG-96:Power Revenge"
        }),
    )?;

    expect_power_effect(
        &conn,
        "CG-28",
        "Chlorophyll",
        serde_json::json!({
            "op": "Custom",
            "id": "CG-28:Chlorophyll",
            "set": "CG",
            "kind": "PokeBody",
            "name": "Chlorophyll",
            "card": "Venusaur",
            "notes": "Grass Pokemon treat Colorless Energy as Grass."
        }),
    )?;

    // Trainer effect validations
    expect_trainer_effect(
        &conn,
        "CG-84",
        serde_json::json!({
            "op": "Sequence",
            "effects": [
                {
                    "op": "ChoosePokemonTargets",
                    "player": "Opponent",
                    "selector": { "scope": "OppBench" },
                    "min": 1,
                    "max": 1,
                    "effect": { "op": "SwitchToTarget", "player": "Opponent" }
                },
                {
                    "op": "ChoosePokemonTargets",
                    "player": "Current",
                    "selector": { "scope": "SelfBench" },
                    "min": 1,
                    "max": 1,
                    "effect": { "op": "SwitchToTarget", "player": "Current" }
                }
            ],
            "notes": "CG-84:Warp Point"
        }),
    )?;

    let energy_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM cards WHERE card_def_id = 'ENERGY-GRASS'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| err.to_string())?;
    if energy_count != 1 {
        return Err("ENERGY-GRASS missing from DB.".to_string());
    }

    println!("CG card validation passed.");
    Ok(())
}

fn normalize_name(name: &str) -> String {
    let normalized = name
        .replace('δ', "Delta")
        .replace("Pokémon", "Pokemon")
        .replace('é', "e");
    normalized
        .trim()
        .strip_suffix(" Delta")
        .unwrap_or(normalized.trim())
        .to_string()
}

fn expect_attack_effect(
    conn: &Connection,
    def_id: &str,
    attack_name: &str,
    expected: Value,
) -> Result<(), String> {
    let effect: String = conn
        .query_row(
            "SELECT effect_ast FROM attacks WHERE card_def_id = ? AND name = ?",
            params![def_id, attack_name],
            |row| row.get(0),
        )
        .map_err(|err| format!("Missing attack {def_id} {attack_name}: {err}"))?;
    let value: Value =
        serde_json::from_str(&effect).map_err(|err| format!("{def_id} {attack_name} invalid JSON: {err}"))?;
    if value != expected {
        return Err(format!(
            "{def_id} {attack_name} effect_ast mismatch. expected={expected} actual={value}"
        ));
    }
    Ok(())
}

fn expect_power_effect(
    conn: &Connection,
    def_id: &str,
    power_name: &str,
    expected: Value,
) -> Result<(), String> {
    let effect: String = conn
        .query_row(
            "SELECT effect_ast FROM powers WHERE card_def_id = ? AND name = ?",
            params![def_id, power_name],
            |row| row.get(0),
        )
        .map_err(|err| format!("Missing power {def_id} {power_name}: {err}"))?;
    let value: Value =
        serde_json::from_str(&effect).map_err(|err| format!("{def_id} {power_name} invalid JSON: {err}"))?;
    if value != expected {
        return Err(format!(
            "{def_id} {power_name} effect_ast mismatch. expected={expected} actual={value}"
        ));
    }
    Ok(())
}

fn expect_trainer_effect(conn: &Connection, def_id: &str, expected: Value) -> Result<(), String> {
    let effect: String = conn
        .query_row(
            "SELECT script_payload FROM cards WHERE card_def_id = ?",
            params![def_id],
            |row| row.get(0),
        )
        .map_err(|err| format!("Missing trainer {def_id}: {err}"))?;
    let value: Value =
        serde_json::from_str(&effect).map_err(|err| format!("{def_id} invalid JSON: {err}"))?;
    if value != expected {
        return Err(format!(
            "{def_id} script_payload mismatch.\nexpected={expected}\nactual={value}"
        ));
    }
    Ok(())
}
