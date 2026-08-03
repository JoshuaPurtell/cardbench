//! Public cast, stack, and entry-counter contract for Chorus of the Conclave.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, CounterKind, CreatureSpellExtraManaPayment, DecisionSelection,
    Game, GameEvent, ManaPaymentSelection, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    rav_static_creature_spell_cost_modifier_bindings,
};

fn game() -> Game {
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture builds");
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .expect("Chorus static binding registers before play");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // The public stack and receipt transcript is intentionally one reviewable contract.
fn chorus_records_optional_creature_payment_and_places_entry_counters_on_resolution() {
    let controller = PlayerId(0);
    let mut game = game();
    let chorus = game
        .put_on_battlefield(controller, "RAV-CHORUS-OF-THE-CONCLAVE")
        .expect("Chorus battlefield setup");
    let watchwolf = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell setup");
    let noncast_watchwolf = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("noncast creature setup");
    game.grant_mana(controller, Color::Green, 2)
        .expect("green payment setup");
    game.grant_mana(controller, Color::White, 2)
        .expect("white payment setup");
    game.clear_event_log();

    game.submit_policy_move(
        controller,
        "rav/chorus-optional-entry-counter-policy",
        PolicyAction::CastWithCreatureSpellAdditionalMana {
            request: CastRequest {
                card: watchwolf,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
            extra_payments: vec![CreatureSpellExtraManaPayment {
                source: chorus,
                colors: vec![Color::Green, Color::White],
            }],
        },
    )
    .expect("optional creature payment casts Watchwolf");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Battlefield));
    assert_eq!(
        game.object(watchwolf)
            .expect("resolved Watchwolf remains live")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&2),
        "one recorded optional mana per counter reaches the entering creature"
    );
    assert!(
        game.object(noncast_watchwolf)
            .expect("direct-entry Watchwolf remains live")
            .counters
            .is_empty(),
        "a non-cast entry cannot inherit a prior optional spell payment"
    );
    let optional_payment = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CreatureSpellExtraManaPaid {
                    player,
                    card,
                    source,
                    amount: 2,
                    ..
                } if *player == controller && *card == watchwolf && *source == chorus
            )
        })
        .expect("optional payment receipt");
    let cast = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SpellCast { player, card }
                    if *player == controller && *card == watchwolf
            )
        })
        .expect("cast receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf))
        .expect("resolution receipt");
    let placed = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CounterPlaced {
                    source,
                    card,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 2,
                } if *source == chorus && *card == watchwolf
            )
        })
        .expect("entry-counter receipt");
    assert!(optional_payment < cast && cast < resolved && resolved < placed);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted { player, policy, .. }
            if *player == controller && policy == "rav/chorus-optional-entry-counter-policy"
    )));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CHORUS-OF-THE-CONCLAVE"));
    game.validate_invariants()
        .expect("Chorus cast and stack lifecycle preserves invariants");
    eprintln!("chorus_of_conclave_trace={:?}", game.canonical_event_log());
}

#[test]
fn chorus_extra_mana_is_rejected_for_a_noncreature_spell_without_mutation() {
    let controller = PlayerId(0);
    let mut game = game();
    let chorus = game
        .put_on_battlefield(controller, "RAV-CHORUS-OF-THE-CONCLAVE")
        .expect("Chorus battlefield setup");
    let char = game
        .add_card(controller, "RAV-CHAR", Zone::Hand)
        .expect("noncreature spell setup");
    game.grant_mana(controller, Color::Green, 1)
        .expect("payment setup");
    game.clear_event_log();

    let error = game
        .cast_creature_spell_with_additional_mana(
            controller,
            CastRequest {
                card: char,
                targets: vec![Target::Permanent(chorus)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            None,
            ManaPaymentSelection::default(),
            vec![CreatureSpellExtraManaPayment {
                source: chorus,
                colors: vec![Color::Green],
            }],
        )
        .expect_err("a noncreature cannot receive Chorus payment");
    assert!(error.to_string().contains("only creature spells"));
    assert_eq!(game.zone_of(char), Some(Zone::Hand));
    assert!(game.event_log.is_empty(), "failed cast is fully atomic");
    game.validate_invariants()
        .expect("rejected optional payment leaves no stale state");
}

#[test]
fn countered_chorus_paid_creature_has_no_stale_entry_counter_provenance() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let chorus = game
        .put_on_battlefield(controller, "RAV-CHORUS-OF-THE-CONCLAVE")
        .expect("Chorus battlefield setup");
    let watchwolf = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell setup");
    let perplex = game
        .add_card(opponent, "RAV-PERPLEX", Zone::Hand)
        .expect("counterspell setup");
    game.grant_mana(controller, Color::Green, 2)
        .expect("green payment setup");
    game.grant_mana(controller, Color::White, 1)
        .expect("white payment setup");
    game.grant_mana(opponent, Color::Blue, 1)
        .expect("counterspell blue payment setup");
    game.grant_mana(opponent, Color::Black, 2)
        .expect("counterspell black payment setup");
    game.clear_event_log();

    game.cast_creature_spell_with_additional_mana(
        controller,
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        None,
        ManaPaymentSelection::default(),
        vec![CreatureSpellExtraManaPayment {
            source: chorus,
            colors: vec![Color::Green],
        }],
    )
    .expect("optional creature payment casts Watchwolf");
    game.pass_priority(controller)
        .expect("caster yields priority to counterspell");
    game.cast_spell(
        opponent,
        CastRequest {
            card: perplex,
            targets: vec![Target::Spell(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Perplex targets the paid creature spell");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("counterspell opens an explicit decline-or-discard decision");
    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::CounterUnlessDiscardsHand { discard: false },
    )
    .expect("controller declines the empty-hand discard option");

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source }
            if *card == watchwolf && *source == perplex
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { card, .. } if *card == watchwolf
    )));
    game.validate_invariants()
        .expect("countering consumes pending Chorus entry-counter provenance");
    eprintln!("chorus_countered_trace={:?}", game.canonical_event_log());
}

#[test]
fn chorus_definition_is_a_full_green_white_forestwalk_creature() {
    let chorus = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CHORUS-OF-THE-CONCLAVE")
        .expect("Chorus definition exists");
    assert_eq!(chorus.card_types, [CardType::Creature].into());
    assert_eq!(chorus.colors, [Color::Green, Color::White].into());
    assert_eq!((chorus.power, chorus.toughness), (Some(3), Some(8)));
}
