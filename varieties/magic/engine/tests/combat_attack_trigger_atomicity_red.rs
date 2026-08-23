//! Red regression: a rejected attacker declaration must not retain its combat prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, RulesError, Step,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const ATTACKER: &str = "TST-ATTACK-TRIGGER-ATOMIC-ATTACKER";
const TARGET: &str = "TST-ATTACK-TRIGGER-ATOMIC-TARGET";

fn creature(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-only-combat-transaction"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    for player in [
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
    ] {
        game.pass_priority(player)
            .expect("fixture priority pass advances to combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn rejected_attack_trigger_target_decision_leaves_no_combat_prefix() {
    // The data model permits this binding: every target slot has one legal
    // opponent creature.  It exceeds only the public decision's u8 range,
    // which is discovered after an attacker declaration used to mutate combat.
    let target_count = usize::from(u8::MAX) + 1;
    let binding = TriggeredAbilityBinding {
        card_definition: ATTACKER,
        ability: TriggeredAbility {
            id: "too-many-attack-trigger-targets",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::OpponentCreature; target_count],
            effects: (0..target_count)
                .map(|_| Effect::ReturnOpponentCreatureToHand)
                .collect(),
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            creature(ATTACKER, Color::Red),
            creature(TARGET, Color::Green),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("fixture initializes");
    let attacker = game
        .add_card(PlayerId(0), ATTACKER, Zone::Battlefield)
        .expect("attacker enters before the game");
    game.add_card(PlayerId(1), TARGET, Zone::Battlefield)
        .expect("one legal target exists");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates turn one");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);

    let before_events = game.canonical_event_log();
    let result = game.declare_attackers(PlayerId(0), &[attacker]);
    eprintln!(
        "oversized attack-trigger declaration: {result:?}; tapped={}; events={:?}",
        game.object(attacker)
            .expect("attacker remains known")
            .tapped,
        game.canonical_event_log()
    );
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "trigger target count exceeds decision range"
        ))
    ));
    assert!(
        !game
            .object(attacker)
            .expect("attacker remains known")
            .tapped,
        "a rejected declaration tapped the attacker before its trigger decision failed"
    );
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected declaration retained visible combat events"
    );
    game.validate_invariants()
        .expect("the rejected declaration leaves an audit-valid pre-declaration state");
}
