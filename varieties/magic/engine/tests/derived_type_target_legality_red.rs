//! Red regression: target legality must read the current type layer rather
//! than the printed type line.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Duration, Effect, Game, ManaCost,
    PlayerId, Target, Zone,
};

const TYPE_SOURCE: &str = "DERIVED-TYPE-TARGET-SOURCE";
const CREATURE: &str = "DERIVED-TYPE-TARGET-CREATURE";
const DESTROY_LAND: &str = "DERIVED-TYPE-TARGET-DESTROY-LAND";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: card_types.clone(),
        is_basic_land: false,
        supported_rules: &["derived-type-target-legality-probe"],
        power: card_types.contains(&CardType::Creature).then_some(2),
        toughness: card_types.contains(&CardType::Creature).then_some(2),
        keywords: vec![],
        effects,
    }
}

#[test]
fn a_creature_made_into_a_land_is_a_legal_land_target() {
    let player = PlayerId(0);
    let mut game = Game::new(
        [
            definition(TYPE_SOURCE, BTreeSet::from([CardType::Enchantment]), vec![]),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                DESTROY_LAND,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetLand],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(player, TYPE_SOURCE)
        .expect("type-changing source begins on the battlefield");
    let target = game
        .put_on_battlefield(player, CREATURE)
        .expect("creature begins on the battlefield");
    let destroy = game
        .add_card(player, DESTROY_LAND, Zone::Hand)
        .expect("land-destruction spell begins in hand");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddCardType(CardType::Land),
        Duration::Permanent,
    )
    .expect("a legal type-layer effect makes the creature a land");
    assert!(
        game.characteristics(target)
            .expect("target has derived characteristics")
            .card_types
            .contains(&CardType::Land),
        "the live type layer, not the catalog definition, says the target is a land",
    );

    game.cast_spell(
        player,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("a live derived land is a legal target for destroy-target-land");
}
