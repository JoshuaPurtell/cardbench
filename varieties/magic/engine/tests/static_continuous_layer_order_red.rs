//! Red regression: timestamped layer-five color changes must be visible to
//! later static layer-seven characteristics effects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, ManaCost, PlayerId,
    StaticContinuousEffectBinding, Zone,
};

const SOURCE: &str = "STATIC-LAYER-SOURCE";
const TARGET: &str = "STATIC-LAYER-TARGET";
const TOP: &str = "STATIC-LAYER-TOP";

fn definition(
    id: &'static str,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["static-global-layer-order"],
        power,
        toughness,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn layer_five_color_change_precedes_static_layer_seven_color_check() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        [
            definition(
                SOURCE,
                BTreeSet::new(),
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
            ),
            definition(
                TARGET,
                BTreeSet::from([Color::Red]),
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
            ),
            definition(
                TOP,
                BTreeSet::from([Color::Green]),
                BTreeSet::from([CardType::Creature]),
                Some(1),
                Some(1),
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [StaticContinuousEffectBinding {
            card_definition: SOURCE,
            change: ContinuousChange::ControlledCreaturesSharingTopLibraryCreatureCardColorsModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
        }],
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("static source setup");
    let target = game
        .put_on_battlefield(controller, TARGET)
        .expect("target creature setup");
    game.add_card(controller, TOP, Zone::Library)
        .expect("green creature top card setup");
    game.begin_game().expect("game begins");

    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ReplaceColorsWith(Color::Green),
        Duration::EndOfTurn(game.turn),
    )
    .expect("layer-five color effect installs");

    let characteristics = game
        .characteristics(target)
        .expect("target remains on the battlefield");
    eprintln!(
        "static-layer trace: {:?}; characteristics: {characteristics:?}",
        game.canonical_event_log(),
    );
    assert_eq!(characteristics.colors, BTreeSet::from([Color::Green]));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(3), Some(3)),
        "the layer-seven static modifier must observe the layer-five color",
    );
    game.validate_invariants()
        .expect("layered characteristics state remains auditable");
}
