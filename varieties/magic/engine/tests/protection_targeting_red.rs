//! Red regression: protection from a color must make a colored spell target illegal.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, Keyword, ManaCost, PlayerId,
    Target, Zone,
};

const PROTECTED: &str = "PROTECTION-TARGET";
const RED_BOLT: &str = "PROTECTION-RED-BOLT";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: PROTECTED,
            name: PROTECTED,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["protection-from-red"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Protection(Color::Red)],
            effects: vec![],
        },
        CardDefinition {
            id: RED_BOLT,
            name: RED_BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["protection-targeting-probe"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: cardbench_magic_engine::TargetRequirement::Creature,
            }],
        },
    ]
}

#[test]
fn protection_from_red_rejects_red_spell_target_before_cast() {
    let caster = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let protected = game
        .put_on_battlefield(caster, PROTECTED)
        .expect("protected creature enters");
    let bolt = game
        .add_card(caster, RED_BOLT, Zone::Hand)
        .expect("red spell enters hand");

    let result = game.cast_spell(
        caster,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(protected)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    eprintln!(
        "protection target red result: {result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        result.is_err(),
        "a red spell must not be cast targeting a permanent with protection from red"
    );
    assert!(
        game.stack.is_empty(),
        "illegal target must not reach the stack"
    );
    assert!(
        game.canonical_event_log().is_empty(),
        "illegal target rejection must not emit gameplay receipts"
    );
    game.validate_invariants()
        .expect("the rejected target leaves a valid fixture state");
}
