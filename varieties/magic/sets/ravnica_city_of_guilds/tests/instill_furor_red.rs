//! Red discovery contract for Instill Furor's attached-creature end-step rule.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardType, CastPaymentManaAbility, CastRequest, Color, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn instill_game() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV Instill Furor fixture builds");
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

fn advance_one_policy_action(game: &mut Game) {
    let active = game.active_player;
    if game.step == Step::Draw && game.resolve_pending_draw(active, None).is_ok() {
        return;
    }
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(active)
            .expect("attacker view")
            .attackers_declared
    {
        game.declare_attackers(active, &[])
            .expect("empty attacker declaration");
        return;
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("blocker view")
            .blockers_declared
    {
        let defender = game.next_policy_player();
        game.declare_blockers(defender, &[])
            .expect("empty blocker declaration");
        return;
    }
    game.pass_priority(game.priority)
        .expect("priority holder advances turn state");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..64 {
        if game.turn == turn && game.step == step {
            return;
        }
        advance_one_policy_action(game);
        game.validate_invariants()
            .expect("ordinary turn transition preserves invariants");
    }
    panic!(
        "turn machine did not reach turn {turn} step {step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

fn cast_instill_furor(game: &mut Game, creature: ObjectId) -> ObjectId {
    let aura = game
        .add_card(PlayerId(0), "RAV-INSTILL-FUROR", Zone::Hand)
        .expect("Aura starts in controller hand");
    let first_mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("first red land setup");
    let second_mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("second red land setup");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: first_mountain,
                    color: Color::Red,
                }),
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: second_mountain,
                    color: Color::Red,
                }),
            ],
        },
    )
    .expect("Aura cast with explicit red land mana");
    pass_pair(game);
    aura
}

#[test]
fn instill_furor_requires_its_attacked_this_turn_sacrifice_rule() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INSTILL-FUROR")
        .expect("Instill Furor definition exists");

    assert_eq!(definition.name, "Instill Furor");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(1, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-creature-end-step-attack-sacrifice")
    );
}

#[test]
fn instill_furor_sacrifices_the_exact_unattacked_attachment_at_controller_end_step() {
    let mut game = instill_game();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature setup");
    let aura = cast_instill_furor(&mut game, creature);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));

    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature may attack if chosen");
    game.begin_game().expect("game begins");
    advance_to(&mut game, 1, Step::End);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, controller, ability, .. }
            if *source == aura
                && *controller == PlayerId(0)
                && *ability == "attached-creature-controller-end-step-sacrifice-unless-attacked"
    )));
    pass_pair(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(aura), Some(Zone::Graveyard));
    let sacrifice = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SacrificedByEffect { source, player, permanent }
                    if *source == aura && *player == PlayerId(0) && *permanent == creature
            )
        })
        .expect("attached creature sacrifice receipt");
    assert!(matches!(
        game.event_log.get(sacrifice + 1),
        Some(GameEvent::CardMoved { card, to: Zone::Graveyard }) if *card == creature
    ));
    game.validate_invariants()
        .expect("conditional attachment sacrifice preserves invariants");
}

#[test]
fn instill_furor_retains_an_attachment_that_attacked_this_turn() {
    let mut game = instill_game();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature setup");
    let aura = cast_instill_furor(&mut game, creature);
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature setup enters before the current turn");
    game.begin_game().expect("game begins");
    advance_to(&mut game, 1, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[creature])
        .expect("attached creature attacks");
    advance_to(&mut game, 1, Step::End);
    pass_pair(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, permanent, .. }
            if *source == aura && *permanent == creature
    )));
    game.validate_invariants()
        .expect("attacked-this-turn attachment path preserves invariants");
}

#[test]
fn instill_furor_uses_the_attached_creatures_controller_end_step() {
    let mut game = instill_game();
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let aura = cast_instill_furor(&mut game, creature);
    game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("opponent has a draw-step card for the second turn");
    game.begin_game().expect("game begins");

    advance_to(&mut game, 1, Step::End);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, .. } if *source == aura
        )),
        "the Aura controller's end step is not the enchanted creature controller's end step"
    );
    advance_to(&mut game, 2, Step::End);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, controller, ability, .. }
            if *source == aura
                && *controller == PlayerId(1)
                && *ability == "attached-creature-controller-end-step-sacrifice-unless-attacked"
    )));
    pass_pair(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(aura), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("cross-controller Aura trigger preserves invariants");
}
