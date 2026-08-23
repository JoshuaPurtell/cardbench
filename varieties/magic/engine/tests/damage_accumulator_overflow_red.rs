//! Red regression: multi-effect damage must not overflow before SBAs run.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const DOUBLE_DAMAGE_SPELL: &str = "TST-DOUBLE-DAMAGE";
const CREATURE: &str = "TST-DAMAGE-TARGET";

fn definitions() -> [CardDefinition; 2] {
    [
        CardDefinition {
            id: DOUBLE_DAMAGE_SPELL,
            name: "double damage spell",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: i16::MAX,
                    target: TargetRequirement::Creature,
                },
                Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                },
            ],
        },
        CardDefinition {
            id: CREATURE,
            name: "damage target",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn a_multi_effect_damage_resolution_does_not_panic_after_marking_partial_damage() {
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    let spell = game
        .add_card(PlayerId(0), DOUBLE_DAMAGE_SPELL, Zone::Hand)
        .expect("spell enters the caster's hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature starts on the battlefield");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            // Each executable damage effect has its own target occurrence.
            // Reusing the same creature is legal, but must occupy both stack
            // target slots.
            targets: vec![Target::Permanent(target), Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the model permits both representable damage effects");

    let resolution = catch_unwind(AssertUnwindSafe(|| {
        game.pass_priority(PlayerId(0))
            .expect("caster passes priority");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes and starts resolution");
    }));

    assert!(
        resolution.is_ok(),
        "damage resolution panicked after marking only the first effect; events: {:?}; target damage: {:?}",
        game.canonical_event_log(),
        game.object(target).map(|object| object.damage),
    );
}
