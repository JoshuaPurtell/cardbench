//! Stack, sacrifice, and blocker-legality contracts for Vindictive Mob.

use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, CreatureSubtype, Game, GameEvent, PlayerId, PolicyAction, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_to_main(game: &mut Game, player: PlayerId) {
    while game.active_player != player || game.step != cardbench_magic_engine::Step::PrecombatMain {
        match game.step {
            cardbench_magic_engine::Step::DeclareAttackers => {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty combat attacker declaration");
                for _ in 0..2 {
                    let priority = game.priority;
                    game.pass_priority(priority)
                        .expect("advance past attackers");
                }
            }
            cardbench_magic_engine::Step::DeclareBlockers => {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty combat blocker declaration");
                for _ in 0..2 {
                    let priority = game.priority;
                    game.pass_priority(priority).expect("advance past blockers");
                }
            }
            cardbench_magic_engine::Step::Draw
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
}

#[test]
fn mob_trigger_sacrifices_another_controlled_creature_before_itself() {
    let mut game = game_with_rav_bindings();
    let mob = game
        .add_card(PlayerId(0), "RAV-VINDICTIVE-MOB", Zone::Hand)
        .expect("Mob enters hand");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature exists");
    let swamps = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Swamp enters before the game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    advance_to_main(&mut game, PlayerId(0));
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("black mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: mob,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Mob casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes spell");
    game.pass_priority(PlayerId(0))
        .expect("controller passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes trigger");
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("Mob controller view")
        .triggered_ability_effect_object_choice
        .expect("Mob controller chooses a sacrifice");
    assert!(choice.candidates.iter().any(|card| card.id == sacrifice));
    game.submit_policy_move(
        PlayerId(0),
        "test.vindictive-mob-sacrifice.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: choice.decision,
            source: mob,
            ability: "etb-sacrifice-controller-creature",
            selected: Some(sacrifice),
        },
    )
    .expect("controller chooses the other creature to sacrifice");

    println!(
        "Vindictive Mob resolution trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(mob), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, player, permanent }
            if *source == mob && *player == PlayerId(0) && *permanent == sacrifice
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == mob && *ability == "etb-sacrifice-controller-creature"
    )));
    game.validate_invariants().expect("Mob sacrifice is valid");
}

#[test]
fn mob_prevents_its_controllers_saprolings_from_blocking() {
    let mut game = game_with_rav_bindings();
    game.put_on_battlefield(PlayerId(1), "RAV-VINDICTIVE-MOB")
        .expect("Mob enters for player one");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker enters for player zero");
    let scatter = game
        .add_card(PlayerId(1), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .expect("Scatter enters player-one hand");
    let forests = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-FOREST")
                .expect("Forest enters before the game")
        })
        .collect::<Vec<_>>();
    for _ in 0..2 {
        game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Library)
            .expect("player-zero library card enters");
    }
    game.add_card(PlayerId(1), "RAV-SWAMP", Zone::Library)
        .expect("player-one library card enters");
    game.begin_game().expect("game starts");
    advance_to_main(&mut game, PlayerId(1));
    for forest in forests {
        game.activate_mana_ability(PlayerId(1), forest, Color::Green)
            .expect("green mana");
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
    .expect("Scatter casts");
    game.pass_priority(PlayerId(1))
        .expect("caster passes scatter");
    game.pass_priority(PlayerId(0))
        .expect("opponent resolves scatter");
    let saproling = game
        .player(PlayerId(1))
        .expect("player one exists")
        .battlefield
        .iter()
        .copied()
        .find(|candidate| {
            game.characteristics(*candidate)
                .is_ok_and(|characteristics| {
                    characteristics
                        .creature_subtypes
                        .contains(&CreatureSubtype::Saproling)
                })
        })
        .expect("Scatter created a Saproling");
    advance_to_main(&mut game, PlayerId(0));
    while game.step != cardbench_magic_engine::Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Watchwolf attacks");
    while game.step != cardbench_magic_engine::Step::DeclareBlockers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to blockers");
    }
    assert!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker,
                blocker: saproling,
            }],
        )
        .is_err()
    );
    game.validate_invariants()
        .expect("failed Saproling block leaves a valid game");
}
