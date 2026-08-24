//! Scores a (deck, pilot) candidate against a `cardbench/magic/code_policy` split.
//!
//! ```text
//! rav-code-policy-sweep --candidate-deck ID [--candidate-pilot vN] [--split train|heldout]
//! rav-code-policy-sweep --print-heldout-digest
//! ```
//!
//! Defaults: `--split train`, `--candidate-pilot` the newest generation,
//! `--roster` the committed `rosters/code_policy_v1.json`.
//!
//! There is deliberately **no `--seeds` flag**. The seed count is part of the
//! surface the roster pins; letting a caller widen it would let a candidate buy
//! significance with compute and would make two runs incomparable.
//!
//! `--split train` is feedback. `--split heldout` is authority, and it refuses
//! to run unless the sealed manifest is present and hashes to the digest the
//! committed roster pins.
//!
//! Exit codes: `0` the sweep scored (pass or fail), `1` the sweep could not
//! score, `2` bad usage. A sweep that could not grade is never a zero score —
//! infrastructure failure and candidate failure are different facts.

use cardbench_magic_policies::code_policy::roster::{Entrant, Split, load_surface};
use cardbench_magic_policies::code_policy::sweep::{SweepReport, run_sweep};
use cardbench_magic_policies::code_policy::{reference, sha256};
use cardbench_magic_policies::{DeckMatchConfig, PolicyVersion};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

fn points(value: f64) -> String {
    format!("{:+.1}", value * 100.0)
}

fn default_roster() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("rosters"), std::path::Path::to_path_buf)
        .join("rosters")
        .join("code_policy_v1.json")
}

struct Options {
    roster: PathBuf,
    split: Split,
    candidate_deck: Option<String>,
    candidate_pilot: PolicyVersion,
    json: Option<PathBuf>,
    print_digest: bool,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        roster: default_roster(),
        split: Split::Train,
        candidate_deck: None,
        candidate_pilot: PolicyVersion::latest(),
        json: None,
        print_digest: false,
    };
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let mut value = || {
            index += 1;
            arguments
                .get(index)
                .cloned()
                .ok_or_else(|| format!("{flag} requires a value"))
        };
        match flag {
            "--roster" => options.roster = PathBuf::from(value()?),
            "--split" => {
                let raw = value()?;
                options.split =
                    Split::parse(&raw).ok_or_else(|| format!("unknown split `{raw}`"))?;
            }
            "--candidate-deck" => options.candidate_deck = Some(value()?),
            "--candidate-pilot" => {
                let raw = value()?;
                options.candidate_pilot = PolicyVersion::parse(&raw)
                    .ok_or_else(|| format!("unknown pilot generation `{raw}`"))?;
            }
            "--json" => options.json = Some(PathBuf::from(value()?)),
            "--print-heldout-digest" => options.print_digest = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
        index += 1;
    }
    Ok(options)
}

fn usage() -> &'static str {
    "rav-code-policy-sweep --candidate-deck ID [--candidate-pilot vN]\n\
     \x20                     [--split train|heldout] [--roster PATH] [--json PATH]\n\
     rav-code-policy-sweep --print-heldout-digest"
}

/// Prints the digest of the sealed manifest on disk. Used by
/// `scripts/build_sealed_heldout.py --verify` to prove the Rust and Python
/// hashers agree on the canonical form rather than assuming it.
fn print_heldout_digest(roster: &std::path::Path) -> Result<(), String> {
    let root = roster
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| "cannot locate the variety root from the roster path".to_owned())?;
    let manifest = root.join(".sealed/code_policy/heldout_v1.json");
    let raw = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|error| format!("cannot parse manifest: {error}"))?;
    let canonical = serde_json::to_string(&value)
        .map_err(|error| format!("cannot canonicalise manifest: {error}"))?;
    println!("{}", sha256::hex_digest(canonical.as_bytes()));
    Ok(())
}

fn report_json(report: &SweepReport) -> serde_json::Value {
    let opponents: Vec<serde_json::Value> = report
        .per_opponent
        .iter()
        .map(|entry| {
            serde_json::json!({
                "opponent_id": entry.opponent_id,
                "opponent_pilot": entry.opponent_pilot,
                "opponent_deck": entry.opponent_deck,
                "candidate_win_rate": entry.candidate.point,
                "candidate_ci": [entry.candidate.low, entry.candidate.high],
                "candidate_decided": entry.candidate.samples,
                "reference_win_rate": entry.reference.point,
                "reference_ci": [entry.reference.low, entry.reference.high],
                "reference_decided": entry.reference.samples,
                "delta": entry.delta.point,
                "delta_ci": [entry.delta.low, entry.delta.high],
                "candidate_on_play": entry.candidate_on_play.point,
                "candidate_on_draw": entry.candidate_on_draw.point,
                "cells": entry.cells,
                "draws": entry.candidate_draws,
                "stalls": entry.candidate_stalls,
                "rejected_moves": entry.candidate_rejected_moves,
                "engine_findings": entry.engine_findings.len(),
                "stall_reasons": entry.stall_reasons,
                "regression": entry.is_regression(),
            })
        })
        .collect();
    serde_json::json!({
        "schema_version": "cardbench.magic.code_policy_sweep.v1",
        "task_id": report.task_id,
        "roster_id": report.roster_id,
        "split": report.split,
        "score_metric": report.score_metric,
        "heldout_manifest_sha256": report.manifest_sha256,
        "reference": {
            "id": report.reference.label,
            "pilot": report.reference.pilot.id(),
            "deck": report.reference.deck,
        },
        "candidate": {
            "id": report.candidate.label,
            "pilot": report.candidate.pilot.id(),
            "deck": report.candidate.deck,
        },
        "coverage": {
            "expected": report.coverage.expected,
            "candidate_produced": report.coverage.candidate_produced,
            "reference_produced": report.coverage.reference_produced,
            "missing": report.coverage.missing.len(),
            "unexpected": report.coverage.unexpected.len(),
            "unpaired": report.coverage.unpaired.len(),
            "stall_failures": report.coverage.stall_failures,
            "complete": report.coverage.is_complete(),
        },
        "scored": report.scored,
        "reward": report.reward,
        "reward_ci": [report.delta_low, report.delta_high],
        "candidate_overall_win_rate": report.candidate_overall.point,
        "reference_overall_win_rate": report.reference_overall.point,
        "regressions": report.regressions().iter().map(|entry| entry.opponent_id.clone()).collect::<Vec<_>>(),
        "contaminated": report.contaminated().iter().map(|entry| entry.opponent_id.clone()).collect::<Vec<_>>(),
        "passes": report.passes(),
        "per_opponent": opponents,
    })
}

fn print_report(report: &SweepReport) {
    println!("schema_version=cardbench.magic.code_policy_sweep.v1");
    println!(
        "task={} roster={} split={} metric={}",
        report.task_id, report.roster_id, report.split, report.score_metric
    );
    if let Some(digest) = &report.manifest_sha256 {
        println!("heldout_manifest_sha256={digest}");
    }
    println!(
        "reference id={} pilot={} deck={}",
        report.reference.label, report.reference.pilot, report.reference.deck
    );
    println!(
        "candidate pilot={} deck={}",
        report.candidate.pilot, report.candidate.deck
    );
    println!(
        "coverage expected={} candidate={} reference={} missing={} unexpected={} unpaired={} complete={}",
        report.coverage.expected,
        report.coverage.candidate_produced,
        report.coverage.reference_produced,
        report.coverage.missing.len(),
        report.coverage.unexpected.len(),
        report.coverage.unpaired.len(),
        report.coverage.is_complete(),
    );
    for failure in &report.coverage.stall_failures {
        println!("coverage_stall {failure}");
    }
    println!();
    println!("# per opponent (candidate rate, reference rate, delta in win-rate points)");
    for entry in &report.per_opponent {
        println!(
            "opponent id={} pilot={} deck={} candidate={}% ci=[{},{}] reference={}% ci=[{},{}] delta={}pt ci=[{},{}] n={} draws={} stalls={} rejected={}",
            entry.opponent_id,
            entry.opponent_pilot,
            entry.opponent_deck,
            percent(entry.candidate.point),
            percent(entry.candidate.low),
            percent(entry.candidate.high),
            percent(entry.reference.point),
            percent(entry.reference.low),
            percent(entry.reference.high),
            points(entry.delta.point),
            points(entry.delta.low),
            points(entry.delta.high),
            entry.candidate.samples,
            entry.candidate_draws,
            entry.candidate_stalls,
            entry.candidate_rejected_moves,
        );
        println!(
            "  seats id={} on_play={}% on_draw={}%",
            entry.opponent_id,
            percent(entry.candidate_on_play.point),
            percent(entry.candidate_on_draw.point),
        );
    }
    println!();
    println!(
        "overall candidate={}% reference={}% n={}",
        percent(report.candidate_overall.point),
        percent(report.reference_overall.point),
        report.candidate_overall.samples,
    );
    println!(
        "reward={}pt ci=[{},{}] scored={} passes={}",
        points(report.reward),
        points(report.delta_low),
        points(report.delta_high),
        report.scored,
        report.passes(),
    );
    for entry in report.regressions() {
        println!(
            "regression opponent={} delta={}pt (blocks the gate)",
            entry.opponent_id,
            points(entry.delta.point)
        );
    }
    for entry in report.contaminated() {
        println!(
            "contaminated opponent={} rejected_moves={} engine_findings={}",
            entry.opponent_id,
            entry.candidate_rejected_moves,
            entry.engine_findings.len()
        );
        for (reason, count) in &entry.stall_reasons {
            println!("  stall opponent={} count={count} reason={reason}", entry.opponent_id);
        }
    }
    if !report.scored {
        println!(
            "NOT SCORED: coverage failed, so reward is the fail-closed zero and not a measurement"
        );
    }
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n\n{}", usage());
            std::process::exit(2);
        }
    };

    if options.print_digest {
        if let Err(error) = print_heldout_digest(&options.roster) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    let Some(candidate_deck) = options.candidate_deck else {
        eprintln!("--candidate-deck is required\n\n{}", usage());
        std::process::exit(2);
    };

    let surface = match load_surface(&options.roster, options.split) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("cannot resolve the {} surface: {error}", options.split.id());
            std::process::exit(1);
        }
    };

    if !surface.candidate_deck_pool.contains(&candidate_deck) {
        eprintln!(
            "deck `{candidate_deck}` is not in the roster's candidate pool: {}",
            surface.candidate_deck_pool.join(" ")
        );
        std::process::exit(2);
    }

    // Resolve the candidate's archetype from its own deck fixture, the same way
    // an opponent's is resolved, so the pilot is weighted for the deck it flies.
    let archetypes: BTreeMap<&str, _> = surface
        .opponents
        .iter()
        .map(|opponent| (opponent.deck.as_str(), opponent.archetype))
        .chain(std::iter::once((
            surface.reference.deck.as_str(),
            surface.reference.archetype,
        )))
        .collect();
    let candidate_archetype = match archetypes.get(candidate_deck.as_str()) {
        Some(archetype) => *archetype,
        None => match resolve_pool_archetype(&options.roster, &candidate_deck) {
            Ok(archetype) => archetype,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },
    };

    let candidate = Entrant {
        label: format!("candidate/{}/{}", candidate_deck, options.candidate_pilot),
        deck: candidate_deck,
        pilot: options.candidate_pilot,
        archetype: candidate_archetype,
    };

    let report = match run_sweep(&surface, &candidate, &DeckMatchConfig::default()) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("sweep could not grade: {error}");
            std::process::exit(1);
        }
    };

    print_report(&report);
    if let Some(path) = options.json {
        let rendered = serde_json::to_string_pretty(&report_json(&report))
            .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"));
        if let Err(error) = std::fs::write(&path, rendered + "\n") {
            eprintln!("cannot write {}: {error}", path.display());
            std::process::exit(1);
        }
    }
    println!("\nreference_id={}", reference::REFERENCE_ID);
}

/// Resolves a pool deck that no opponent and no reference happens to play.
fn resolve_pool_archetype(
    roster: &std::path::Path,
    deck: &str,
) -> Result<cardbench_magic_policies::Archetype, String> {
    // `load_surface` already validated every pool deck against the fixtures, so
    // the fixture lookup here cannot fail for a deck that reached this point.
    let fixtures = cardbench_magic_rav::load_constructed_decks()
        .map_err(|error| error.to_string())?
        .into_iter()
        .chain(
            cardbench_magic_rav::load_hillclimb_decks().map_err(|error| error.to_string())?,
        );
    for fixture in fixtures {
        if fixture.id == deck {
            return cardbench_magic_policies::Archetype::parse(&fixture.archetype).ok_or_else(
                || format!("deck `{deck}` names unknown archetype `{}`", fixture.archetype),
            );
        }
    }
    Err(format!(
        "deck `{deck}` is in the pool of {} but has no fixture",
        roster.display()
    ))
}
