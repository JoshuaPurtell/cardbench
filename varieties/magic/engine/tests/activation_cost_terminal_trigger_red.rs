//! Red regression: a creature card that dies as an activation cost and whose
//! controller then loses the game must leave both the game and the
//! turn-scoped graveyard-return provenance cleanly.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, ActivatedAbility, ActivatedAbilityBinding,
    ActivatedAbilityCostBinding, CardDefinition, CardType, CastRequest, Effect, Game, GameEvent,
    GeneralizedAbilityActivation, GeneralizedActivatedAbilityCost, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const SELF_SACRIFICE: &str = "TST-ACTIVATION-COST-TERMINAL-SOURCE";
const SELF_DAMAGE: &str = "TST-ACTIVATION-COST-TERMINAL-DAMAGE";

fn creature() -> CardDefinition {
    CardDefinition {
        id: SELF_SACRIFICE,
        name: SELF_SACRIFICE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["activation-cost-terminal-trigger-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn self_damage() -> CardDefinition {
    CardDefinition {
        id: SELF_DAMAGE,
        name: SELF_DAMAGE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["activation-cost-terminal-trigger-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamage {
            amount: 19,
            target: TargetRequirement::Player,
        }],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves the stack");
}

#[test]
fn terminal_sacrifice_cost_does_not_strand_creature_graveyard_turn_provenance() {
    let mut game = Game::new_with_all_bindings(
        [creature(), self_damage()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SELF_SACRIFICE,
            ability: ActivatedAbility {
                id: "sacrifice-and-pay-life",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture constructs");
    game.register_generalized_activated_ability_cost_bindings([ActivatedAbilityCostBinding {
        card_definition: SELF_SACRIFICE,
        ability_id: "sacrifice-and-pay-life",
        cost: GeneralizedActivatedAbilityCost {
            life_payment: 1,
            counter_removals: vec![],
            return_source_to_hand: false,
            return_controlled_permanents: 0,
            detach_source_equipment: false,
            put_hand_cards_on_library_top: 0,
            exile_controller_graveyard_creature_cards: 0,
            sacrifice_land_basic_type: None,
            has_x_cost: false,
        },
    }])
    .expect("generalized life cost registers");
    let source = game
        .put_on_battlefield(PlayerId(0), SELF_SACRIFICE)
        .expect("source enters before game start");
    let damage = game
        .add_card(PlayerId(0), SELF_DAMAGE, Zone::Hand)
        .expect("damage spell enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: damage,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("self-damage spell casts");
    pass_pair(&mut game);
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 1);
    game.clear_event_log();

    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source,
                ability_id: "sacrifice-and-pay-life",
                sacrifice_sources: vec![source],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![],
                return_permanents: vec![],
                hand_cards_to_library_top: vec![],
                graveyard_cards_to_exile: vec![],
                chosen_x: None,
            },
            mana_payment_selection: None,
        },
    )
    .expect("terminal activation cost remains a legal game action");

    eprintln!(
        "terminal activation cost trace: {:?}",
        game.canonical_event_log()
    );
    assert!(game.is_game_over());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ObjectLeftGame { object, owner }
            if *object == source && *owner == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("departed creature has no stale current-turn graveyard provenance");
}
