//! Red regression: the stack must retain every independently chosen target.
//!
//! A Seeds-of-Strength-style spell has three separate instances of the word
//! "target".  They are selected while casting, may name the same creature, and
//! resolve independently: if one becomes illegal, the legal targets still get
//! their modifiers.  The initial engine slice incorrectly collapsed all
//! targeted effects to one target slot.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target, Zone,
};

const THREE_TARGET_BOOST: &str = "TST-THREE-TARGET-BOOST";
const CREATURE: &str = "TST-TARGET-CREATURE";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn game() -> Game {
    Game::new(
        [
            CardDefinition {
                id: THREE_TARGET_BOOST,
                name: THREE_TARGET_BOOST,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: colors([Color::Green]),
                mana_colors: BTreeSet::new(),
                card_types: types([CardType::Instant]),
                is_basic_land: false,
                supported_rules: &["three-independent-targeted-modifiers"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![
                    Effect::ModifyTargetPtUntilEndOfTurn {
                        power: 1,
                        toughness: 1,
                    },
                    Effect::ModifyTargetPtUntilEndOfTurn {
                        power: 1,
                        toughness: 1,
                    },
                    Effect::ModifyTargetPtUntilEndOfTurn {
                        power: 1,
                        toughness: 1,
                    },
                ],
            },
            CardDefinition {
                id: CREATURE,
                name: CREATURE,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: colors([Color::Green]),
                mana_colors: BTreeSet::new(),
                card_types: types([CardType::Creature]),
                is_basic_land: false,
                supported_rules: &["base-characteristics"],
                power: Some(2),
                toughness: Some(2),
                keywords: vec![],
                effects: vec![],
            },
        ],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn independently_targeted_effects_survive_one_target_becoming_illegal() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(caster, THREE_TARGET_BOOST, Zone::Hand)
        .expect("boost enters hand");
    let first = game
        .put_on_battlefield(caster, CREATURE)
        .expect("first creature enters battlefield");
    let middle = game
        .put_on_battlefield(caster, CREATURE)
        .expect("middle creature enters battlefield");
    let last = game
        .put_on_battlefield(caster, CREATURE)
        .expect("last creature enters battlefield");

    let cast = game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![
                Target::Permanent(first),
                Target::Permanent(middle),
                Target::Permanent(last),
            ],
            convoke: vec![],
        },
    );
    eprintln!(
        "three-target cast result: {cast:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        cast.is_ok(),
        "each target occurrence must have its own stack target slot"
    );

    game.players[caster.0]
        .battlefield
        .retain(|card| *card != middle);
    game.players[caster.0].graveyard.push(middle);
    game.pass_priority(caster)
        .expect("caster passes priority over spell");
    game.pass_priority(opponent)
        .expect("opponent resolves the spell");

    assert_eq!(
        game.characteristics(first).expect("first exists").power,
        Some(3)
    );
    assert_eq!(
        game.characteristics(last).expect("last exists").power,
        Some(3)
    );
    assert_eq!(game.zone_of(middle), Some(Zone::Graveyard));
}
