use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tcg_core::GameStateSnapshot;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameSnapshot {
    pub fingerprint: String,
    pub turn_number: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameLogRecord {
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing game state for {0}")]
    MissingState(String),
}

pub fn save_game_state(
    conn: &Connection,
    game_id: &str,
    snapshot: &GameSnapshot,
) -> Result<(), ReplayError> {
    let json = serde_json::to_string(snapshot)?;
    conn.execute(
        "INSERT OR REPLACE INTO game_state (game_id, state_json) VALUES (?1, ?2)",
        params![game_id, json],
    )?;
    Ok(())
}

pub fn load_game_state(conn: &Connection, game_id: &str) -> Result<GameSnapshot, ReplayError> {
    let json: String = conn
        .query_row(
            "SELECT state_json FROM game_state WHERE game_id = ?1",
            params![game_id],
            |row| row.get(0),
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => ReplayError::MissingState(game_id.to_string()),
            _ => ReplayError::Database(err),
        })?;
    Ok(serde_json::from_str(&json)?)
}

pub fn save_game_log(
    conn: &mut Connection,
    game_id: &str,
    records: &[GameLogRecord],
) -> Result<(), ReplayError> {
    let tx = conn.transaction()?;
    for (idx, record) in records.iter().enumerate() {
        let json = serde_json::to_string(record)?;
        tx.execute(
            "INSERT OR REPLACE INTO game_log (game_id, seq, record_json) VALUES (?1, ?2, ?3)",
            params![game_id, idx as i64, json],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn load_game_log(conn: &Connection, game_id: &str) -> Result<Vec<GameLogRecord>, ReplayError> {
    let mut stmt = conn.prepare(
        "SELECT record_json FROM game_log WHERE game_id = ?1 ORDER BY seq",
    )?;
    let rows = stmt.query_map(params![game_id], |row| row.get::<_, String>(0))?;
    let mut records = Vec::new();
    for row in rows {
        let json = row?;
        let record: GameLogRecord = serde_json::from_str(&json)?;
        records.push(record);
    }
    Ok(records)
}

pub fn save_game_state_snapshot(
    conn: &Connection,
    game_id: &str,
    snapshot: &GameStateSnapshot,
) -> Result<(), ReplayError> {
    let json = serde_json::to_string(snapshot)?;
    conn.execute(
        "INSERT OR REPLACE INTO game_state (game_id, state_json) VALUES (?1, ?2)",
        params![game_id, json],
    )?;
    Ok(())
}

pub fn load_game_state_snapshot(
    conn: &Connection,
    game_id: &str,
) -> Result<GameStateSnapshot, ReplayError> {
    let json: String = conn
        .query_row(
            "SELECT state_json FROM game_state WHERE game_id = ?1",
            params![game_id],
            |row| row.get(0),
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => ReplayError::MissingState(game_id.to_string()),
            _ => ReplayError::Database(err),
        })?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_migrations;
    use rusqlite::Connection;
    use serde_json::json;
    use tcg_core::{CardDefId, CardInstance, GameState, PlayerId};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open db");
        run_migrations(&conn).expect("migrations");
        conn
    }

    #[test]
    fn test_game_save_load() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-1', 'Deck 1', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck1");
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-2', 'Deck 2', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck2");
        conn.execute(
            "INSERT INTO games (game_id, created_at, ruleset_ver, rng_seed, deck0_id, deck1_id, winner, result_json)
             VALUES ('game-1', '2026-01-01', 'ex-1.0', '123', 'deck-1', 'deck-2', NULL, '{}')",
            [],
        )
        .expect("insert game");

        let snapshot = GameSnapshot {
            fingerprint: "state-abc".to_string(),
            turn_number: 5,
        };
        save_game_state(&conn, "game-1", &snapshot).expect("save state");
        let loaded = load_game_state(&conn, "game-1").expect("load state");
        assert_eq!(loaded.fingerprint, snapshot.fingerprint);
        assert_eq!(loaded.turn_number, snapshot.turn_number);
    }

    #[test]
    fn test_replay_determinism() {
        let mut conn = setup_db();
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-1', 'Deck 1', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck1");
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-2', 'Deck 2', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck2");
        conn.execute(
            "INSERT INTO games (game_id, created_at, ruleset_ver, rng_seed, deck0_id, deck1_id, winner, result_json)
             VALUES ('game-2', '2026-01-01', 'ex-1.0', '123', 'deck-1', 'deck-2', NULL, '{}')",
            [],
        )
        .expect("insert game");

        let records = vec![
            GameLogRecord {
                kind: "Action".to_string(),
                payload: json!({"action": "Draw"}),
            },
            GameLogRecord {
                kind: "Action".to_string(),
                payload: json!({"action": "Attack"}),
            },
        ];
        save_game_log(&mut conn, "game-2", &records).expect("save log");
        let loaded = load_game_log(&conn, "game-2").expect("load log");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].kind, "Action");
        assert_eq!(loaded[1].payload["action"], "Attack");
    }

    #[test]
    fn test_game_state_snapshot_roundtrip() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-1', 'Deck 1', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck1");
        conn.execute(
            "INSERT INTO decks (deck_id, name, format, created_at)
             VALUES ('deck-2', 'Deck 2', 'EX', '2026-01-01')",
            [],
        )
        .expect("insert deck2");
        conn.execute(
            "INSERT INTO games (game_id, created_at, ruleset_ver, rng_seed, deck0_id, deck1_id, winner, result_json)
             VALUES ('game-3', '2026-01-01', 'ex-1.0', '123', 'deck-1', 'deck-2', NULL, '{}')",
            [],
        )
        .expect("insert game");

        // Build a real snapshot via tcg_core to avoid coupling this test to
        // GameStateSnapshot field visibility/shape.
        fn build_simple_deck(prefix: &str, player: PlayerId) -> Vec<CardInstance> {
            (0..60)
                .map(|index| {
                    CardInstance::new(
                        CardDefId::new(format!("{prefix}-{index:03}")),
                        player,
                    )
                })
                .collect()
        }

        let game = GameState::new(
            build_simple_deck("P1", PlayerId::P1),
            build_simple_deck("P2", PlayerId::P2),
            123,
            tcg_rules_ex::RulesetConfig::default(),
        );
        let snapshot: GameStateSnapshot = game.to_snapshot();

        save_game_state_snapshot(&conn, "game-3", &snapshot).expect("save snapshot");
        let loaded = load_game_state_snapshot(&conn, "game-3").expect("load snapshot");
        assert_eq!(loaded.version, snapshot.version);
        assert_eq!(loaded.turn.number, snapshot.turn.number);
    }
}
