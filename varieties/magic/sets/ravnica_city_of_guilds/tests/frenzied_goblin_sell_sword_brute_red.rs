//! Red discovery probes for Frenzied Goblin and Sell-Sword Brute.
//!
//! These assertions deliberately fail until the target-bearing triggered-ability
//! substrate can represent both printed rules texts without approximation.

use cardbench_magic_engine::{CastRequest, Game, GameEvent, PlayerId, PolicyAction, Target, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn frenzied_goblin_requires_its_optional_paid_attack_trigger() {
    let goblin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FRENZIED-GOBLIN")
        .expect("Frenzied Goblin definition exists");
    println!(
        "Frenzied Goblin discovery: full_fidelity={}, supported_rules={:?}",
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&goblin.id),
        goblin.supported_rules
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&goblin.id),
        "Frenzied Goblin needs its attack trigger, optional {{R}} payment, creature target, and cannot-block-until-end-of-turn effect"
    );
}

#[test]
fn frenzied_goblin_attack_trigger_pays_red_and_restricts_a_blocker() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let goblin = game
        .put_on_battlefield(PlayerId(0), "RAV-FRENZIED-GOBLIN")
        .expect("Frenzied Goblin enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters");
    game.set_entered_turn_for_setup(goblin, 0)
        .expect("old fixture entry");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("old fixture entry");
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("red mana source enters before game start");
    game.begin_game().expect("game starts");
    while game.step != cardbench_magic_engine::Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[goblin])
        .expect("goblin attacks");
    game.submit_policy_move(
        PlayerId(0),
        "test.frenzied-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: goblin,
            ability: "attack-cannot-block",
            targets: vec![Target::Permanent(blocker)],
        },
    )
    .expect("choose attack-trigger target");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == goblin && *ability == "attack-cannot-block"
    )));
    game.activate_mana_ability(PlayerId(0), mountain, cardbench_magic_engine::Color::Red)
        .expect("red trigger mana after the trigger is stacked");
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("attack trigger reaches optional payment");
    game.submit_policy_move(
        PlayerId(0),
        "test.frenzied-pay.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source: goblin,
            ability: "attack-cannot-block",
            pay: true,
            target: None,
        },
    )
    .expect("pay optional red");
    assert!(
        game.characteristics(blocker)
            .expect("blocker characteristics")
            .keywords
            .contains(&cardbench_magic_engine::Keyword::CannotBlock)
    );
}

#[test]
fn frenzied_goblin_target_remains_able_to_attack() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let goblin = game
        .put_on_battlefield(PlayerId(0), "RAV-FRENZIED-GOBLIN")
        .expect("Frenzied Goblin enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters");
    game.set_entered_turn_for_setup(goblin, 0)
        .expect("old fixture entry");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("old fixture entry");
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("red mana source enters before game start");
    game.begin_game().expect("game starts");
    while game.step != cardbench_magic_engine::Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[goblin])
        .expect("goblin attacks");
    game.submit_policy_move(
        PlayerId(0),
        "test.frenzied-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: goblin,
            ability: "attack-cannot-block",
            targets: vec![Target::Permanent(blocker)],
        },
    )
    .expect("choose attack-trigger target");
    game.activate_mana_ability(PlayerId(0), mountain, cardbench_magic_engine::Color::Red)
        .expect("red trigger mana after the trigger is stacked");
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("attack trigger reaches optional payment");
    game.submit_policy_move(
        PlayerId(0),
        "test.frenzied-pay.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source: goblin,
            ability: "attack-cannot-block",
            pay: true,
            target: None,
        },
    )
    .expect("pay optional red");
    let characteristics = game.characteristics(blocker).expect("blocker remains");
    println!(
        "Frenzied Goblin target restriction: {:?}",
        characteristics.keywords
    );
    assert!(
        characteristics
            .keywords
            .contains(&cardbench_magic_engine::Keyword::CannotBlock),
        "Frenzied Goblin's paid trigger must restrict the selected creature from blocking"
    );
    assert!(
        !characteristics
            .keywords
            .contains(&cardbench_magic_engine::Keyword::CannotAttackOrBlock),
        "Frenzied Goblin should restrict blocking only; target must remain able to attack"
    );
}

#[test]
fn sell_sword_brute_requires_its_death_damage_trigger() {
    let brute = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELL-SWORD-BRUTE")
        .expect("Sell-Sword Brute definition exists");
    println!(
        "Sell-Sword Brute discovery: full_fidelity={}, supported_rules={:?}",
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&brute.id),
        brute.supported_rules
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&brute.id),
        "Sell-Sword Brute needs its dies trigger and two-damage player-or-creature target"
    );
}

#[test]
fn sell_sword_brute_dies_trigger_deals_two_to_its_controller() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let brute = game
        .put_on_battlefield(PlayerId(0), "RAV-SELL-SWORD-BRUTE")
        .expect("Sell-Sword Brute enters");
    game.set_entered_turn_for_setup(brute, 0)
        .expect("old fixture entry");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("Char enters hand");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
                .expect("opponent red mana source enters before game start")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0)).expect("pass to opponent");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, cardbench_magic_engine::Color::Red)
            .expect("activate opponent red mana source");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(brute)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char casts");
    game.pass_priority(PlayerId(1)).expect("caster passes");
    game.pass_priority(PlayerId(0)).expect("Char resolves");
    assert_eq!(game.zone_of(brute), Some(Zone::Graveyard));
    let trigger_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == brute && *ability == "dies-deal-two-to-controller"
            )
        })
        .expect("dies trigger stacks");
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    assert!(game.event_log.iter().enumerate().any(|(index, event)| {
        index > trigger_index
            && matches!(
                event,
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(0),
                    amount: 2,
                } if *source == brute
            )
    }));
    assert_eq!(
        game.player(PlayerId(0)).expect("controller exists").life,
        18
    );
}
