//! Regression: multiple legal Aura-relative combat triggers may observe one packet.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_attackers(game: &mut Game) {
    game.begin_game().expect("game begins");
    for _ in 0..16 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances toward attackers");
    }
    panic!("fixture did not reach declare attackers");
}

#[test]
fn two_attached_token_auras_can_capture_one_combat_packet_without_rollback() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_triggers();
    let creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    let first_aura = game
        .add_card(controller, "RAV-POLLENBRIGHT-WINGS", Zone::Hand)
        .expect("first Aura setup");
    let second_aura = game
        .add_card(controller, "RAV-POLLENBRIGHT-WINGS", Zone::Hand)
        .expect("second Aura setup");
    game.grant_mana(controller, Color::Green, 6)
        .expect("green payment setup");
    game.grant_mana(controller, Color::Blue, 6)
        .expect("blue payment setup");
    for aura in [first_aura, second_aura] {
        game.cast_spell(
            controller,
            CastRequest {
                card: aura,
                targets: vec![Target::Permanent(creature)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Aura casts targeting the same Watchwolf");
        pass_pair(&mut game);
        assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    }
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature is long controlled for combat setup");
    game.clear_event_log();

    advance_to_attackers(&mut game);
    game.declare_attackers(controller, &[creature])
        .expect("doubly enchanted creature attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(1), &[])
        .expect("defender declares no blocks");
    pass_pair(&mut game);

    println!("multi_aura_event_log={:?}", game.canonical_event_log());
    let captures = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::AttachedCombatDamageTokenCountCaptured {
                    aura,
                    creature: captured_creature,
                    player,
                    amount: 3,
                    ..
                } if [first_aura, second_aura].contains(aura)
                    && *captured_creature == creature
                    && *player == PlayerId(1)
            )
        })
        .count();
    assert_eq!(
        captures, 2,
        "each Aura captures the same committed damage packet"
    );
    game.validate_invariants()
        .expect("a consecutive legal Aura capture group preserves invariants");
}
