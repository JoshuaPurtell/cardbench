use std::env;

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <db_path>", args[0]);
        std::process::exit(1);
    }

    let db_path = &args[1];
    let conn = Connection::open(db_path)?;
    let report = tcg_db::generate_coverage_report(&conn)?;
    let output = tcg_db::format_coverage_report(&report);
    println!("{output}");
    Ok(())
}
