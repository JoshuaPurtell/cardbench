//! Red regression: live activated-cost modifiers must calculate the full
//! payable cost before a nonmana ability mutates game state.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostModifier,
    ActivatedAbilityCostModifierBinding, ActivatedManaAbility, CardDefinition, CardType, Color,
    Game, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost,
    PlayerId, RulesError, Zone,
};

const TAXER: &str = "ACTIVATED-COST-RED-TAXER";
const ABILITY_SOURCE: &str = "ACTIVATED-COST-RED-ABILITY-SOURCE";
const COSTLY_SOURCE: &str = "ACTIVATED-COST-RED-COSTLY-SOURCE";
const DISCARD: &str = "ACTIVATED-COST-RED-DISCARD";
const MANA_SOURCE: &str = "ACTIVATED-COST-RED-MANA-SOURCE";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["activated-cost-modification-probe"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn fixture() -> Game {
    let mut game = Game::new_with_all_bindings(
        vec![
            creature(TAXER),
            creature(ABILITY_SOURCE),
            creature(COSTLY_SOURCE),
            creature(MANA_SOURCE),
            CardDefinition {
                card_types: BTreeSet::from([CardType::Instant]),
                power: None,
                toughness: None,
                ..creature(DISCARD)
            },
        ],
        2,
        [ManaAbilityBinding {
            card_definition: MANA_SOURCE,
            ability: ActivatedManaAbility {
                id: "paid-bundle",
                tap_cost: false,
                output: ManaAbilityOutput::PaidBundle {
                    mana_cost: ManaCost::new(1),
                    bundle: ManaBundle::new([(Color::Blue, 1)]),
                },
                amount: 0,
                life_payment: None,
                controller_damage: None,
            },
        }],
        [],
        [],
        [
            ActivatedAbilityBinding {
                card_definition: TAXER,
                ability: ActivatedAbility {
                    id: "sacrifice-self",
                    mana_cost: ManaCost::new(0),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: true,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![],
                },
            },
            ActivatedAbilityBinding {
                card_definition: ABILITY_SOURCE,
                ability: ActivatedAbility {
                    id: "red-ability",
                    mana_cost: ManaCost::with_colors(1, [Color::Red]),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![],
                },
            },
            ActivatedAbilityBinding {
                card_definition: COSTLY_SOURCE,
                ability: ActivatedAbility {
                    id: "atomic-costs",
                    mana_cost: ManaCost::new(0),
                    tap_cost: true,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: true,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 1,
                    targets: vec![],
                    effects: vec![],
                },
            },
        ],
    )
    .expect("fixture game initializes");
    game.register_activated_ability_cost_modifier_bindings([ActivatedAbilityCostModifierBinding {
        source_definition: TAXER,
        modifier: ActivatedAbilityCostModifier::IncreaseGeneric {
            amount: 2,
            nonmana_only: true,
        },
    }])
    .expect("tax binding registers before the game begins");
    game
}

fn activate(game: &mut Game, source: cardbench_magic_engine::ObjectId, ability: &'static str) {
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: ability,
            sacrifice_sources: if ability == "sacrifice-self" {
                vec![source]
            } else {
                vec![]
            },
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("activation succeeds");
}

#[test]
fn nonmana_tax_stacks_and_preserves_colored_symbols() {
    let mut game = fixture();
    let player = PlayerId(0);
    game.put_on_battlefield(player, TAXER)
        .expect("first taxer enters");
    game.put_on_battlefield(player, TAXER)
        .expect("second taxer enters");
    let source = game
        .put_on_battlefield(player, ABILITY_SOURCE)
        .expect("ability source enters");
    game.grant_mana(player, Color::Red, 1)
        .expect("red mana setup");
    game.grant_mana(player, Color::Colorless, 4)
        .expect("four generic mana setup");

    let result = game.activate_ability(
        player,
        AbilityActivation {
            source,
            ability_id: "red-ability",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    );
    eprintln!(
        "activated-cost red tax result={result:?}; pool={:?}; events={:?}",
        game.player(player).expect("player exists").mana_pool,
        game.canonical_event_log()
    );

    assert!(
        matches!(result, Err(RulesError::Mana(_))),
        "two live +2 generic taxes must turn {{1}}{{R}} into an unaffordable {{5}}{{R}} cost"
    );
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Red),
        1,
        "a rejected effective cost must preserve the colored payment"
    );
    assert!(
        game.stack.is_empty(),
        "failed payment must not place an ability"
    );
}

#[test]
fn nonmana_cost_is_preflighted_before_tap_sacrifice_or_discard() {
    let mut game = fixture();
    let player = PlayerId(0);
    game.put_on_battlefield(player, TAXER)
        .expect("taxer enters");
    let source = game
        .put_on_battlefield(player, COSTLY_SOURCE)
        .expect("costly source enters");
    let discard = game
        .add_card(player, DISCARD, Zone::Hand)
        .expect("discard card enters hand");
    game.set_entered_turn_for_setup(source, 0)
        .expect("fixture source has been controlled since an earlier turn");
    game.grant_mana(player, Color::Colorless, 1)
        .expect("insufficient mana setup");
    let events_before = game.canonical_event_log();

    let result = game.activate_ability(
        player,
        AbilityActivation {
            source,
            ability_id: "atomic-costs",
            sacrifice_sources: vec![source],
            additional_tap_creatures: vec![],
            discard_cards: vec![discard],
            targets: vec![],
        },
    );
    eprintln!(
        "activated-cost atomicity result={result:?}; source_zone={:?}; discard_zone={:?}; events={:?}",
        game.zone_of(source),
        game.zone_of(discard),
        game.canonical_event_log()
    );

    assert!(matches!(result, Err(RulesError::Mana(_))));
    assert_eq!(game.zone_of(source), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(discard), Some(Zone::Hand));
    assert_eq!(game.canonical_event_log(), events_before);
}

#[test]
fn tax_does_not_apply_to_mana_abilities_and_departure_revokes_it() {
    let mut game = fixture();
    let player = PlayerId(0);
    let taxer = game
        .put_on_battlefield(player, TAXER)
        .expect("taxer enters");
    let mana_source = game
        .put_on_battlefield(player, MANA_SOURCE)
        .expect("mana source enters");
    let ability_source = game
        .put_on_battlefield(player, ABILITY_SOURCE)
        .expect("ordinary ability source enters");
    game.grant_mana(player, Color::Colorless, 3)
        .expect("tax and mana-ability payment setup");
    game.grant_mana(player, Color::Red, 1)
        .expect("colored payment setup");

    game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source: mana_source,
            ability_id: "paid-bundle",
            chosen_color: None,
        },
    )
    .expect("nonmana-only tax does not change a mana ability's {1} cost");
    activate(&mut game, taxer, "sacrifice-self");
    assert_eq!(game.zone_of(taxer), Some(Zone::Graveyard));
    activate(&mut game, ability_source, "red-ability");
    game.validate_invariants()
        .expect("source departure revokes the tax cleanly");
}
