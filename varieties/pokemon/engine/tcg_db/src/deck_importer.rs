use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde_json::Value;
use thiserror::Error;

use crate::{validate_deck, DeckEntry, DeckList, DeckValidationError};

#[derive(Debug, Error)]
pub enum DeckImportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid deck json: {0}")]
    InvalidDeck(String),
    #[error("deck validation error: {0}")]
    Validation(#[from] DeckValidationError),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn import_deck_json(conn: &mut Connection, path: &str) -> Result<String, DeckImportError> {
    let content = fs::read_to_string(path)?;
    let deck_json: Value = serde_json::from_str(&content)?;
    let deck = decklist_from_json(&deck_json)?;
    validate_deck(conn, &deck)?;

    let name = deck_json
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Imported Deck");
    let format = deck_json
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let deck_id = deck_json
        .get("deck_id")
        .and_then(Value::as_str)
        .map(|id| id.to_string())
        .unwrap_or_else(|| slugify(name));
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR REPLACE INTO decks (deck_id, name, format, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![deck_id, name, format, created_at],
    )?;
    tx.execute("DELETE FROM deck_cards WHERE deck_id = ?1", params![deck_id])?;
    for entry in deck.entries {
        tx.execute(
            "INSERT INTO deck_cards (deck_id, card_def_id, count) VALUES (?1, ?2, ?3)",
            params![deck_id, entry.card_def_id, entry.count],
        )?;
    }
    tx.commit()?;

    Ok(deck_id)
}

#[allow(dead_code)]
pub fn load_decklist(conn: &Connection, deck_id: &str) -> Result<DeckList, DeckImportError> {
    let mut stmt = conn.prepare(
        "SELECT card_def_id, count FROM deck_cards WHERE deck_id = ?1 ORDER BY card_def_id",
    )?;
    let rows = stmt.query_map(params![deck_id], |row| {
        Ok(DeckEntry {
            card_def_id: row.get(0)?,
            count: row.get::<_, i64>(1)? as u32,
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    if entries.is_empty() {
        return Err(DeckImportError::InvalidDeck(format!(
            "no cards found for deck_id {deck_id}"
        )));
    }
    Ok(DeckList { entries })
}

fn decklist_from_json(deck_json: &Value) -> Result<DeckList, DeckImportError> {
    let mut entries = Vec::new();
    if let Some(cards) = deck_json.get("cards").and_then(Value::as_array) {
        for card_entry in cards {
            let def_id = card_entry
                .get("def_id")
                .and_then(Value::as_str)
                .ok_or_else(|| DeckImportError::InvalidDeck("missing def_id".to_string()))?;
            let count = card_entry
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or(1) as u32;
            entries.push(DeckEntry {
                card_def_id: def_id.to_string(),
                count,
            });
        }
    }

    if let Some(energy) = deck_json.get("energy").and_then(Value::as_array) {
        for energy_entry in energy {
            let energy_type = energy_entry
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| DeckImportError::InvalidDeck("missing energy type".to_string()))?;
            let count = energy_entry
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let def_id = format!("ENERGY-{}", energy_type.to_uppercase());
            entries.push(DeckEntry { card_def_id: def_id, count });
        }
    }

    Ok(DeckList { entries })
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "deck-imported".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_card, run_migrations};
    use rusqlite::Connection;
    use serde_json::{json, Value};

    #[test]
    fn test_import_deck_json() {
        let mut conn = Connection::open_in_memory().expect("db");
        run_migrations(&conn).expect("migrations");
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let basic_path = format!("{manifest_dir}/../examples/basic_pokemon.json");
        let basic_data = fs::read_to_string(&basic_path).expect("basic example");
        let basic: Value = serde_json::from_str(&basic_data).expect("parse basic");
        let energy = json!({
            "set": "ENERGY",
            "number": "WATER",
            "name": "Water Energy",
            "supertype": "Energy",
            "energy_kind": "Basic",
            "effect_ast": {"op": "NoOp"},
            "tags": []
        });
        let basic_id = compile_card(&conn, &basic).expect("basic");
        let _ = compile_card(&conn, &energy).expect("energy");
        let deck_json = json!({
            "name": "Test Deck",
            "format": "EX",
            "cards": [
                {"def_id": basic_id, "count": 1}
            ],
            "energy": [
                {"type": "Water", "count": 59}
            ]
        });
        let mut temp_path = std::env::temp_dir();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        temp_path.push(format!("tcg-deck-{suffix}.json"));
        fs::write(&temp_path, deck_json.to_string()).expect("write");
        let deck_id = import_deck_json(&mut conn, temp_path.to_str().unwrap()).expect("import");
        let deck = load_decklist(&conn, &deck_id).expect("load");
        let total: u32 = deck.entries.iter().map(|entry| entry.count).sum();
        assert_eq!(total, 60);
        let name: String = conn
            .query_row(
                "SELECT name FROM decks WHERE deck_id = ?1",
                params![deck_id],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(name, "Test Deck");
    }
}
