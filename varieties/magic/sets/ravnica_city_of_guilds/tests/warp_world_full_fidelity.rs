//! Event-log contract for Warp World's owner/library exchange.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn warp_world_resumes_life_and_aura_choices_then_enters_enchantments_second() {
    use cardbench_magic_engine::{DecisionKind, DecisionSelection};
    for pay in [false, true] {
    let player = PlayerId(0);
    let mut game = cardbench_magic_rav::new_rav_game(2).unwrap();
    let creature = game.put_on_battlefield(player, "RAV-WATCHWOLF").unwrap();
    let shock = game.put_on_battlefield(player, "RAV-SACRED-FOUNDRY").unwrap();
    let aura = game.add_card(player, "RAV-MOLDERVINE-CLOAK", Zone::Hand).unwrap();
    game.enter_attachment_without_cast(aura, creature).unwrap();
    let mountains = (0..8).map(|_| game.put_on_battlefield(player, "RAV-MOUNTAIN").unwrap()).collect::<Vec<_>>();
    let warp = game.add_card(player, "RAV-WARP-WORLD", Zone::Hand).unwrap();
    advance_to_precombat_main(&mut game);
    for card in mountains { game.activate_mana_ability(player, card, Color::Red).unwrap(); }
    game.clear_event_log();
    game.cast_spell(player, CastRequest { card: warp, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }).unwrap();
    resolve_top(&mut game);
    let life = game.view_for_player(player).unwrap().pending_decision.unwrap();
    assert_eq!(life.kind, DecisionKind::WarpWorldEntry);
    assert_eq!(life.source.as_ref().map(|card| card.id), Some(shock));
    assert_eq!(life.candidates[0].id, shock);
    let before = game.player(player).unwrap().life;
    game.submit_decision(player, life.id, DecisionSelection::Objects(if pay { vec![shock] } else { vec![] })).unwrap();
    let attachment = game.view_for_player(player).unwrap().pending_decision.unwrap();
    assert_eq!(attachment.kind, DecisionKind::WarpWorldEntry);
    assert_eq!(attachment.source.as_ref().map(|card| card.id), Some(aura));
    assert!(attachment.candidates.iter().any(|card| card.id == creature));
    assert_eq!(game.zone_of(aura), Some(Zone::Library));
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.player(player).unwrap().life, before - if pay { 2 } else { 0 });
    assert_eq!(game.object(shock).unwrap().tapped, !pay);
    assert!(game.pass_priority(player).is_err(), "no priority during an entry choice");
    assert!(game.submit_decision(player, life.id, DecisionSelection::Objects(vec![])).is_err());
    game.submit_decision(player, attachment.id, DecisionSelection::Objects(vec![creature])).unwrap();
    assert_eq!(game.object(aura).unwrap().attached_to, Some(creature));
    assert_eq!(game.zone_of(warp), Some(Zone::Graveyard));
    assert_eq!(game.player(player).unwrap().life, before - if pay { 2 } else { 0 }, "life not paid again on resume");
    assert_eq!(game.event_log.iter().filter(|event| matches!(event, GameEvent::LibraryShuffled { .. })).count(), 2);
    assert_eq!(game.event_log.iter().filter(|event| matches!(event, GameEvent::CardRevealed { .. })).count(), 11);
    game.validate_invariants().unwrap();
    }
}

#[test]
fn warp_world_counts_tokens_and_orders_unattachable_aura_with_other_revealed_cards() {
    use cardbench_magic_engine::{DecisionKind, DecisionSelection};
    let player = PlayerId(0);
    let mut game = cardbench_magic_rav::new_rav_game(2).unwrap();
    let mountains = (0..8).map(|_| game.put_on_battlefield(player, "RAV-MOUNTAIN").unwrap()).collect::<Vec<_>>();
    let forests = (0..5).map(|_| game.put_on_battlefield(player, "RAV-FOREST").unwrap()).collect::<Vec<_>>();
    let bottom = ["RAV-MOLDERVINE-CLOAK", "RAV-LIGHTNING-HELIX", "RAV-LAST-GASP"].map(|id|
        game.add_card(player, id, Zone::Library).unwrap());
    let scatter = game.add_card(player, "RAV-SCATTER-THE-SEEDS", Zone::Hand).unwrap();
    let warp = game.add_card(player, "RAV-WARP-WORLD", Zone::Hand).unwrap();
    advance_to_precombat_main(&mut game);
    for card in forests { game.activate_mana_ability(player, card, Color::Green).unwrap(); }
    game.cast_spell(player, CastRequest { card: scatter, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }).unwrap();
    resolve_top(&mut game);
    for card in mountains { game.activate_mana_ability(player, card, Color::Red).unwrap(); }
    game.clear_event_log();
    game.cast_spell(player, CastRequest { card: warp, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }).unwrap();
    resolve_top(&mut game);
    let decision = game.view_for_player(player).unwrap().pending_decision.unwrap();
    assert_eq!(decision.kind, DecisionKind::WarpWorldBottom);
    assert_eq!(decision.candidates.len(), 3);
    assert!(game.submit_decision(player, decision.id, DecisionSelection::Objects(vec![bottom[0]; 3])).is_err());
    game.submit_decision(player, decision.id, DecisionSelection::Objects(bottom.to_vec())).unwrap();
    assert_eq!(game.player(player).unwrap().library.iter().rev().copied().collect::<Vec<_>>(), bottom);
    assert_eq!(game.zone_of(bottom[0]), Some(Zone::Library), "unattachable Aura never enters");
    assert!(game.event_log.iter().any(|event| matches!(event,
        GameEvent::WarpWorldBottomOrdered { player: owner, top_to_bottom }
        if *owner == player && top_to_bottom == &bottom)));
    assert_eq!(game.event_log.iter().filter(|event| matches!(event, GameEvent::CardRevealed { .. })).count(), 16);
    assert_eq!(game.zone_of(warp), Some(Zone::Graveyard));
    game.validate_invariants().unwrap();
}

fn rav_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins");
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first stack pass");
    let second = game.priority;
    game.pass_priority(second).expect("second stack pass");
}

#[test]
fn warp_world_records_owner_shuffle_exact_reveal_count_and_simultaneous_permanent_return() {
    let controller = PlayerId(0);
    let mut game = rav_game();
    let warp = game
        .add_card(controller, "RAV-WARP-WORLD", Zone::Hand)
        .expect("Warp World setup");
    let original_creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("original creature setup");
    let mountains = (0..8)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("mana land setup")
        })
        .collect::<Vec<_>>();
    for _ in 0..9 {
        game.add_card(controller, "RAV-FOREST", Zone::Library)
            .expect("permanent-only reveal fixture");
    }

    advance_to_precombat_main(&mut game);
    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("red mana activation");
    }
    game.clear_event_log();
    game.cast_spell(
        controller,
        CastRequest {
            card: warp,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Warp World casts");
    resolve_top(&mut game);

    println!("warp_world_trace={:#?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled { player, cards } if *player == controller && *cards == 18
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardRevealed { player, .. } if *player == controller))
            .count(),
        9,
        "the reveal count is the original owner-permanent count"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::CardMoved {
                    to: Zone::Library,
                    ..
                }
            ))
            .count(),
        9,
        "every original owned permanent crosses to its owner's library"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::CardMoved {
                    to: Zone::Battlefield,
                    ..
                }
            ))
            .count(),
        9,
        "each revealed permanent returns through the shared entry batch"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Library } if *card == original_creature
    )));
    game.validate_invariants()
        .expect("Warp World owner/library exchange preserves invariants");
}
