//! Plays a campaign and reports what the pilots actually did, per deck.
//!
//! Usage: `rav-campaign-stats [SEEDS]` (default 30).
//!
//! Win rates say which deck is better. These say *why*, and they exist because
//! two policy-improvement leads in a row came from inference rather than
//! counting: a six-game sample and a static card list, both wrong.

use cardbench_magic_engine::CardType;
use cardbench_magic_policies::{
    Archetype, DeckMatchConfig, PolicyVersion, run_deck_matchup_capturing, seat_policy,
    shared_card_index,
};
use cardbench_magic_rav::{card_definitions, load_constructed_decks};
use cardbench_magic_session::stats::{
    CardIdentity, CardRegistry, MatchStats, aggregate, summarize,
};
use cardbench_magic_session::{MatchManifest, MatchTranscript, TRANSCRIPT_SCHEMA, project_events};
use std::collections::BTreeMap;

#[allow(clippy::too_many_lines)] // One straight-line campaign driver.
fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);

    let decks = match load_constructed_decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("failed to load constructed decks: {error}");
            std::process::exit(1);
        }
    };
    // Card types come from the catalog, so a statistic never depends on a
    // hand-maintained copy of what a card is.
    let types: BTreeMap<&'static str, (bool, bool, u8)> = card_definitions()
        .into_iter()
        .map(|definition| {
            let mana_value = definition.mana_cost.generic
                + u8::try_from(definition.mana_cost.colored.len()).unwrap_or(0)
                + u8::try_from(definition.mana_cost.hybrid.len()).unwrap_or(0);
            (
                definition.id,
                (
                    definition.card_types.contains(&CardType::Creature),
                    definition.card_types.contains(&CardType::Land),
                    mana_value,
                ),
            )
        })
        .collect();

    let index = shared_card_index();
    let version = PolicyVersion::latest();
    let mut all: Vec<MatchStats> = Vec::new();

    println!("schema_version=cardbench.magic.campaign-stats.v1");
    println!("policy_version={version} seeds={seeds}");

    for (left, right) in pairings(&decks) {
        for seed in 0..seeds {
            let archetypes = [archetype_of(&decks, &left), archetype_of(&decks, &right)];
            let pilots = [
                seat_policy(
                    version,
                    cardbench_magic_engine::PlayerId(0),
                    archetypes[0],
                    index.clone(),
                ),
                seat_policy(
                    version,
                    cardbench_magic_engine::PlayerId(1),
                    archetypes[1],
                    index.clone(),
                ),
            ];
            let policies = [pilots[0].id().to_owned(), pilots[1].id().to_owned()];
            let config = DeckMatchConfig {
                shuffle_seed: seed,
                ..DeckMatchConfig::default()
            };
            let Ok((result, events, identities)) =
                run_deck_matchup_capturing(config, &left, &right, pilots)
            else {
                eprintln!("match failed: {left} vs {right} seed {seed}");
                std::process::exit(1);
            };
            let registry: CardRegistry = identities
                .into_iter()
                .map(|identity| {
                    let (is_creature, is_land, mana_value) =
                        types.get(identity.definition).copied().unwrap_or_default();
                    (
                        identity.object.0,
                        CardIdentity {
                            object: identity.object.0,
                            definition: identity.definition.to_owned(),
                            owner: u16::try_from(identity.owner.0).unwrap_or(u16::MAX),
                            is_creature,
                            is_land,
                            mana_value,
                        },
                    )
                })
                .collect();
            let transcript = MatchTranscript {
                manifest: MatchManifest {
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
                },
                events: project_events(&events),
            };
            all.push(summarize(&transcript, &registry));
        }
    }

    println!("games={}", all.len());
    println!(
        "\n{:<24} {:>5} {:>5} {:>6} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>7} {:>7}",
        "deck",
        "games",
        "wins",
        "turns",
        "drewCr",
        "castCr",
        "conv",
        "lands",
        "mana",
        "atks",
        "blks",
        "abils",
        "noAtk%"
    );
    for row in aggregate(&all) {
        println!(
            "{:<24} {:>5} {:>5} {:>6.1} {:>7.1} {:>7.1} {:>6.2} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {:>7.2} {:>7.1}",
            row.deck,
            row.games,
            row.wins,
            row.mean_turns,
            row.mean_drew_creatures,
            row.mean_cast_creatures,
            row.creature_conversion,
            row.mean_lands_played,
            row.mean_mana_produced,
            row.mean_attacks,
            row.mean_blocks,
            row.mean_abilities,
            row.never_attacked_rate * 100.0,
        );
    }
}

fn pairings(decks: &[cardbench_magic_rav::DeckFixture]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (index, left) in decks.iter().enumerate() {
        for right in decks.iter().skip(index) {
            out.push((left.id.clone(), right.id.clone()));
        }
    }
    out
}

fn archetype_of(decks: &[cardbench_magic_rav::DeckFixture], id: &str) -> Archetype {
    decks
        .iter()
        .find(|deck| deck.id == id)
        .and_then(|deck| Archetype::parse(&deck.archetype))
        .unwrap_or(Archetype::Midrange)
}
