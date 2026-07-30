use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

const SET_CODE: &str = "DF";
const CRITICAL_TRAINERS: &[&str] = &[
    "DF-72",
    "DF-75", // Holon Mentor
    "DF-79",
    "DF-80", // Professor Oak's Research
    "DF-82",
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
        return Err(format!("Usage: {} <db_path> <df_jsonl>", args[0]));
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

    expect_attack_effect(
        &conn,
        "DF-2",
        "Drag Off",
        serde_json::json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppBench"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "SwitchToTarget",
                "player": "Opponent"
            },
            "notes": "DF-2:Drag Off"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-12",
        "Burning Ball",
        serde_json::json!({
            "op": "ApplySpecialConditionIfEnergyAttached",
            "target": "OppActive",
            "energy_type": "Fire",
            "count": 2,
            "condition": "Burned",
            "notes": "DF-12:Burning Ball"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-13",
        "Burning Venom",
        serde_json::json!({
            "op": "Sequence",
            "effects": [
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Burned" },
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" }
            ],
            "notes": "DF-13:Burning Venom"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-13",
        "Strangle",
        serde_json::json!({
            "op": "DealDamageIfTargetDelta",
            "target": "OppActive",
            "amount": 30,
            "notes": "DF-13:Strangle"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-14",
        "Grind",
        serde_json::json!({
            "op": "DealDamageByAttachedEnergy",
            "target": "OppActive",
            "per_energy": 10,
            "notes": "DF-14:Grind"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-19",
        "Lap Up",
        serde_json::json!({
            "op": "DrawCards",
            "player": "Current",
            "count": 2,
            "notes": "DF-19:Lap Up"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-19",
        "Delta Mind",
        serde_json::json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "PlaceDamageCountersIfTargetDelta",
                "target": "Selected",
                "base": 1,
                "delta": 3
            },
            "notes": "DF-19:Delta Mind"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-20",
        "Spiral Drain",
        serde_json::json!({
            "op": "HealDamage",
            "target": "SelfActive",
            "amount": 10,
            "notes": "DF-20:Spiral Drain"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-22",
        "Smokescreen",
        serde_json::json!({
            "op": "AddMarker",
            "target": "OppActive",
            "name": "Smokescreen",
            "expires_after_turns": 1,
            "notes": "DF-22:Smokescreen"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-24",
        "Quick Blow",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DealDamage", "target": "OppActive", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": "DF-24:Quick Blow"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-27",
        "Scary Face",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "Sequence",
                "effects": [
                    { "op": "AddMarker", "target": "OppActive", "name": "CannotAttack", "expires_after_turns": 1 },
                    { "op": "AddMarker", "target": "OppActive", "name": "CannotRetreat", "expires_after_turns": 1 }
                ]
            },
            "on_tails": { "op": "NoOp" },
            "notes": "DF-27:Scary Face"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-33",
        "Flickering Flames",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": "DF-33:Flickering Flames"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-36",
        "Quick Attack",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DealDamage", "target": "OppActive", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": "DF-36:Quick Attack"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-45",
        "Swift",
        serde_json::json!({
            "op": "DealDamageNoModifiers",
            "target": "OppActive",
            "amount": 30,
            "notes": "DF-45:Swift"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-50",
        "Sleepy Ball",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": "DF-50:Sleepy Ball"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-52",
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
            "notes": "DF-52:Paralyzing Gaze"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-59",
        "Hyper Beam",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DiscardAttachedEnergy", "target": "OppActive", "count": 1 },
            "on_tails": { "op": "NoOp" },
            "notes": "DF-59:Hyper Beam"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-61",
        "Calm Mind",
        serde_json::json!({
            "op": "HealDamage",
            "target": "SelfActive",
            "amount": 20,
            "notes": "DF-61:Calm Mind"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-63",
        "Shell Grab",
        serde_json::json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "ApplySpecialCondition",
                "target": "OppActive",
                "condition": "Paralyzed"
            },
            "on_tails": { "op": "NoOp" },
            "notes": "DF-63:Shell Grab"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-67",
        "Rage",
        serde_json::json!({
            "op": "DealDamageByAttackerDamage",
            "target": "OppActive",
            "base": 10,
            "per_counter": 10,
            "notes": "DF-67:Rage"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-70",
        "Hypnotic Gaze",
        serde_json::json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": "DF-70:Hypnotic Gaze"
        }),
    )?;

    // Sceptile deck attacks
    expect_attack_effect(
        &conn,
        "DF-3",
        "Dig Deep",
        serde_json::json!({
            "op": "SearchDiscardWithSelector",
            "player": "Current",
            "selector": { "is_energy": true },
            "count": 1,
            "min": 0,
            "max": 1,
            "destination": "Hand",
            "notes": "DF-3:Dig Deep"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-3",
        "Extra Claws",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-3:Extra Claws",
            "set": "DF",
            "notes": "30 damage + 20 if target is Pokemon-ex"
        }),
    )?;
    expect_attack_effect(
        &conn,
        "DF-93",
        "Flame Ball",
        serde_json::json!({
            "op": "MoveAttachedEnergy",
            "source": "SelfActive",
            "target_selector": { "is_active": false },
            "energy_type": "Fire",
            "count": 1,
            "min": 0,
            "optional": true
        }),
    )?;

    expect_power_effect(
        &conn,
        "DF-2",
        "Battle Aura",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-2:Battle Aura",
            "set": "DF",
            "kind": "PokeBody",
            "name": "Battle Aura",
            "card": "Feraligatr Delta",
            "notes": "Your Delta Pokemon attacks do +10 damage."
        }),
    )?;
    expect_power_effect(
        &conn,
        "DF-12",
        "Shady Move",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-12:Shady Move",
            "set": "DF",
            "kind": "PokePower",
            "name": "Shady Move",
            "card": "Typhlosion Delta",
            "notes": "Active only; move 1 counter between any Pokemon."
        }),
    )?;
    expect_power_effect(
        &conn,
        "DF-14",
        "Solid Shell",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-14:Solid Shell",
            "set": "DF",
            "kind": "PokeBody",
            "name": "Solid Shell",
            "card": "Cloyster Delta",
            "notes": "Prevent effects of attacks to your Benched Delta Pokemon."
        }),
    )?;
    expect_power_effect(
        &conn,
        "DF-20",
        "Power Circulation",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-20:Power Circulation",
            "set": "DF",
            "kind": "PokePower",
            "name": "Power Circulation",
            "card": "Mantine Delta",
            "notes": "Put basic Energy from discard on top of deck, then damage self."
        }),
    )?;
    expect_power_effect(
        &conn,
        "DF-24",
        "Psychic Wing",
        serde_json::json!({
            "op": "Custom",
            "id": "DF-24:Psychic Wing",
            "set": "DF",
            "kind": "PokeBody",
            "name": "Psychic Wing",
            "card": "Vibrava Delta",
            "notes": "If Psychic Energy attached, retreat cost is 0."
        }),
    )?;

    // Trainer effect validations
    expect_trainer_effect(
        &conn,
        "DF-75",
        serde_json::json!({
            "op": "IfHandNotEmpty",
            "player": "Current",
            "effect": {
                "op": "Sequence",
                "effects": [
                    { "op": "DiscardFromHand", "player": "Current", "count": 1 },
                    {
                        "op": "SearchDeckWithSelector",
                        "player": "Current",
                        "selector": { "is_basic": true, "is_pokemon": true, "max_hp": 100 },
                        "min": 0,
                        "max": 3,
                        "count": 3,
                        "destination": "Hand",
                        "reveal": true,
                        "shuffle": true
                    }
                ]
            },
            "notes": "DF-75:Holon Mentor"
        }),
    )?;
    expect_trainer_effect(
        &conn,
        "DF-80",
        serde_json::json!({
            "op": "Sequence",
            "effects": [
                { "op": "ShuffleHandIntoDeck", "player": "Current" },
                { "op": "DrawCards", "player": "Current", "count": 5 }
            ],
            "notes": "DF-80:Professor Oaks Research"
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

    validate_non_noop_effects(&conn, &cards)?;

    println!("DF card validation passed.");
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
