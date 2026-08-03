//! Public behavior contract for Sisters of Stone Death's linked combat slice.

use cardbench_magic_engine::{
    AbilityActivation, CombatBlock, DecisionSelection, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_legendary_permanent_bindings, rav_mana_ability_bindings,
};

const FORCE_BLOCK: &str = "green-target-creature-must-block-source";
const EXILE_COMBATANT: &str = "black-green-exile-creature-blocking-or-blocked-by-source";
const RETURN_LINKED: &str = "two-black-return-source-linked-exiled-creature";

fn sisters_game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture constructs");
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .expect("legend binding registers");
    game
}

fn resolve_top(game: &mut Game) {
    let holder = game.priority;
    game.pass_priority(holder).expect("holder passes");
    let responder = game.priority;
    game.pass_priority(responder).expect("responder passes");
}

fn add_mana_sources(game: &mut Game, player: PlayerId) -> Vec<cardbench_magic_engine::ObjectId> {
    [
        "RAV-FOREST",
        "RAV-FOREST",
        "RAV-FOREST",
        "RAV-SWAMP",
        "RAV-SWAMP",
        "RAV-SWAMP",
        "RAV-SWAMP",
    ]
    .into_iter()
    .map(|definition| {
        game.put_on_battlefield(player, definition)
            .expect("mana source setup")
    })
    .collect()
}

fn activate_mana(
    game: &mut Game,
    player: PlayerId,
    lands: &[cardbench_magic_engine::ObjectId],
    indices: &[usize],
) {
    for index in indices {
        let color = if *index < 3 {
            cardbench_magic_engine::Color::Green
        } else {
            cardbench_magic_engine::Color::Black
        };
        game.activate_mana_ability(player, lands[*index], color)
            .expect("typed mana source activates");
    }
}

fn activate(
    game: &mut Game,
    player: PlayerId,
    source: cardbench_magic_engine::ObjectId,
    ability_id: &'static str,
    targets: Vec<Target>,
) {
    game.activate_ability(
        player,
        AbilityActivation {
            source,
            ability_id,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets,
        },
    )
    .unwrap_or_else(|error| panic!("{ability_id} activation enters stack: {error}"));
}

#[test]
fn sisters_force_block_exile_and_selected_return_share_one_source_incarnation() {
    let controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = sisters_game();
    let sisters = game
        .put_on_battlefield(controller, "RAV-SISTERS-OF-STONE-DEATH")
        .expect("Sisters setup");
    let blocker = game
        .put_on_battlefield(defender, "RAV-WATCHWOLF")
        .expect("opposing blocker setup");
    game.set_entered_turn_for_setup(sisters, 0)
        .expect("Sisters has been controlled since an earlier turn");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker has been controlled since an earlier turn");
    let lands = add_mana_sources(&mut game, controller);
    game.begin_game().expect("fixture starts");

    // Untap through precombat main to attacker declaration.
    for _ in 0..4 {
        resolve_top(&mut game);
    }
    game.declare_attackers(controller, &[sisters])
        .expect("Sisters attacks");

    activate_mana(&mut game, controller, &lands, &[0]);
    activate(
        &mut game,
        controller,
        sisters,
        FORCE_BLOCK,
        vec![Target::Permanent(blocker)],
    );
    resolve_top(&mut game);

    // Move through the post-attackers priority window and prove the target
    // cannot decline a legal source-relative block.
    resolve_top(&mut game);
    assert!(
        game.declare_blockers(defender, &[])
            .expect_err("forced blocker cannot decline")
            .to_string()
            .contains("required to block")
    );
    game.declare_blockers(
        defender,
        &[CombatBlock {
            attacker: sisters,
            blocker,
        }],
    )
    .expect("the required block is legal");

    activate_mana(&mut game, controller, &lands, &[1, 3]);
    activate(
        &mut game,
        controller,
        sisters,
        EXILE_COMBATANT,
        vec![Target::Permanent(blocker)],
    );
    resolve_top(&mut game);
    assert_eq!(game.zone_of(blocker), Some(Zone::Exile));

    // The final ability is target-free but cannot select arbitrarily: the
    // public no-priority decision exposes only this exact exile member.
    activate_mana(&mut game, controller, &lands, &[4, 5, 6]);
    activate(&mut game, controller, sisters, RETURN_LINKED, vec![]);
    resolve_top(&mut game);
    let choice = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("linked exile return opens a public object choice");
    assert_eq!(choice.min_selections, 1);
    assert_eq!(choice.max_selections, 1);
    assert_eq!(choice.candidates.len(), 1);
    assert_eq!(choice.candidates[0].id, blocker);
    game.submit_decision(
        controller,
        choice.id,
        DecisionSelection::Objects(vec![blocker]),
    )
    .expect("controller selects only source-linked exile member");

    assert_eq!(game.zone_of(blocker), Some(Zone::Battlefield));
    assert_eq!(game.controller_of(blocker), Ok(controller));
    let opened = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::DecisionOpened { decision, .. } if *decision == choice.id))
        .expect("choice opening receipt");
    let moved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blocker))
        .expect("returned creature battlefield receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == sisters && *ability == RETURN_LINKED))
        .expect("terminal return-ability receipt");
    assert!(opened < moved && moved < resolved);
    game.validate_invariants()
        .expect("source-linked combat lifecycle preserves invariants");
    eprintln!(
        "Sisters of Stone Death trace={:?}",
        game.canonical_event_log()
    );
}

#[test]
fn sisters_return_with_no_linked_creature_is_a_legal_no_op() {
    let controller = PlayerId(0);
    let mut game = sisters_game();
    let sisters = game
        .put_on_battlefield(controller, "RAV-SISTERS-OF-STONE-DEATH")
        .expect("Sisters setup");
    let lands = add_mana_sources(&mut game, controller);
    game.begin_game().expect("fixture starts");

    // Reach a normal priority window.  The source has never exiled a card,
    // so the target-free return ability must resolve without opening a
    // fabricated choice or stranding its stack item.
    for _ in 0..4 {
        resolve_top(&mut game);
    }
    game.declare_attackers(controller, &[])
        .expect("empty attack declaration opens priority");
    activate_mana(&mut game, controller, &lands, &[3, 4, 5]);
    activate(&mut game, controller, sisters, RETURN_LINKED, vec![]);
    resolve_top(&mut game);

    assert!(
        game.stack.is_empty(),
        "no-op ability leaves no stack residue"
    );
    assert!(
        game.view_for_player(controller)
            .expect("controller view")
            .pending_decision
            .is_none(),
        "no candidate opens no choice"
    );
    assert_eq!(game.zone_of(sisters), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sisters && *ability == RETURN_LINKED
    )));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::DecisionOpened { .. }))
    );
    game.validate_invariants()
        .expect("empty linked-exile return preserves invariants");
    eprintln!(
        "Sisters empty-return trace={:?}",
        game.canonical_event_log()
    );
}
