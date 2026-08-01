//! Red contract for typed colorless mana.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CastRequest, Color, Game, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaCost, PlayerId, Zone,
};

const COLORLESS_LAND: &str = "TST-COLORLESS-LAND";
const GENERIC_SPELL: &str = "TST-GENERIC-SPELL";
const RED_SPELL: &str = "TST-RED-SPELL";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: COLORLESS_LAND,
            name: COLORLESS_LAND,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Colorless]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: false,
            supported_rules: &["typed-colorless-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: GENERIC_SPELL,
            name: GENERIC_SPELL,
            set_code: "TST",
            mana_cost: ManaCost::new(1),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["generic-cost"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: RED_SPELL,
            name: RED_SPELL,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["colored-cost"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn colorless_mana_pays_generic_costs_but_not_colored_symbols() {
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [ManaAbilityBinding {
            card_definition: COLORLESS_LAND,
            ability: ActivatedManaAbility {
                id: "produce-colorless",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Colorless),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        }],
    )
    .expect("typed colorless binding is valid");
    let generic = game
        .add_card(PlayerId(0), GENERIC_SPELL, Zone::Hand)
        .expect("generic spell starts in hand");
    let red = game
        .add_card(PlayerId(0), RED_SPELL, Zone::Hand)
        .expect("red spell starts in hand");
    let first_land = game
        .put_on_battlefield(PlayerId(0), COLORLESS_LAND)
        .expect("first land starts on battlefield");
    let second_land = game
        .put_on_battlefield(PlayerId(0), COLORLESS_LAND)
        .expect("second land starts on battlefield");
    game.begin_game().expect("game begins");

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: first_land,
            ability_id: "produce-colorless",
            chosen_color: None,
        },
    )
    .expect("typed colorless mana is produced");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").mana_pool.amount(Color::Colorless), 1);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: generic,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("colorless pays one generic symbol");

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: second_land,
            ability_id: "produce-colorless",
            chosen_color: None,
        },
    )
    .expect("a second typed colorless mana is produced");
    assert!(game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: red,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .is_err());
    game.validate_invariants()
        .expect("colorless payment boundary is invariant-valid");
}
