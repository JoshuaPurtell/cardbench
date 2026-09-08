//! Red discovery contract for Wizened Snitches' public top-library visibility.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, event_digest, rav_basic_land_type_bindings,
    rav_static_library_top_reveal_bindings,
};

#[test]
fn wizened_snitches_has_its_global_top_library_visibility_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WIZENED-SNITCHES")
        .expect("Wizened Snitches definition exists");

    assert_eq!(definition.name, "Wizened Snitches");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Blue])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"global-static-top-library-visibility")
    );
}

fn game_with_visibility_bindings() -> Game {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV fixture builds");
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .expect("RAV static library visibility bindings register");
    game
}

fn visible_tops(game: &Game, player: PlayerId) -> Vec<(PlayerId, u64, Option<&'static str>)> {
    game.view_for_player(player)
        .expect("policy view exists")
        .revealed_library_tops
        .into_iter()
        .map(|top| (top.owner, top.card.id.0, top.card.definition))
        .collect()
}

fn advance_to_precombat_main(game: &mut Game, active_player: PlayerId) {
    while game.active_player != active_player || game.step != Step::PrecombatMain {
        let decision_player = game.next_policy_player();
        if game
            .view_for_player(decision_player)
            .expect("policy view exists")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(decision_player, None)
                .expect("ordinary draw resolves");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("combat view exists")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view exists")
                    .blockers_declared =>
            {
                let defender = game.next_policy_player();
                game.declare_blockers(defender, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let first = game.priority;
                game.pass_priority(first).expect("first priority pass");
                let second = game.priority;
                game.pass_priority(second).expect("second priority pass");
            }
        }
    }
}

#[test]
fn wizened_snitches_reveals_only_current_library_tops_and_revokes_on_departure() {
    let mut game = game_with_visibility_bindings();
    let player = PlayerId(1);
    let opponent = PlayerId(0);
    let bottom = game
        .add_card(opponent, "RAV-WATCHWOLF", Zone::Library)
        .expect("bottom library card exists");
    let player_top = game
        .add_card(opponent, "RAV-CHAR", Zone::Library)
        .expect("top library card exists");
    let opponent_top = game
        .add_card(player, "RAV-LAST-GASP", Zone::Library)
        .expect("opponent top library card exists");
    let snitches = game
        .put_on_battlefield(player, "RAV-WIZENED-SNITCHES")
        .expect("Wizened Snitches enters battlefield");
    let swamp = game
        .put_on_battlefield(player, "RAV-SWAMP")
        .expect("black mana source exists");
    let last_gasp = game
        .add_card(player, "RAV-LAST-GASP", Zone::Hand)
        .expect("removal spell exists");
    let generic_swamp = game.put_on_battlefield(player, "RAV-SWAMP").expect("generic source exists");

    let expected_initial = vec![
        (opponent, player_top.0, Some("RAV-CHAR")),
        (player, opponent_top.0, Some("RAV-LAST-GASP")),
    ];
    assert_eq!(visible_tops(&game, player), expected_initial);
    assert_eq!(visible_tops(&game, opponent), expected_initial);
    assert!(
        visible_tops(&game, player)
            .iter()
            .all(|(_, card, _)| *card != bottom.0),
        "the visibility effect exposes no card below the current library top"
    );

    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game, player);
    let expected_after_draw = vec![(opponent, player_top.0, Some("RAV-CHAR"))];
    assert_eq!(visible_tops(&game, player), expected_after_draw);
    assert_eq!(visible_tops(&game, opponent), expected_after_draw);

    game.activate_mana_ability(player, swamp, Color::Black)
        .expect("Swamp produces Black");
    game.activate_mana_ability(player, generic_swamp, Color::Black).expect("generic mana");
    game.cast_spell(
        player,
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(snitches)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp targets the reveal source");
    game.pass_priority(player).expect("caster passes");
    game.pass_priority(opponent)
        .expect("opponent resolves spell");

    assert_eq!(game.zone_of(snitches), Some(Zone::Graveyard));
    assert!(visible_tops(&game, player).is_empty());
    assert!(visible_tops(&game, opponent).is_empty());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCast { card, .. } if *card == last_gasp
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == snitches
    )));
    eprintln!("Wizened Snitches trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Wizened Snitches preserves invariant state");
}

#[test]
fn wizened_snitches_public_scenario_trace_is_deterministic() {
    let mut game = game_with_visibility_bindings();
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let snitches = game
        .add_card(player, "RAV-WIZENED-SNITCHES", Zone::Hand)
        .expect("Wizened Snitches exists");
    let own_top = game
        .add_card(player, "RAV-WATCHWOLF", Zone::Library)
        .expect("own top exists");
    let opponent_top = game
        .add_card(opponent, "RAV-CHAR", Zone::Library)
        .expect("opponent top exists");
    let islands = (0..4)
        .map(|_| game.put_on_battlefield(player, "RAV-ISLAND"))
        .collect::<Result<Vec<_>, _>>()
        .expect("four Islands exist");

    game.begin_game().expect("game begins");
    for expected in [player, opponent, player, opponent] {
        assert_eq!(game.priority, expected);
        game.pass_priority(expected).expect("priority pass");
    }
    for island in islands {
        game.activate_mana_ability(player, island, Color::Blue)
            .expect("Island produces Blue");
    }
    game.cast_spell(
        player,
        CastRequest {
            card: snitches,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Wizened Snitches casts");
    game.pass_priority(player).expect("caster passes");
    game.pass_priority(opponent)
        .expect("opponent resolves spell");

    assert_eq!(
        visible_tops(&game, player),
        vec![
            (player, own_top.0, Some("RAV-WATCHWOLF")),
            (opponent, opponent_top.0, Some("RAV-CHAR")),
        ]
    );
    let trace = game.canonical_event_log();
    assert_eq!(event_digest(&trace), "fnv1a64:f8f065ddc2b243ec");
    eprintln!(
        "Wizened Snitches public scenario digest={} trace={trace:?}",
        event_digest(&trace)
    );
    game.validate_invariants()
        .expect("public scenario preserves invariant state");
}
