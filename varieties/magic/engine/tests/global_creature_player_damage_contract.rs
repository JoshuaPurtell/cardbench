//! Expansion-neutral contracts for one-pass global creature-and-player damage.
//!
//! These synthetic fixtures ensure the effect has no target, snapshots every
//! creature before state-based actions, skips noncreatures, and records a
//! deterministic receipt for each affected game object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    RulesError, Target, Zone,
};

const SWEEP: &str = "TST-SWEEP";
const SMALL: &str = "TST-SMALL";
const LARGE: &str = "TST-LARGE";
const ARTIFACT: &str = "TST-ARTIFACT";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn permanent(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    toughness: i16,
) -> CardDefinition {
    let card_types = types(card_types);
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn game() -> Game {
    Game::new(
        vec![
            CardDefinition {
                id: SWEEP,
                name: SWEEP,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: colors([Color::Red]),
                mana_colors: BTreeSet::new(),
                card_types: types([CardType::Sorcery]),
                is_basic_land: false,
                supported_rules: &["global-creature-and-player-damage"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
            },
            permanent(SMALL, [CardType::Creature], 1),
            permanent(LARGE, [CardType::Creature], 2),
            permanent(ARTIFACT, [CardType::Artifact], 0),
        ],
        2,
    )
    .expect("global-damage contract game initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player resolves the spell");
}

#[test]
fn global_damage_snapshots_all_creatures_before_sbas_and_hits_each_player() {
    let mut game = game();
    let sweep = game
        .add_card(PlayerId(0), SWEEP, Zone::Hand)
        .expect("sweep enters hand");
    let small = game
        .put_on_battlefield(PlayerId(0), SMALL)
        .expect("small creature enters");
    let large = game
        .put_on_battlefield(PlayerId(1), LARGE)
        .expect("large creature enters");
    let artifact = game
        .put_on_battlefield(PlayerId(1), ARTIFACT)
        .expect("artifact enters");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sweep,
            targets: vec![],
            convoke: vec![],
        },
    )
    .expect("target-free global spell casts");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(sweep), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(small), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(large), Some(Zone::Battlefield));
    assert_eq!(game.object(large).expect("large remains").damage, 1);
    assert_eq!(game.zone_of(artifact), Some(Zone::Battlefield));
    assert_eq!(game.object(artifact).expect("artifact remains").damage, 0);
    assert_eq!(game.players[0].life, 19);
    assert_eq!(game.players[1].life, 19);

    let damage_events = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, amount, .. } if *source == sweep && *amount == 1))
        .count();
    assert_eq!(damage_events, 2, "every creature gets one receipt");
    let last_damage = game
        .event_log
        .iter()
        .rposition(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, amount, .. } if *source == sweep && *amount == 1))
        .expect("player-damage receipts exist");
    let first_sba = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::StateBasedAction { .. }))
        .expect("lethal small creature triggers an SBA");
    assert!(
        last_damage < first_sba,
        "all recipients take damage before SBAs"
    );
    game.validate_invariants()
        .expect("global damage trace preserves invariants");
}

#[test]
fn global_damage_rejects_spurious_targets_without_mutating_state() {
    let mut game = game();
    let sweep = game
        .add_card(PlayerId(0), SWEEP, Zone::Hand)
        .expect("sweep enters hand");

    assert_eq!(
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: sweep,
                targets: vec![Target::Player(PlayerId(1))],
                convoke: vec![],
            },
        ),
        Err(RulesError::IllegalAction(
            "this spell does not take targets"
        ))
    );
    assert_eq!(game.zone_of(sweep), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("rejected global spell leaves state valid");
}
