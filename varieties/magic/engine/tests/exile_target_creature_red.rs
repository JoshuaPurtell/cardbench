//! RED: the engine lacks a first-class exile-target-creature instruction.
//!
//! This is the smallest substrate needed by RAV's Devouring Light.  The
//! target must leave the battlefield for exile (not graveyard), and its
//! normal stack/event lifecycle must remain intact.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    Target, TargetRequirement, Zone,
};

const EXILE: &str = "TST-EXILE-CREATURE";
const CREATURE: &str = "TST-CREATURE";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn game() -> Game {
    Game::new(
        vec![
            CardDefinition {
                id: EXILE,
                name: EXILE,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::from([Color::White]),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Instant]),
                is_basic_land: false,
                supported_rules: &["exile-target-creature"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![Effect::ExileTargetCreature],
            },
            creature(CREATURE),
        ],
        2,
    )
    .expect("exile fixture initializes")
}

#[test]
fn exile_target_creature_moves_a_legal_target_to_exile() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), EXILE, Zone::Hand)
        .expect("spell enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature enters battlefield");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted instant casts");
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("resolution pass");

    assert_eq!(game.zone_of(target), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if *card == target)
    }));
    game.validate_invariants().expect("exile trace is valid");
}

#[test]
fn exile_target_creature_requires_a_creature_target() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), EXILE, Zone::Hand)
        .expect("spell enters hand");
    let target = game
        .add_card(PlayerId(1), EXILE, Zone::Battlefield)
        .expect("noncreature enters battlefield");

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(result.is_err(), "noncreature target must be rejected");
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
}

#[test]
fn exile_target_creature_rechecks_repeated_targets_after_zone_change() {
    let mut game = Game::new(
        vec![
            CardDefinition {
                id: EXILE,
                name: EXILE,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::from([Color::White]),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Instant]),
                is_basic_land: false,
                supported_rules: &["exile-target-creature"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![Effect::ExileTargetCreature, Effect::ExileTargetCreature],
            },
            creature(CREATURE),
        ],
        2,
    )
    .expect("repeated-target fixture initializes");
    let spell = game
        .add_card(PlayerId(0), EXILE, Zone::Hand)
        .expect("spell enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature enters battlefield");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target), Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("repeated target casts");
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("resolution pass");

    assert_eq!(game.zone_of(target), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::TargetInstructionSkipped { card, effect_index: 1, target: Target::Permanent(id) } if *card == spell && *id == target)
    }));
    game.validate_invariants()
        .expect("repeated-target trace is valid");
}

#[allow(dead_code)]
const _: Option<TargetRequirement> = Some(TargetRequirement::Creature);
