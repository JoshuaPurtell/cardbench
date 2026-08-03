//! Red regression: a terminal life-payment land entry cannot roll back merely
//! because its otherwise valid ETB trigger source left with its controller.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, LandEntryBinding, ManaCost,
    PlayerId, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SHOCK_LAND: &str = "TST-TERMINAL-LAND-ENTRY";
const SELF_DAMAGE: &str = "TST-TERMINAL-LAND-ENTRY-DAMAGE";

fn shock_land() -> CardDefinition {
    CardDefinition {
        id: SHOCK_LAND,
        name: SHOCK_LAND,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &["terminal-land-entry-etb-red"],
        power: None,
        toughness: None,
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
        supported_rules: &["terminal-land-entry-etb-red"],
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

fn advance_to_precombat_main(game: &mut Game) {
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("players advance through the opening automatic steps");
    }
}

#[test]
fn terminal_land_entry_omits_its_departed_etb_trigger_without_rollback() {
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        [shock_land(), self_damage()],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: SHOCK_LAND,
            ability: TriggeredAbility {
                id: "gain-one-on-entry",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
        [],
        [LandEntryBinding {
            card_definition: SHOCK_LAND,
            enters_tapped: false,
            optional_life_payment: Some(1),
        }],
    )
    .expect("fixture constructs");
    let damage = game
        .add_card(PlayerId(0), SELF_DAMAGE, Zone::Hand)
        .expect("self-damage spell begins in hand");
    let land = game
        .add_card(PlayerId(0), SHOCK_LAND, Zone::Hand)
        .expect("life-payment land begins in hand");
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
    assert_eq!(game.player(PlayerId(0)).expect("player lives").life, 1);
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    let result = game.play_land_with_entry_life_payment(PlayerId(0), land, true);

    eprintln!(
        "terminal land-entry result={result:?}; trace={:?}",
        game.canonical_event_log()
    );
    result.expect("the legal terminal land play must commit rather than roll back");
    assert!(game.is_game_over());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LandEntryLifePaid { player, card, amount }
            if *player == PlayerId(0) && *card == land && *amount == 1
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ObjectLeftGame { object, owner }
            if *object == land && *owner == PlayerId(0)
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == land && *ability == "gain-one-on-entry"
        )),
        "a lost controller cannot put the land's ETB trigger onto the stack",
    );
    game.validate_invariants()
        .expect("terminal land entry leaves no stale trigger state");
}
