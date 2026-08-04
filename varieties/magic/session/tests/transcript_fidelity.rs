//! A transcript must be a faithful, round-trippable record of a real match.
//!
//! The failure this guards against is the one the Pokémon variety's eval
//! scripts actually exhibit: an artefact named like a match log whose contents
//! were synthesised from the result. A transcript that does not reproduce the
//! engine's canonical log is worse than no transcript, because it invites
//! conclusions about a game that never happened.

use cardbench_magic_policies::{
    Archetype, DeckMatchConfig, PolicyVersion, run_deck_matchup_capturing, seat_policy,
    shared_card_index,
};
use cardbench_magic_session::{
    MatchManifest, MatchTranscript, TRANSCRIPT_SCHEMA, critique, project_events, timeline,
};

const AGGRO: &str = "rav_boros_aggro";
const MIDRANGE: &str = "rav_selesnya_midrange";

fn play(seed: u64) -> (MatchTranscript, Vec<String>) {
    let index = shared_card_index();
    let pilots = [
        seat_policy(
            PolicyVersion::latest(),
            cardbench_magic_engine::PlayerId(0),
            Archetype::Aggro,
            index.clone(),
        ),
        seat_policy(
            PolicyVersion::latest(),
            cardbench_magic_engine::PlayerId(1),
            Archetype::Midrange,
            index,
        ),
    ];
    let policies = [pilots[0].id().to_owned(), pilots[1].id().to_owned()];
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let (result, events, _identities) =
        run_deck_matchup_capturing(config, AGGRO, MIDRANGE, pilots).expect("match runs");
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
    (transcript, result.event_log)
}

/// The projection must preserve the engine's canonical log exactly. If it does
/// not, the replay digest computed over a transcript means nothing.
#[test]
fn a_transcript_reproduces_the_engine_canonical_log_exactly() {
    let (transcript, canonical) = play(3);
    assert!(!canonical.is_empty(), "the fixture must play a real game");
    assert_eq!(
        transcript.canonical(),
        canonical,
        "projection altered the canonical log"
    );
}

#[test]
fn a_transcript_round_trips_through_jsonl() {
    let (transcript, _) = play(3);
    let text = transcript.to_jsonl().expect("serialise");
    let parsed = MatchTranscript::from_jsonl(&text).expect("deserialise");
    assert_eq!(
        parsed, transcript,
        "JSONL round trip changed the transcript"
    );
    // Manifest first, then one line per event.
    assert_eq!(text.lines().count(), transcript.events.len() + 1);
}

/// Sequence numbers are the replay cursor, so they must be dense and ordered.
#[test]
fn sequences_are_dense_and_ordered() {
    let (transcript, _) = play(5);
    for (index, event) in transcript.events.iter().enumerate() {
        assert_eq!(
            event.sequence,
            u64::try_from(index).expect("index fits"),
            "sequence is not dense at {index}"
        );
    }
}

/// The whole point of projecting rather than keeping `Debug` strings: the
/// fields a reviewer queries must be typed, not parsed back out of text.
#[test]
fn structured_facts_are_populated_for_reviewable_events() {
    let (transcript, _) = play(3);

    let submissions: Vec<_> = transcript.of_kind("PolicyMoveSubmitted").collect();
    assert!(!submissions.is_empty());
    for event in &submissions {
        assert!(
            event.facts.policy.is_some(),
            "a submission receipt must name its pilot: {}",
            event.canonical
        );
        assert!(event.facts.move_kind.is_some());
        assert_eq!(event.facts.seats.len(), 1);
    }
    // Both pilot generations must appear, and be attributable.
    let pilots: std::collections::BTreeSet<&str> = submissions
        .iter()
        .filter_map(|event| event.facts.policy.as_deref())
        .collect();
    assert_eq!(
        pilots.len(),
        2,
        "both seats must be attributable: {pilots:?}"
    );

    let damage: Vec<_> = transcript.of_kind("DamageDealtToPlayer").collect();
    assert!(!damage.is_empty(), "the fixture must deal damage");
    for event in &damage {
        assert!(event.facts.amount.is_some());
        assert_eq!(event.facts.seats.len(), 1);
        assert_eq!(event.facts.objects.len(), 1, "damage names its source");
    }
}

/// Turn numbers are carried forward so a record is placeable without a join.
#[test]
fn every_event_after_the_first_step_carries_a_turn() {
    let (transcript, _) = play(3);
    let first_step = transcript
        .events
        .iter()
        .position(|event| event.kind == "StepBegan")
        .expect("a game has steps");
    for event in transcript.events.iter().skip(first_step) {
        assert!(event.turn > 0, "untagged event: {}", event.canonical);
    }
    assert!(transcript.last_turn() >= 2);
}

#[test]
fn the_timeline_accounts_for_every_turn_in_order() {
    let (transcript, _) = play(3);
    let turns = timeline(&transcript);
    assert!(!turns.is_empty());
    for pair in turns.windows(2) {
        assert!(
            pair[0].turn <= pair[1].turn,
            "timeline turns must not go backwards"
        );
    }
    let played: Vec<u32> = turns
        .iter()
        .map(|turn| turn.turn)
        .filter(|turn| *turn > 0)
        .collect();
    assert_eq!(
        played.last().copied(),
        Some(transcript.manifest.turns),
        "the timeline must reach the manifest's final turn"
    );
}

/// A clean, decisive game must not produce a Defect finding: those are
/// reserved for games that did not end by play.
#[test]
fn a_clean_decisive_game_reports_no_defect() {
    let (transcript, _) = play(3);
    assert!(transcript.manifest.termination.starts_with("Winner"));
    assert_eq!(transcript.manifest.rejected_policy_moves, 0);
    let findings = critique(&transcript);
    assert_eq!(
        findings.count(cardbench_magic_session::Severity::Defect),
        0,
        "clean game produced defect findings: {:?}",
        findings.findings
    );
}

/// The critique must be per-seat where the question is per-seat. A match-wide
/// total hides one pilot never attacking while the other attacks every turn --
/// which is exactly what this fixture does.
#[test]
fn the_critique_catches_a_seat_that_never_attacks() {
    let (transcript, _) = play(3);
    let findings = critique(&transcript);
    let passive: Vec<_> = findings
        .findings
        .iter()
        .filter(|finding| finding.code == "seat-never-attacked")
        .collect();
    assert_eq!(
        passive.len(),
        1,
        "expected exactly one passive seat in this fixture: {:?}",
        findings.findings
    );
    assert_eq!(passive[0].seat, Some(1));
}

/// Two runs of the same seed must produce byte-identical transcripts, or a
/// transcript cannot be used as evidence about a specific game.
#[test]
fn transcripts_are_deterministic_for_a_seed() {
    let (first, _) = play(7);
    let (second, _) = play(7);
    assert_eq!(first.manifest.digest, second.manifest.digest);
    assert_eq!(
        first.to_jsonl().expect("serialise"),
        second.to_jsonl().expect("serialise")
    );
}

/// The registry must cover both seats, including the loser's cards.
///
/// This is the bug the campaign stats caught in their own capture: resolving
/// identities only at the end of a match loses every object owned by the
/// departing player, because CR 800.4a removes them. Half the seats in every
/// decisive game reported zero cards drawn, which looked like a policy fact
/// and was a measurement artefact.
#[test]
fn the_card_registry_covers_the_loser_as_well_as_the_winner() {
    use cardbench_magic_session::stats::{CardIdentity, CardRegistry, summarize};

    let index = shared_card_index();
    let pilots = [
        seat_policy(
            PolicyVersion::latest(),
            cardbench_magic_engine::PlayerId(0),
            Archetype::Aggro,
            index.clone(),
        ),
        seat_policy(
            PolicyVersion::latest(),
            cardbench_magic_engine::PlayerId(1),
            Archetype::Midrange,
            index,
        ),
    ];
    let policies = [pilots[0].id().to_owned(), pilots[1].id().to_owned()];
    let config = DeckMatchConfig {
        shuffle_seed: 3,
        ..DeckMatchConfig::default()
    };
    let (result, events, identities) =
        run_deck_matchup_capturing(config, AGGRO, MIDRANGE, pilots).expect("match runs");
    assert!(
        result.winner.is_some(),
        "the fixture must produce a decisive game, so one seat departs"
    );

    let owners: std::collections::BTreeSet<usize> =
        identities.iter().map(|entry| entry.owner.0).collect();
    assert_eq!(
        owners.len(),
        2,
        "both seats' cards must be in the registry, including the loser's"
    );
    // Two 60-card decks: the registry must hold essentially all of them.
    assert!(
        identities.len() >= 120,
        "expected at least both full decks, got {}",
        identities.len()
    );

    let registry: CardRegistry = identities
        .iter()
        .map(|entry| {
            (
                entry.object.0,
                CardIdentity {
                    object: entry.object.0,
                    definition: entry.definition.to_owned(),
                    owner: u16::try_from(entry.owner.0).unwrap_or(u16::MAX),
                    is_creature: true,
                    is_land: false,
                    mana_value: 0,
                },
            )
        })
        .collect();
    let manifest = MatchManifest {
        schema_version: TRANSCRIPT_SCHEMA.to_owned(),
        decks: result.deck_ids.clone(),
        policies,
        shuffle_seed: 3,
        opening_hand_size: result.config.opening_hand_size,
        turns: result.turns,
        winner: result
            .winner
            .map(|player| u16::try_from(player.0).unwrap_or(u16::MAX)),
        termination: format!("{:?}", result.termination),
        life: result.life,
        accepted_policy_moves: result.accepted_policy_moves,
        rejected_policy_moves: 0,
        digest: result.digest.clone(),
    };
    let transcript = MatchTranscript {
        manifest,
        events: project_events(&events),
    };
    let stats = summarize(&transcript, &registry);
    for seat in &stats.seats {
        assert!(
            seat.drew_total > 0,
            "seat {} recorded no cards drawn, which no real game produces",
            seat.seat
        );
    }
}
