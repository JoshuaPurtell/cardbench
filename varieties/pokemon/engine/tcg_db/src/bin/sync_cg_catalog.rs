use std::env;
use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use serde_json::Value;
use tcg_db::{compile_card, run_migrations};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing normalized catalog path")?);
    let database = PathBuf::from(args.next().ok_or("missing output database path")?);
    if args.next().is_some() {
        return Err("usage: sync_cg_catalog NORMALIZED_CATALOG OUTPUT_DB".into());
    }

    let cards: Vec<Value> = serde_json::from_slice(&fs::read(input)?)?;
    if cards
        .iter()
        .any(|card| card.get("set").and_then(Value::as_str) != Some("CG"))
    {
        return Err("normalized catalog may contain only CG cards".into());
    }

    let mut conn = Connection::open(database)?;
    run_migrations(&conn)?;
    let transaction = conn.transaction()?;
    for card in &cards {
        compile_card(&transaction, card)?;
    }
    transaction.commit()?;
    println!("synchronized {} Crystal Guardians cards", cards.len());
    Ok(())
}
