//! Plays one match and emits a reviewable transcript, timeline, and critique.
//!
//! Usage:
//!   rav-match-review <deck-a> <deck-b> [SEED] [--jsonl PATH]
//!
//! Unlike the Pokémon variety's `eventlog.jsonl`, whose contents are
//! synthesised from the candidate's score, everything written here comes from
//! the match that was actually played.

use cardbench_magic_policies::{
    Archetype, DeckMatchConfig, PolicyVersion, run_deck_matchup_capturing, seat_policy,
    shared_card_index,
};
use cardbench_magic_rav::load_constructed_decks;
use cardbench_magic_session::critique::render as render_critique;
use cardbench_magic_session::{
    MatchManifest, MatchTranscript, TRANSCRIPT_SCHEMA, critique, project_events, render_timeline,
};

#[allow(clippy::too_many_lines)] // One straight-line CLI reads better than five helpers.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if positional.len() < 2 {
        eprintln!("usage: rav-match-review <deck-a> <deck-b> [SEED] [--jsonl PATH]");
        std::process::exit(2);
    }
    let deck_a = positional[0].clone();
    let deck_b = positional[1].clone();
    let seed: u64 = positional
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let jsonl_path = args
        .iter()
        .position(|arg| arg == "--jsonl")
        .and_then(|index| args.get(index + 1))
        .cloned();

    let decks = match load_constructed_decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("failed to load constructed decks: {error}");
            std::process::exit(1);
        }
    };
    let archetype_of = |id: &str| {
        decks
            .iter()
            .find(|deck| deck.id == id)
            .and_then(|deck| Archetype::parse(&deck.archetype))
            .unwrap_or(Archetype::Midrange)
    };

    let index = shared_card_index();
    let version = PolicyVersion::latest();
    let pilots = [
        seat_policy(
            version,
            cardbench_magic_engine::PlayerId(0),
            archetype_of(&deck_a),
            index.clone(),
        ),
        seat_policy(
            version,
            cardbench_magic_engine::PlayerId(1),
            archetype_of(&deck_b),
            index,
        ),
    ];
    let policies = [pilots[0].id().to_owned(), pilots[1].id().to_owned()];

    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let (result, events) = match run_deck_matchup_capturing(config, &deck_a, &deck_b, pilots) {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("match failed: {error}");
            std::process::exit(1);
        }
    };

    let manifest = MatchManifest {
        schema_version: TRANSCRIPT_SCHEMA.to_owned(),
        decks: result.deck_ids.clone(),
        policies,
        shuffle_seed: seed,
        opening_hand_size: result.config.opening_hand_size,
        turns: result.turns,
        winner: result
            .winner
            .map(|player| u16::try_from(player.0).unwrap_or(u16::MAX)),
        termination: format!("{:?}", result.termination),
        life: result.life,
        accepted_policy_moves: result.accepted_policy_moves,
        rejected_policy_moves: result
            .attempted_policy_moves
            .saturating_sub(result.accepted_policy_moves),
        digest: result.digest.clone(),
    };
    let transcript = MatchTranscript {
        manifest,
        events: project_events(&events),
    };

    print!("{}", render_timeline(&transcript));
    println!("\n# critique");
    let findings = critique(&transcript);
    print!("{}", render_critique(&findings));

    if let Some(path) = jsonl_path {
        match transcript.to_jsonl() {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    eprintln!("failed to write {path}: {error}");
                    std::process::exit(1);
                }
                println!(
                    "\ntranscript_written={path} events={}",
                    transcript.events.len()
                );
            }
            Err(error) => {
                eprintln!("failed to serialise transcript: {error}");
                std::process::exit(1);
            }
        }
    }
}
