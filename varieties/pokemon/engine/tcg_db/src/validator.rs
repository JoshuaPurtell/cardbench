use rusqlite::{params, Connection};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct DeckEntry {
    pub card_def_id: String,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct DeckList {
    pub entries: Vec<DeckEntry>,
}

#[derive(Debug, Error)]
pub enum DeckValidationError {
    #[error("invalid deck size: expected 60, got {0}")]
    InvalidDeckSize(u32),
    #[error("too many copies of card {name}: {count}")]
    TooManyCopies { name: String, count: u32 },
    #[error("too many Pokemon Star cards: {0}")]
    TooManyPokemonStar(u32),
    #[error("invalid set for card {card_def_id}: {set_code}")]
    InvalidSet { card_def_id: String, set_code: String },
    #[error("deck has no Basic Pokemon")]
    MissingBasicPokemon,
    #[error("missing card definition for {0}")]
    MissingCard(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid tags json: {0}")]
    InvalidTags(String),
}

#[derive(Debug)]
struct CardInfo {
    name: String,
    supertype: String,
    stage: Option<String>,
    set_code: String,
    tags: Vec<String>,
    energy_kind: Option<String>,
}

pub fn validate_deck(conn: &Connection, deck: &DeckList) -> Result<(), DeckValidationError> {
    let total_cards: u32 = deck.entries.iter().map(|entry| entry.count).sum();
    if total_cards != 60 {
        return Err(DeckValidationError::InvalidDeckSize(total_cards));
    }

    let mut name_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut pokemon_star_count = 0u32;
    let mut has_basic_pokemon = false;

    for entry in &deck.entries {
        let info = load_card_info(conn, &entry.card_def_id)?;
        let is_basic_energy = info.supertype == "Energy"
            && matches!(info.energy_kind.as_deref(), Some("Basic"));

        if info.supertype == "Pokemon" && info.stage.as_deref() == Some("Basic") {
            has_basic_pokemon = true;
        }

        if info.tags.iter().any(|tag| tag == "PokemonStar") {
            pokemon_star_count += entry.count;
        }

        if !is_basic_energy {
            let count = name_counts.entry(info.name.clone()).or_insert(0);
            *count += entry.count;
        }

        // Allow CG, DF, LM (for theme deck reprints), ENERGY (basic energies), and basic energy
        let allowed_set = info.set_code == "CG" || info.set_code == "DF" || info.set_code == "LM" || info.set_code == "ENERGY" || is_basic_energy;
        if !allowed_set {
            return Err(DeckValidationError::InvalidSet {
                card_def_id: entry.card_def_id.clone(),
                set_code: info.set_code,
            });
        }
    }

    for (name, count) in name_counts {
        if count > 4 {
            return Err(DeckValidationError::TooManyCopies { name, count });
        }
    }

    if pokemon_star_count > 1 {
        return Err(DeckValidationError::TooManyPokemonStar(pokemon_star_count));
    }

    if !has_basic_pokemon {
        return Err(DeckValidationError::MissingBasicPokemon);
    }

    Ok(())
}

fn load_card_info(conn: &Connection, card_def_id: &str) -> Result<CardInfo, DeckValidationError> {
    let mut stmt = conn.prepare(
        "SELECT cards.name, cards.supertype, cards.stage, sets.code, cards.tags_json, cards.energy_kind
         FROM cards
         JOIN sets ON cards.set_id = sets.set_id
         WHERE cards.card_def_id = ?1",
    )?;
    let result = stmt.query_row(params![card_def_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    });

    match result {
        Ok((name, supertype, stage, set_code, tags_json, energy_kind)) => {
            let tags = parse_tags(&tags_json)?;
            Ok(CardInfo {
                name,
                supertype,
                stage,
                set_code,
                tags,
                energy_kind,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(DeckValidationError::MissingCard(card_def_id.to_string()))
        }
        Err(err) => Err(DeckValidationError::Database(err)),
    }
}

fn parse_tags(tags_json: &str) -> Result<Vec<String>, DeckValidationError> {
    let value: Value =
        serde_json::from_str(tags_json).map_err(|err| DeckValidationError::InvalidTags(err.to_string()))?;
    match value {
        Value::Array(items) => Ok(items
            .into_iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect()),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_card;
    use crate::run_migrations;
    use rusqlite::Connection;
    use serde_json::json;
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

    fn compile_examples(conn: &Connection) -> Vec<String> {
        let paths = [
            "../examples/basic_pokemon.json",
            "../examples/metal_energy.json",
            "../examples/torchic_star.json",
        ];
        paths
            .iter()
            .map(|path| compile_card(conn, &load_example(path)).expect("compile"))
            .collect()
    }

    #[test]
    fn test_deck_validator_60_cards() {
        let conn = setup_test_db();
        let ids = compile_examples(&conn);
        let basic_pokemon = ids[0].clone();
        let basic_energy = ids[1].clone();

        let deck = DeckList {
            entries: vec![
                DeckEntry {
                    card_def_id: basic_pokemon.clone(),
                    count: 1,
                },
                DeckEntry {
                    card_def_id: basic_energy.clone(),
                    count: 58,
                },
            ],
        };
        assert!(validate_deck(&conn, &deck).is_err());

        let deck = DeckList {
            entries: vec![
                DeckEntry {
                    card_def_id: basic_pokemon,
                    count: 1,
                },
                DeckEntry {
                    card_def_id: basic_energy,
                    count: 59,
                },
            ],
        };
        assert!(validate_deck(&conn, &deck).is_ok());
    }

    #[test]
    fn test_deck_validator_4_copy_rule() {
        let conn = setup_test_db();
        let ids = compile_examples(&conn);
        let basic_pokemon = ids[0].clone();
        let basic_energy = ids[1].clone();

        let deck = DeckList {
            entries: vec![
                DeckEntry {
                    card_def_id: basic_pokemon.clone(),
                    count: 5,
                },
                DeckEntry {
                    card_def_id: basic_energy,
                    count: 55,
                },
            ],
        };
        assert!(validate_deck(&conn, &deck).is_err());
    }

    #[test]
    fn test_deck_validator_star_restriction() {
        let conn = setup_test_db();
        let ids = compile_examples(&conn);
        let basic_pokemon = ids[0].clone();
        let basic_energy = ids[1].clone();
        let star = ids[2].clone();

        let deck = DeckList {
            entries: vec![
                DeckEntry {
                    card_def_id: basic_pokemon,
                    count: 1,
                },
                DeckEntry {
                    card_def_id: star,
                    count: 2,
                },
                DeckEntry {
                    card_def_id: basic_energy,
                    count: 57,
                },
            ],
        };
        assert!(validate_deck(&conn, &deck).is_err());
    }

    #[test]
    fn test_deck_validator_set_whitelist() {
        let conn = setup_test_db();
        let ids = compile_examples(&conn);
        let basic_pokemon = ids[0].clone();
        let basic_energy = ids[1].clone();

        let invalid_card = json!({
            "set": "XY",
            "number": "001/146",
            "name": "Invalid Mon",
            "supertype": "Pokemon",
            "stage": "Basic",
            "hp": 60,
            "types": ["Grass"],
            "weakness": [],
            "resistance": [],
            "retreat": 1,
            "attacks": [],
            "powers": []
        });
        let invalid_id = compile_card(&conn, &invalid_card).expect("compile invalid set");

        let deck = DeckList {
            entries: vec![
                DeckEntry {
                    card_def_id: basic_pokemon,
                    count: 1,
                },
                DeckEntry {
                    card_def_id: invalid_id,
                    count: 1,
                },
                DeckEntry {
                    card_def_id: basic_energy,
                    count: 58,
                },
            ],
        };
        assert!(validate_deck(&conn, &deck).is_err());
    }

    #[test]
    fn test_deck_validator_basic_pokemon() {
        let conn = setup_test_db();
        let ids = compile_examples(&conn);
        let basic_energy = ids[1].clone();

        let deck = DeckList {
            entries: vec![DeckEntry {
                card_def_id: basic_energy,
                count: 60,
            }],
        };
        assert!(validate_deck(&conn, &deck).is_err());
    }
}
