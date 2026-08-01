//! Red discovery contract for Blazing Archon's static attack restriction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
};

fn game_with_static_attack_restrictions() -> Game {
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV static-binding game builds");
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .expect("RAV static attack restrictions register before start");
    game
}

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn blazing_archon_requires_its_complete_static_attack_restriction() {
    let archon = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLAZING-ARCHON")
        .expect("Blazing Archon definition exists");

    assert_eq!(archon.name, "Blazing Archon");
    assert_eq!(
        archon.mana_cost,
        ManaCost::with_colors(6, [Color::White, Color::White, Color::White])
    );
    assert_eq!(archon.colors, BTreeSet::from([Color::White]));
    assert_eq!(archon.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((archon.power, archon.toughness), (Some(5), Some(6)));
    assert_eq!(archon.keywords, [Keyword::Flying]);
    assert_eq!(archon.effects, []);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&archon.id));
    assert!(
        archon
            .supported_rules
            .contains(&"static-opponents-cannot-attack-controller")
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn blazing_archon_rejects_attacks_atomically_then_stops_on_battlefield_departure() {
    let mut game = game_with_static_attack_restrictions();
    let archon = game
        .put_on_battlefield(PlayerId(1), "RAV-BLAZING-ARCHON")
        .expect("Archon setup");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker setup");
    for card in [archon, attacker] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("permanent established before measured turn");
    }
    advance_to_declare_attackers(&mut game);

    let before_rejection = game.event_log.clone();
    let error = game
        .declare_attackers(PlayerId(0), &[attacker])
        .expect_err("Archon protects its controller from attacks");
    assert_eq!(
        error.to_string(),
        "cannot attack a player protected by a static attack restriction"
    );
    assert_eq!(game.event_log, before_rejection);
    assert!(!game.object(attacker).expect("attacker remains live").tapped);
    game.validate_invariants()
        .expect("rejected attack preserves combat invariants");

    // Attacker declaration is a mandatory turn-based action, so use a second
    // game to show the live battlefield restriction stops applying before the
    // declaration window opens; the rejected game above remains unchanged.
    let mut removal_game = game_with_static_attack_restrictions();
    let archon = removal_game
        .put_on_battlefield(PlayerId(1), "RAV-BLAZING-ARCHON")
        .expect("Archon removal setup");
    let attacker = removal_game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker removal setup");
    let first_gasp = removal_game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("first removal setup");
    let second_gasp = removal_game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("second removal setup");
    let swamps = [
        removal_game
            .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
            .expect("first Swamp setup"),
        removal_game
            .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
            .expect("second Swamp setup"),
    ];
    for card in [archon, attacker] {
        removal_game
            .set_entered_turn_for_setup(card, 0)
            .expect("permanent established before measured turn");
    }
    removal_game.begin_game().expect("removal game starts");
    for _ in 0..2 {
        removal_game
            .pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        removal_game
            .pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
    assert_eq!(removal_game.step, Step::PrecombatMain);
    for swamp in swamps {
        removal_game
            .activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Swamp produces removal mana");
    }

    for gasp in [first_gasp, second_gasp] {
        removal_game
            .cast_spell(
                PlayerId(0),
                CastRequest {
                    card: gasp,
                    targets: vec![Target::Permanent(archon)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .expect("Last Gasp targets the Archon");
        removal_game
            .pass_priority(PlayerId(0))
            .expect("caster passes removal");
        removal_game
            .pass_priority(PlayerId(1))
            .expect("removal resolves");
    }
    assert_eq!(removal_game.zone_of(archon), Some(Zone::Graveyard));
    for _ in 0..2 {
        removal_game
            .pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        removal_game
            .pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(removal_game.step, Step::DeclareAttackers);

    removal_game
        .declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker becomes legal as soon as Archon leaves battlefield");
    assert!(
        removal_game
            .object(attacker)
            .expect("attacker remains live")
            .tapped
    );
    assert!(removal_game.event_log.iter().any(
        |event| matches!(event, GameEvent::AttackersDeclared { attackers, .. } if attackers == &[attacker]),
    ));
    removal_game
        .validate_invariants()
        .expect("combat restriction removal preserves invariants");
}
