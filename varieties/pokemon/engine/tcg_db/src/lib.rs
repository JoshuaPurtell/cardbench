mod compiler;
mod importer;
mod deck_importer;
mod coverage;
mod validator;
mod replay;
mod card_meta;
mod energies;

use rusqlite::Connection;

pub use compiler::{compile_card, validate_card_schema, CardCompilerError};
pub use importer::{import_jsonl, ImportError, ImportReport};
pub use deck_importer::{import_deck_json, DeckImportError};
pub use coverage::{format_coverage_report, generate_coverage_report, CoverageError, CoverageReport};
pub use validator::{validate_deck, DeckEntry, DeckList, DeckValidationError};
pub use replay::{
    load_game_log, load_game_state, load_game_state_snapshot, save_game_log, save_game_state,
    save_game_state_snapshot, GameLogRecord, GameSnapshot, ReplayError,
};
pub use card_meta::{load_card_meta_map, CardMetaError};
pub use energies::add_basic_energies;

const MIGRATION_001: &str = include_str!("../migrations/001_initial.sql");

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(MIGRATION_001)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Result;

    fn setup_test_db() -> Result<Connection> {
        let conn = Connection::open_in_memory()?;
        run_migrations(&conn)?;
        Ok(conn)
    }

    fn get_tables(conn: &Connection) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut names = Vec::new();
        for name in rows {
            names.push(name?);
        }
        Ok(names)
    }

    #[test]
    fn test_database_migrations() -> Result<()> {
        let conn = setup_test_db()?;
        let tables = get_tables(&conn)?;
        for table in [
            "sets",
            "cards",
            "attacks",
            "powers",
            "decks",
            "deck_cards",
            "games",
            "game_log",
        ] {
            assert!(tables.contains(&table.to_string()));
        }
        Ok(())
    }
}
