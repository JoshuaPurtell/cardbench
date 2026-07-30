//! Regression probe for a colorless creature targeted by a radiance effect.
//!
//! A target receives the effect independently of whether it shares a color
//! with another creature. This public transition exercises the already-shipped
//! radiance-and-untap semantic operation without any RAV card fixture.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target, Zone,
};

const COLORLESS_CREATURE: &str = "TEST-COLORLESS-CREATURE";
const RADIANCE: &str = "TEST-RADIANCE";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: COLORLESS_CREATURE,
            name: COLORLESS_CREATURE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: RADIANCE,
            name: RADIANCE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["radiance", "layer-7-modifier", "untap"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }],
        },
    ]
}

#[test]
fn colorless_radiance_target_receives_its_modifier() {
    let caster = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let spell = game
        .add_card(caster, RADIANCE, Zone::Hand)
        .expect("radiance spell enters hand");
    let target = game
        .put_on_battlefield(caster, COLORLESS_CREATURE)
        .expect("colorless creature enters battlefield");

    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
        },
    )
    .expect("targeted radiance cast is legal");
    game.pass_priority(caster)
        .expect("caster passes priority");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes priority and resolves the spell");

    assert_eq!(
        game.characteristics(target)
            .expect("target remains on the battlefield")
            .power,
        Some(4),
        "the target receives the radiance modifier even with no colors"
    );
    game.validate_invariants()
        .expect("the resolved public state remains internally consistent");
}
