use std::env;

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 6 && args.len() != 7 {
        eprintln!(
            "Usage: {} <db_path> <cg_jsonl> <df_jsonl> <rs_jsonl> [ss_jsonl] <out_report>",
            args[0]
        );
        std::process::exit(1);
    }

    let db_path = &args[1];
    let cg_path = &args[2];
    let df_path = &args[3];
    let rs_path = &args[4];
    let (ss_path, report_path) = if args.len() == 7 {
        (Some(&args[5]), &args[6])
    } else {
        (None, &args[5])
    };

    let conn = Connection::open(db_path)?;
    tcg_db::run_migrations(&conn)?;

    let cg_report = tcg_db::import_jsonl(&conn, cg_path, "CG", 100)?;
    let df_report = tcg_db::import_jsonl(&conn, df_path, "DF", 101)?;
    let rs_report = tcg_db::import_jsonl(&conn, rs_path, "RS", 109)?;
    let ss_report = match ss_path {
        Some(path) => Some(tcg_db::import_jsonl(&conn, path, "SS", 100)?),
        None => None,
    };
    let energy_added = tcg_db::add_basic_energies(&conn)?;

    let mut report = format!(
        "CG imported: {}, skipped: {}\nDF imported: {}, skipped: {}\nRS imported: {}, skipped: {}\n",
        cg_report.imported,
        cg_report.skipped,
        df_report.imported,
        df_report.skipped,
        rs_report.imported,
        rs_report.skipped
    );
    if let Some(ss_report) = ss_report {
        report.push_str(&format!(
            "SS imported: {}, skipped: {}\n",
            ss_report.imported, ss_report.skipped
        ));
    }
    report.push_str(&format!("Basic energy added: {}\n", energy_added));
    std::fs::write(report_path, report)?;
    Ok(())
}
