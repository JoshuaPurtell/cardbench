//! Public behavior contract for Tunnel Vision's named-card library traversal.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

fn cast_tunnel_vision(
    game: &mut Game,
    caster: PlayerId,
    target: PlayerId,
) -> cardbench_magic_engine::ObjectId {
    let spell = game
        .add_card(caster, "RAV-TUNNEL-VISION", Zone::Hand)
        .expect("Tunnel Vision fixture card");
    game.grant_mana(caster, Color::Blue, 6)
        .expect("six blue mana fixture");
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Player(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Tunnel Vision casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(target)
        .expect("target passes into choice");
    spell
}

#[test]
fn tunnel_vision_reveals_to_named_card_mills_preceding_cards_then_shuffles() {
    let caster = PlayerId(0);
    let target = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    // Libraries are bottom-to-top vectors.  Tunnel Vision reveals Char first,
    // then Watchwolf, leaving the named card on top before shuffle.
    let bottom = game
        .add_card(target, "RAV-FOREST", Zone::Library)
        .expect("bottom fixture");
    let named = game
        .add_card(target, "RAV-WATCHWOLF", Zone::Library)
        .expect("named fixture");
    let preceding = game
        .add_card(target, "RAV-CHAR", Zone::Library)
        .expect("revealed-before-named fixture");
    let spell = cast_tunnel_vision(&mut game, caster, target);

    let choice = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("name decision opens before hidden traversal");
    assert_eq!(choice.kind, DecisionKind::NamedCardTargetLibraryTraversal);
    assert!(choice.card_name_candidates.contains(&"Watchwolf"));
    assert!(choice.card_name_candidates.contains(&"Tunnel Vision"));
    assert!(
        game.view_for_player(target)
            .expect("target public decision view")
            .pending_decision
            .expect("public name choice is visible")
            .card_name_candidates
            .contains(&"Watchwolf")
    );
    game.submit_decision(caster, choice.id, DecisionSelection::CardName("Watchwolf"))
        .expect("caster names Watchwolf");

    assert_eq!(game.zone_of(preceding), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(named), Some(Zone::Library));
    assert_eq!(game.zone_of(bottom), Some(Zone::Library));
    let revealed = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardRevealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(revealed, vec![preceding, named]);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardNameChosen { decision, player, name }
            if *decision == choice.id && *player == caster && *name == "Watchwolf"
    )));
    let shuffle = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::LibraryShuffled { player, .. } if *player == target),
        )
        .expect("required target-library shuffle receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("spell terminal receipt");
    assert!(shuffle < resolved);
    game.validate_invariants()
        .expect("named-card traversal preserves engine invariants");
    eprintln!("Tunnel Vision found trace={:?}", game.canonical_event_log());
}

#[test]
fn tunnel_vision_missing_name_reveals_and_shuffles_without_milling() {
    let caster = PlayerId(0);
    let target = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let bottom = game
        .add_card(target, "RAV-FOREST", Zone::Library)
        .expect("bottom fixture");
    let top = game
        .add_card(target, "RAV-CHAR", Zone::Library)
        .expect("top fixture");
    let _spell = cast_tunnel_vision(&mut game, caster, target);
    let choice = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("name decision opens");
    let events_before_rejected_name = game.event_log.clone();
    assert!(
        game.submit_decision(
            caster,
            choice.id,
            DecisionSelection::CardName("not-a-represented-card"),
        )
        .expect_err("foreign name is rejected atomically")
        .to_string()
        .contains("unavailable name")
    );
    assert_eq!(game.event_log, events_before_rejected_name);
    assert_eq!(
        game.view_for_player(caster)
            .expect("caster still owns original decision")
            .pending_decision
            .expect("rejected answer leaves decision live")
            .id,
        choice.id
    );
    game.submit_decision(caster, choice.id, DecisionSelection::CardName("Watchwolf"))
        .expect("caster names a represented absent card");

    assert_eq!(game.zone_of(bottom), Some(Zone::Library));
    assert_eq!(game.zone_of(top), Some(Zone::Library));
    assert!(game.players[target.0].graveyard.is_empty());
    let revealed = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardRevealed { card, .. } => Some(*card),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(revealed, vec![top, bottom]);
    game.validate_invariants()
        .expect("missing-name traversal preserves engine invariants");
    eprintln!(
        "Tunnel Vision absent trace={:?}",
        game.canonical_event_log()
    );
}
