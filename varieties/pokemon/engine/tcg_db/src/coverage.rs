use std::collections::BTreeSet;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoverageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct SetCoverage {
    pub set_code: String,
    pub expected: usize,
    pub imported: usize,
    pub missing_numbers: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub sets: Vec<SetCoverage>,
    pub total_expected: usize,
    pub total_imported: usize,
    pub coverage_percent: f64,
}

pub fn generate_coverage_report(conn: &Connection) -> Result<CoverageReport, CoverageError> {
    let expected_sets = [("CG", 100usize), ("DF", 101usize)];
    let mut coverage_sets = Vec::new();
    let mut total_expected = 0;
    let mut total_imported = 0;

    for (set_code, expected) in expected_sets {
        let numbers = load_card_numbers(conn, set_code)?;
        let imported = numbers.len();
        let missing_numbers = missing_number_list(&numbers, expected as u32);

        total_expected += expected;
        total_imported += imported;

        coverage_sets.push(SetCoverage {
            set_code: set_code.to_string(),
            expected,
            imported,
            missing_numbers,
        });
    }

    let coverage_percent = if total_expected == 0 {
        0.0
    } else {
        (total_imported as f64 / total_expected as f64) * 100.0
    };

    Ok(CoverageReport {
        sets: coverage_sets,
        total_expected,
        total_imported,
        coverage_percent,
    })
}

fn load_card_numbers(conn: &Connection, set_code: &str) -> Result<BTreeSet<u32>, CoverageError> {
    let mut stmt = conn.prepare(
        "SELECT number FROM cards
         JOIN sets ON cards.set_id = sets.set_id
         WHERE sets.code = ?1",
    )?;
    let rows = stmt.query_map([set_code], |row| row.get::<_, String>(0))?;
    let mut numbers = BTreeSet::new();
    for row in rows {
        let number_str = row?;
        if let Ok(number) = number_str.parse::<u32>() {
            numbers.insert(number);
        }
    }
    Ok(numbers)
}

fn missing_number_list(numbers: &BTreeSet<u32>, expected: u32) -> Vec<u32> {
    let mut missing = Vec::new();
    for num in 1..=expected {
        if !numbers.contains(&num) {
            missing.push(num);
        }
    }
    missing
}

pub fn format_coverage_report(report: &CoverageReport) -> String {
    let mut lines = Vec::new();
    for set in &report.sets {
        lines.push(format!(
            "{}: {}/{} imported, missing {}",
            set.set_code,
            set.imported,
            set.expected,
            set.missing_numbers.len()
        ));
        if !set.missing_numbers.is_empty() {
            let missing = set
                .missing_numbers
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("{} missing numbers: {}", set.set_code, missing));
        }
    }
    lines.push(format!(
        "Total: {}/{} ({:.2}%)",
        report.total_imported, report.total_expected, report.coverage_percent
    ));
    lines.join("\n")
}
