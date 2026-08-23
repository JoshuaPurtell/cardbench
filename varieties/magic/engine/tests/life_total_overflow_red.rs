use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const LIFE_GAIN_TEST_SPELL: &str = "LIFE-GAIN-TEST-SPELL";

fn life_gain_spell() -> CardDefinition {
    CardDefinition {
        id: LIFE_GAIN_TEST_SPELL,
        name: "life gain test spell",
        set_code: "TST",
        mana_cost: ManaCost::with_colors(0, [Color::White, Color::Red]),
        colors: BTreeSet::from([Color::White, Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["damage", "life-gain"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::Player,
            },
            Effect::GainLifeController { amount: 3 },
        ],
    }
}

#[test]
fn resolving_life_gain_at_the_representable_ceiling_is_not_a_partial_panicking_transition() {
    let mut game = Game::new([life_gain_spell()], 2).expect("two-player game initializes");
    let helix = game
        .add_card(PlayerId(0), LIFE_GAIN_TEST_SPELL, Zone::Hand)
        .expect("life-gain instant enters the caster's hand");
    game.players[PlayerId(0).0].life = i64::from(i16::MAX);
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white mana is granted");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana is granted");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the spell is legal before it resolves");

    let resolution = catch_unwind(AssertUnwindSafe(|| {
        game.pass_priority(PlayerId(0))
            .expect("caster passes priority");
        game.pass_priority(PlayerId(1))
            .expect("opponent's pass resolves the spell");
    }));

    assert!(
        resolution.is_ok(),
        "life gain panicked instead of producing an engine-defined result; events: {:?}",
        game.canonical_event_log()
    );
}
