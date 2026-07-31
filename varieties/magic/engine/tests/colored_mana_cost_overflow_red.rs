//! Red regression: colored-cost accounting must not saturate and undercharge.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, RulesError,
    Zone,
};

const TOO_MANY_RED_SYMBOLS: &str = "TST-TOO-MANY-RED-SYMBOLS";

fn expensive_red_spell() -> CardDefinition {
    CardDefinition {
        id: TOO_MANY_RED_SYMBOLS,
        name: "too many red symbols",
        set_code: "TST",
        // Magic costs may repeat a colored symbol. This cost deliberately has
        // one more red symbol than the bounded mana-pool representation can
        // supply, so a correct engine must reject payment rather than silently
        // treating 256 required red mana as 255.
        mana_cost: ManaCost::with_colors(0, vec![Color::Red; usize::from(u8::MAX) + 1]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["life-gain"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::GainLifeController { amount: 1 }],
    }
}

#[test]
fn a_colored_cost_above_the_per_color_pool_limit_is_not_silently_underpaid() {
    let player = PlayerId(0);
    let mut game = Game::new([expensive_red_spell()], 2).expect("two-player game initializes");
    let spell = game
        .add_card(player, TOO_MANY_RED_SYMBOLS, Zone::Hand)
        .expect("expensive spell enters its owner's hand");
    game.grant_mana(player, Color::Red, u8::MAX)
        .expect("the largest representable red pool is valid setup");
    game.clear_event_log();

    let result = game.cast_spell(
        player,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    assert!(
        matches!(result, Err(RulesError::Mana(_))),
        "256 required red mana was accepted with only 255 available; result: {result:?}; events: {:?}",
        game.canonical_event_log(),
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert_eq!(game.players[player.0].mana_pool.amount(Color::Red), u8::MAX);
}
