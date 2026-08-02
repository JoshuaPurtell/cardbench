//! Red discovery contract for Loxodon Gatekeeper's opposing-permanent entry replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_entry_restriction_bindings,
};

fn gatekeeper_game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("RAV entry-restriction bindings register");
    game
}

fn advance_to_main_phase(game: &mut Game, player: PlayerId) {
    for _ in 0..64 {
        if game.active_player == player && game.step == Step::PrecombatMain {
            return;
        }
        match game.step {
            Step::DeclareAttackers => {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty combat attacker declaration");
                for _ in 0..2 {
                    let priority = game.priority;
                    game.pass_priority(priority)
                        .expect("advance past attackers");
                }
            }
            Step::DeclareBlockers => {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty combat blocker declaration");
                for _ in 0..2 {
                    let priority = game.priority;
                    game.pass_priority(priority).expect("advance past blockers");
                }
            }
            Step::Draw
                if game
                    .view_for_player(game.active_player)
                    .expect("active-player view")
                    .draw_replacement_pending =>
            {
                game.resolve_pending_draw(game.active_player, None)
                    .expect("ordinary draw resolves");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority).expect("advance turn state");
            }
        }
    }
    panic!(
        "did not advance to requested main phase; active={:?} step={:?} priority={:?} events={:?}",
        game.active_player,
        game.step,
        game.priority,
        game.canonical_event_log(),
    );
}

#[test]
fn loxodon_gatekeeper_requires_a_live_opponent_entry_restriction() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LOXODON-GATEKEEPER")
        .expect("Loxodon Gatekeeper definition exists");
    assert_eq!(definition.name, "Loxodon Gatekeeper");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"static-opponents-artifacts-creatures-lands-enter-tapped")
    );
}

#[test]
fn loxodon_gatekeeper_taps_an_opponents_creature_on_ordinary_stack_entry() {
    let mut game = gatekeeper_game();
    let gatekeeper = game
        .put_on_battlefield(PlayerId(0), "RAV-LOXODON-GATEKEEPER")
        .expect("Gatekeeper enters during setup");
    let watchwolf = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Hand)
        .expect("opponent has Watchwolf");
    let signet = game
        .add_card(PlayerId(1), "RAV-BOROS-SIGNET", Zone::Hand)
        .expect("opponent has Boros Signet");
    let mountain = game
        .add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Hand)
        .expect("opponent has a land to play");
    let scatter = game
        .add_card(PlayerId(1), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .expect("opponent has a token spell");
    let forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("opponent Forest enters during setup");
    let plains = game
        .put_on_battlefield(PlayerId(1), "RAV-PLAINS")
        .expect("opponent Plains enters during setup");
    let island = game
        .put_on_battlefield(PlayerId(1), "RAV-ISLAND")
        .expect("opponent Island enters during setup");
    let swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("opponent Swamp enters during setup");
    let token_forests = (0..5)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-FOREST")
                .expect("opponent Forest enters during setup")
        })
        .collect::<Vec<_>>();
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..2 {
            game.add_card(player, "RAV-PLAINS", Zone::Library)
                .expect("each player has enough cards for turn-one draws");
        }
    }
    game.begin_game().expect("game starts");
    advance_to_main_phase(&mut game, PlayerId(1));
    game.activate_mana_ability(PlayerId(1), forest, Color::Green)
        .expect("Forest provides green");
    game.activate_mana_ability(PlayerId(1), plains, Color::White)
        .expect("Plains provides white");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts Watchwolf");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes Watchwolf");
    game.pass_priority(PlayerId(0))
        .expect("Gatekeeper controller allows Watchwolf to resolve");

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Battlefield));
    assert!(game.object(watchwolf).expect("Watchwolf lives").tapped);
    game.activate_mana_ability(PlayerId(1), island, Color::Blue)
        .expect("Island provides generic payment mana");
    game.activate_mana_ability(PlayerId(1), swamp, Color::Black)
        .expect("Swamp provides generic payment mana");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: signet,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts Boros Signet");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes Boros Signet");
    game.pass_priority(PlayerId(0))
        .expect("Gatekeeper controller allows Boros Signet to resolve");
    game.play_land(PlayerId(1), mountain)
        .expect("opponent plays a Mountain");
    assert!(game.object(signet).expect("Signet lives").tapped);
    assert!(game.object(mountain).expect("Mountain lives").tapped);
    for forest in token_forests {
        game.activate_mana_ability(PlayerId(1), forest, Color::Green)
            .expect("Forest pays Scatter the Seeds");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: scatter,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts Scatter the Seeds");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes Scatter the Seeds");
    game.pass_priority(PlayerId(0))
        .expect("Gatekeeper controller allows Scatter the Seeds to resolve");
    let tokens = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::TokenCreated { token, .. } => Some(*token),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 3, "Scatter creates three Saprolings");
    assert!(tokens.iter().all(|token| {
        game.object(*token)
            .expect("created token remains live")
            .tapped
    }));
    let tapped_entries = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::PermanentEnteredTapped {
                permanent, source, ..
            } if *source == gatekeeper => Some(*permanent),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tapped_entries,
        [vec![watchwolf, signet, mountain], tokens.clone()].concat()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentEnteredTapped { permanent, source, .. }
            if *permanent == watchwolf && *source == gatekeeper
    )));
    eprintln!("Loxodon Gatekeeper trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("entry restriction trace preserves invariants");
}
