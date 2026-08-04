//! Prints standings from a replayable Elo ledger without running matches.
//!
//! Usage: `rav-ratings LEDGER.tsv [AXIS]`.

use cardbench_magic_policies::EloLedger;
use std::path::Path;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!("usage: rav-ratings LEDGER.tsv [AXIS]");
        std::process::exit(2);
    };
    let requested_axis = arguments.next();
    if arguments.next().is_some() {
        eprintln!("usage: rav-ratings LEDGER.tsv [AXIS]");
        std::process::exit(2);
    }
    let ledger = match EloLedger::load(Path::new(&path)) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("failed to load Elo ledger: {error}");
            std::process::exit(1);
        }
    };
    let axes = requested_axis.map_or_else(|| ledger.axes(), |axis| vec![axis]);
    println!(
        "schema_version=cardbench.magic.elo.v1 ledger={} stored_pairs={}",
        path,
        ledger.record_count()
    );
    for axis in axes {
        println!("elo_standings axis={axis}");
        for standing in ledger.standings(&axis) {
            println!(
                "elo id={} rating={:.1} paired_matches={} anchored={}",
                standing.id, standing.rating, standing.matches, standing.anchored
            );
        }
    }
}
