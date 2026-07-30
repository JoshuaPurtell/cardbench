//! Red regression probe: public continuous-effect installation must stabilize SBAs.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, GameEvent, ManaCost,
    PlayerId, Zone,
};

const SOURCE: &str = "CONTINUOUS-SBA-SOURCE";
const FRAGILE: &str = "CONTINUOUS-SBA-FRAGILE";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn creature(id: &'static str, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: [CardType::Creature].into_iter().collect(),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(1),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn public_continuous_effect_installation_stabilizes_lethal_toughness() {
    let player = PlayerId(0);
    let mut game = Game::new(vec![creature(SOURCE, 2), creature(FRAGILE, 1)], 2)
        .expect("test game initializes");
    let source = game
        .put_on_battlefield(player, SOURCE)
        .expect("source enters battlefield");
    let fragile = game
        .put_on_battlefield(player, FRAGILE)
        .expect("one-toughness creature enters battlefield");

    game.add_continuous_effect(
        source,
        fragile,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -1,
        },
        Duration::Permanent,
    )
    .expect("the legal effect installs");

    eprintln!(
        "continuous-install trace: zone={:?}; events={:?}",
        game.zone_of(fragile),
        game.canonical_event_log()
    );
    assert_eq!(
        game.zone_of(fragile),
        Some(Zone::Graveyard),
        "a completed public transition left a zero-toughness creature on the battlefield"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::StateBasedAction { card, reason: "creature has toughness zero or less" }
            if *card == fragile
    )));
}
