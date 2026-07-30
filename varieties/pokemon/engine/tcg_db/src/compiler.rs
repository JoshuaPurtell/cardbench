use jsonschema::{Draft, JSONSchema};
use rusqlite::{params, Connection};
use serde_json::Value;
use thiserror::Error;

static CARD_SCHEMA_JSON: &str = include_str!("../card_schema.json");

#[derive(Debug, Error)]
pub enum CardCompilerError {
    #[error("schema validation failed: {0}")]
    SchemaValidation(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("invalid field type: {0}")]
    InvalidField(&'static str),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("schema parse error: {0}")]
    SchemaParse(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn validate_card_schema(card_json: &Value) -> Result<(), CardCompilerError> {
    let schema_json: Value =
        serde_json::from_str(CARD_SCHEMA_JSON).map_err(CardCompilerError::Json)?;
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema_json)
        .map_err(|err| CardCompilerError::SchemaParse(err.to_string()))?;

    if let Err(errors) = compiled.validate(card_json) {
        let mut messages = Vec::new();
        for error in errors {
            messages.push(error.to_string());
        }
        return Err(CardCompilerError::SchemaValidation(messages.join("; ")));
    }
    Ok(())
}

pub fn compile_card(conn: &Connection, card_json: &Value) -> Result<String, CardCompilerError> {
    validate_card_schema(card_json)?;

    let set = get_string(card_json, "set")?;
    let number = get_string(card_json, "number")?;
    let name = get_string(card_json, "name")?;
    let supertype = get_string(card_json, "supertype")?;
    let tags_json = serialize_json(card_json.get("tags"))?;

    conn.execute(
        "INSERT OR IGNORE INTO sets (code, name, era) VALUES (?1, ?2, 'EX')",
        params![set, set],
    )?;
    let set_id: i64 = conn.query_row(
        "SELECT set_id FROM sets WHERE code = ?1",
        params![set],
        |row| row.get(0),
    )?;

    let card_def_id = format!("{set}-{number}");
    let subtypes_json = "[]".to_string();
    let mut stage: Option<String> = None;
    let mut evolves_from: Option<String> = None;
    let mut hp: Option<i64> = None;
    let mut types_json: Option<String> = None;
    let mut weakness_json: Option<String> = None;
    let mut resist_json: Option<String> = None;
    let mut retreat_cost: Option<i64> = None;
    let mut trainer_kind: Option<String> = None;
    let mut script_payload: String = "{}".to_string();
    let mut energy_kind: Option<String> = None;

    match supertype {
        "Pokemon" => {
            stage = Some(get_string(card_json, "stage")?.to_string());
            evolves_from = card_json
                .get("evolves_from")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            hp = Some(get_i64(card_json, "hp")?);
            types_json = Some(serialize_json(card_json.get("types"))?);
            weakness_json = Some(serialize_json(card_json.get("weakness"))?);
            resist_json = Some(serialize_json(card_json.get("resistance"))?);
            retreat_cost = Some(get_i64(card_json, "retreat")?);
        }
        "Trainer" => {
            trainer_kind = Some(get_string(card_json, "trainer_kind")?.to_string());
            script_payload = serialize_json(card_json.get("effect_ast"))?;
        }
        "Energy" => {
            energy_kind = Some(get_string(card_json, "energy_kind")?.to_string());
            script_payload = serialize_json(card_json.get("effect_ast"))?;
        }
        _ => return Err(CardCompilerError::InvalidField("supertype")),
    }

    conn.execute(
        "INSERT INTO cards (
            card_def_id, set_id, number, name, supertype, subtypes_json, tags_json,
            stage, evolves_from, hp, types_json, weakness_json, resist_json, retreat_cost,
            trainer_kind, energy_kind, script_kind, script_payload
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
            ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18
        )",
        params![
            card_def_id,
            set_id,
            number,
            name,
            supertype,
            subtypes_json,
            tags_json,
            stage,
            evolves_from,
            hp,
            types_json,
            weakness_json,
            resist_json,
            retreat_cost,
            trainer_kind,
            energy_kind,
            "Dsl",
            script_payload
        ],
    )?;

    if supertype == "Pokemon" {
        if let Some(attacks) = card_json.get("attacks").and_then(Value::as_array) {
            for (idx, attack) in attacks.iter().enumerate() {
                let attack_name = attack
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or(CardCompilerError::MissingField("attacks.name"))?;
                let cost_json = serialize_json(attack.get("cost"))?;
                let damage_expr = attack
                    .get("damage")
                    .map(|dmg| match dmg {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => s.clone(),
                        _ => "0".to_string(),
                    })
                    .unwrap_or_else(|| "0".to_string());
                let effect_ast = serialize_json(attack.get("effect_ast"))?;
                conn.execute(
                    "INSERT INTO attacks (
                        card_def_id, idx, name, cost_json, damage_expr, effect_ast
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        card_def_id,
                        idx as i64,
                        attack_name,
                        cost_json,
                        damage_expr,
                        effect_ast
                    ],
                )?;
            }
        }
        if let Some(powers) = card_json.get("powers").and_then(Value::as_array) {
            for (idx, power) in powers.iter().enumerate() {
                let kind = power
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or(CardCompilerError::MissingField("powers.kind"))?;
                let power_name = power
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or(CardCompilerError::MissingField("powers.name"))?;
                let effect_ast = serialize_json(power.get("effect_ast"))?;
                conn.execute(
                    "INSERT INTO powers (
                        card_def_id, idx, kind, name, effect_ast
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![card_def_id, idx as i64, kind, power_name, effect_ast],
                )?;
            }
        }
    }

    Ok(card_def_id)
}

fn get_string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, CardCompilerError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(CardCompilerError::MissingField(field))
}

fn get_i64(value: &Value, field: &'static str) -> Result<i64, CardCompilerError> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or(CardCompilerError::InvalidField(field))
}

fn serialize_json(value: Option<&Value>) -> Result<String, CardCompilerError> {
    match value {
        Some(val) => Ok(serde_json::to_string(val)?),
        None => Ok("[]".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_migrations;
    use rusqlite::Connection;
    use std::fs;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open db");
        run_migrations(&conn).expect("migrations");
        conn
    }

    fn load_example(path: &str) -> Value {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let full_path = format!("{manifest_dir}/{path}");
        let data = fs::read_to_string(full_path).expect("read example");
        serde_json::from_str(&data).expect("parse json")
    }

    #[test]
    fn test_card_schema_validation() {
        let valid_card = load_example("../examples/lanturn_delta.json");
        assert!(validate_card_schema(&valid_card).is_ok());

        let invalid_card = serde_json::json!({"name": "Missing fields"});
        assert!(validate_card_schema(&invalid_card).is_err());
    }

    #[test]
    fn test_card_compiler() {
        let conn = setup_test_db();
        let card_json = load_example("../examples/lanturn_delta.json");
        let card_id = compile_card(&conn, &card_json).expect("compile card");

        let name: String = conn
            .query_row(
                "SELECT name FROM cards WHERE card_def_id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .expect("query card");
        assert_eq!(name, "Lanturn Delta");
    }

    #[test]
    fn test_import_multiple_cards() {
        let conn = setup_test_db();
        let paths = [
            "../examples/lanturn_delta.json",
            "../examples/professor_cozmo.json",
            "../examples/metal_energy.json",
            "../examples/charizard_ex.json",
            "../examples/torchic_star.json",
            "../examples/switch.json",
            "../examples/stadium_holon_ruins.json",
            "../examples/tool_strength_charm.json",
            "../examples/basic_damage_pokemon.json",
            "../examples/coin_flip_pokemon.json",
            "../examples/trainer_draw.json",
            "../examples/trainer_damage.json",
            "../examples/trainer_if.json",
        ];
        for path in paths {
            let card_json = load_example(path);
            compile_card(&conn, &card_json).expect("compile card");
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("count cards");
        assert_eq!(count, paths.len() as i64);
    }
}
