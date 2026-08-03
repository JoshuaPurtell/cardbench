//! Red regression: cross-target relations are checked once as resolution starts.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    Zone,
};

const FIRST: &str = "TST-EXCHANGE-SNAPSHOT-FIRST";
const SECOND: &str = "TST-EXCHANGE-SNAPSHOT-SECOND";
const SPELL: &str = "TST-EXCHANGE-SNAPSHOT-SPELL";

fn creature(id: &'static str, power: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["control-exchange-target-snapshot-red"],
        power: Some(power),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

fn spell() -> CardDefinition {
    CardDefinition {
        id: SPELL,
        name: SPELL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["control-exchange-target-snapshot-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: -2,
                toughness: 0,
            },
            Effect::ExchangeControlOfTargetCreatures,
        ],
    }
}

#[test]
fn earlier_power_change_does_not_retroactively_invalidate_a_legal_exchange_pair() {
    let mut game = Game::new([creature(FIRST, 3), creature(SECOND, 2), spell()], 2)
        .expect("fixture initializes");
    let first = game
        .put_on_battlefield(PlayerId(0), FIRST)
        .expect("first target setup");
    let second = game
        .put_on_battlefield(PlayerId(1), SECOND)
        .expect("second target setup");
    let card = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell setup");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card,
            targets: vec![
                Target::Permanent(first),
                Target::Permanent(first),
                Target::Permanent(second),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the 3-power controlled creature and 2-power opposing creature are legal targets");
    game.pass_priority(PlayerId(0))
        .expect("caster passes to resolution");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves the spell");

    eprintln!(
        "controllers=({:?}, {:?}); first_pt={:?}; events={:?}",
        game.controller_of(first),
        game.controller_of(second),
        game.characteristics(first),
        game.canonical_event_log()
    );
    assert_eq!(game.controller_of(first), Ok(PlayerId(1)));
    assert_eq!(game.controller_of(second), Ok(PlayerId(0)));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { target, .. } if *target == first
    )));
    game.validate_invariants()
        .expect("the exchange retains auditable target provenance");
}
