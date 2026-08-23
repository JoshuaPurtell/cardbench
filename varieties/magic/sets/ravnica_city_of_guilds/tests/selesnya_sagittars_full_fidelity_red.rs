//! Red regression for Selesnya Sagittars' omitted combat damage activation.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Step,
    Target, TargetRequirement,
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

fn advance_to_declare_attackers(game: &mut Game) {
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player).expect("priority advances");
    }
}

#[test]
fn selesnya_sagittars_requires_its_activated_damage_ability_for_full_fidelity() {
    let sagittars = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-SAGITTARS")
        .expect("Selesnya Sagittars definition exists");

    assert_eq!(
        sagittars.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::White])
    );
    assert_eq!(
        sagittars.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((sagittars.power, sagittars.toughness), (Some(2), Some(5)));
    assert_eq!(sagittars.keywords, [Keyword::Reach]);
    assert!(
        sagittars
            .supported_rules
            .contains(&"tap-damage-attacking-or-blocking-creature")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sagittars.id));

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == sagittars.id)
        .expect("Selesnya Sagittars activation exists")
        .ability;
    assert_eq!(ability.id, "tap-damage-attacker-or-blocker");
    assert_eq!(ability.mana_cost, ManaCost::new(0));
    assert!(ability.tap_cost);
    assert_eq!(
        ability.targets,
        [TargetRequirement::AttackingOrBlockingCreature]
    );
    assert_eq!(
        ability.effects,
        [Effect::DealDamage {
            target: TargetRequirement::AttackingOrBlockingCreature,
            amount: 1,
        }]
    );
}

#[test]
fn sagittarius_activation_uses_the_stack_and_rechecks_combat_target_legality() {
    let mut game = game_with_rav_bindings();
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker setup");
    let sagittars = game
        .put_on_battlefield(PlayerId(1), "RAV-SELESNYA-SAGITTARS")
        .expect("Sagittars setup");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker is long-controlled");
    game.set_entered_turn_for_setup(sagittars, 0)
        .expect("Sagittars is long-controlled");
    game.begin_game().expect("game starts");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker enters combat");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes");

    game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: sagittars,
            ability_id: "tap-damage-attacker-or-blocker",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(attacker)],
        },
    )
    .expect("Sagittars activation is legal while target attacks");
    game.pass_priority(PlayerId(1))
        .expect("Sagittars controller passes");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller resolves activation");

    assert!(game.object(sagittars).expect("source remains live").tapped);
    assert_eq!(
        game.object(attacker).expect("attacker remains live").damage,
        1
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sagittars && *ability == "tap-damage-attacker-or-blocker"
    )));
    game.validate_invariants()
        .expect("combat-target activation preserves invariants");
}
