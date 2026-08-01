//! Red discovery regression for Ivy Dancer's targeted Forestwalk activation.

use cardbench_magic_engine::{
    AbilityActivation, BasicLandType, CombatBlock, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn ivy_dancer_requires_its_targeted_tap_forestwalk_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-IVY-DANCER")
        .expect("Ivy Dancer definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the targeted temporary Forestwalk activation is required"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"tap-target-creature-grant-forestwalk")
    );

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-IVY-DANCER"
                && binding.ability.id == "tap-target-creature-grant-forestwalk"
        })
        .expect("Ivy Dancer targeted Forestwalk binding exists");
    assert_eq!(ability.ability.mana_cost, ManaCost::new(0));
    assert!(ability.ability.tap_cost);
    assert_eq!(ability.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        ability.ability.effects,
        [Effect::ModifyTargetKeywordUntilEndOfTurn {
            keyword: Keyword::Landwalk(BasicLandType::Forest),
        }]
    );
}

#[test]
fn ivy_dancer_grants_forestwalk_to_exactly_one_creature_for_this_turn() {
    let mut game = game_with_rav_bindings();
    let ivy_dancer = game
        .put_on_battlefield(PlayerId(0), "RAV-IVY-DANCER")
        .expect("Ivy Dancer begins on the battlefield");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker begins on the battlefield");
    game.put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("defender controls a Forest");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker begins on the battlefield");
    for creature in [ivy_dancer, attacker, blocker] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature predates the measured turn");
    }
    game.begin_game().expect("fixture begins game");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: ivy_dancer,
            ability_id: "tap-target-creature-grant-forestwalk",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(attacker)],
        },
    )
    .expect("Ivy Dancer ability enters the stack");
    resolve_top(&mut game);
    assert!(
        game.characteristics(attacker)
            .expect("target characteristics")
            .keywords
            .contains(&Keyword::Landwalk(BasicLandType::Forest))
    );
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to combat");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Forestwalking attacker declares");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blocker declaration");
    let before = game.event_log.len();
    assert!(
        game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
            .is_err(),
        "a defender controlling Forest cannot block the Forestwalking attacker"
    );
    assert_eq!(game.event_log.len(), before);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == ivy_dancer && *target == attacker
    )));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
    println!("ivy_dancer_event_log={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Ivy Dancer trace preserves invariants");
}

#[test]
fn ivy_dancer_rejects_a_noncreature_target_before_its_tap_cost() {
    let mut game = game_with_rav_bindings();
    let ivy_dancer = game
        .put_on_battlefield(PlayerId(0), "RAV-IVY-DANCER")
        .expect("Ivy Dancer begins on the battlefield");
    let land = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("land begins on the battlefield");
    game.set_entered_turn_for_setup(ivy_dancer, 0)
        .expect("Ivy Dancer predates the measured turn");
    game.begin_game().expect("fixture begins game");
    game.clear_event_log();
    assert!(
        game.activate_ability(
            PlayerId(0),
            AbilityActivation {
                source: ivy_dancer,
                ability_id: "tap-target-creature-grant-forestwalk",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(land)],
            },
        )
        .is_err()
    );
    assert!(!game.object(ivy_dancer).expect("source exists").tapped);
    assert_eq!(game.zone_of(land), Some(Zone::Battlefield));
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Ivy Dancer activation preserves invariants");
}
