//! Direct, ability-complete contract for RAV Greater Mossdog.
//!
//! This stores only `CardBench` semantic facts and the public event receipt, not
//! source rules prose or source JSON.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Game, Keyword, ManaCost, PlayerId, Step, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn greater_mossdog_has_complete_dredge_and_characteristics_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GREATER-MOSSDOG")
        .expect("Greater Mossdog definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.supported_rules,
        ["full-rules-fidelity", "dredge", "base-characteristics"]
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Green])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Green]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(3)));
    assert_eq!(definition.keywords, [Keyword::Dredge(3)]);
    assert!(definition.effects.is_empty());
}

#[test]
fn greater_mossdog_public_trace_is_the_complete_dredge_then_cast_receipt() {
    let mossdog = run_all_scenarios()
        .expect("public RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_greater_mossdog_dredge_and_cast")
        .expect("Greater Mossdog public scenario exists");

    assert_eq!(mossdog.digest, "fnv1a64:d55b48973736e61d");
    assert_eq!(
        mossdog.event_log,
        [
            "CardMoved { card: ObjectId(4), to: Graveyard }",
            "ObjectIncarnationAdvanced { object: ObjectId(4), incarnation: 2 }",
            "CardMoved { card: ObjectId(3), to: Graveyard }",
            "ObjectIncarnationAdvanced { object: ObjectId(3), incarnation: 2 }",
            "CardMoved { card: ObjectId(2), to: Graveyard }",
            "ObjectIncarnationAdvanced { object: ObjectId(2), incarnation: 2 }",
            "CardMoved { card: ObjectId(1), to: Hand }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 2 }",
            "Dredged { player: PlayerId(0), card: ObjectId(1), count: 3 }",
            "SpellCast { player: PlayerId(0), card: ObjectId(1) }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 3 }",
            "PriorityPassed { player: PlayerId(0) }",
            "PriorityPassed { player: PlayerId(1) }",
            "SpellResolved { card: ObjectId(1) }",
            "CardMoved { card: ObjectId(1), to: Battlefield }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 4 }",
        ]
    );
}

#[test]
fn greater_mossdog_dredge_uses_the_live_draw_step_decision_boundary() {
    // Three seats mean player 0 takes its first-turn draw, so this reaches a
    // real draw-replacement decision without needing to advance through two
    // full two-player turns.
    let player = PlayerId(0);
    let mut game = Game::new(card_definitions(), 3).expect("three-player RAV game builds");
    let mossdog = game
        .add_card(player, "RAV-GREATER-MOSSDOG", Zone::Graveyard)
        .expect("Mossdog begins in its owner's graveyard");
    let first = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("first library card exists");
    let second = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("second library card exists");
    let third = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("third library card exists");

    game.begin_game()
        .expect("game starts and automatically advances past untap");
    for seat in 0..3 {
        game.pass_priority(PlayerId(seat))
            .expect("each live priority pass is legal");
    }
    assert_eq!(game.step, Step::Draw);
    assert_eq!(game.active_player, player);
    assert!(
        game.view_for_player(player)
            .expect("controller view exists")
            .draw_replacement_pending,
        "the turn machine must expose a replacement decision before Dredge"
    );

    game.clear_event_log();
    game.resolve_pending_draw(player, Some(mossdog))
        .expect("Dredge replaces only this live draw");
    assert_eq!(
        game.canonical_event_log(),
        [
            format!("CardMoved {{ card: {third:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {third:?}, incarnation: 2 }}"),
            format!("CardMoved {{ card: {second:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {second:?}, incarnation: 2 }}"),
            format!("CardMoved {{ card: {first:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {first:?}, incarnation: 2 }}"),
            format!("CardMoved {{ card: {mossdog:?}, to: Hand }}"),
            format!("ObjectIncarnationAdvanced {{ object: {mossdog:?}, incarnation: 2 }}"),
            format!("Dredged {{ player: {player:?}, card: {mossdog:?}, count: 3 }}"),
        ]
    );
    assert_eq!(game.zone_of(mossdog), Some(Zone::Hand));
    assert!(
        !game
            .view_for_player(player)
            .expect("controller view remains available")
            .draw_replacement_pending,
        "the replacement marker must be consumed by the Dredge decision"
    );
    game.validate_invariants()
        .expect("live Greater Mossdog Dredge preserves engine invariants");
}
