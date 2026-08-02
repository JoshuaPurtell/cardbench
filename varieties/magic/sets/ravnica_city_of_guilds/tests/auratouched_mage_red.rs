//! Red discovery contract for Auratouched Mage's bounded Aura-search ETB.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

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
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

fn cast_and_resolve_mage(game: &mut Game, mage: cardbench_magic_engine::ObjectId) {
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: mage,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Mage casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Mage resolves");
    game.pass_priority(PlayerId(0))
        .expect("Mage trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("Mage trigger resolves");
}

fn add_and_activate_mage_mana(game: &mut Game) {
    let plains = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters before game start")
        })
        .collect::<Vec<_>>();
    advance_to_precombat_main(game);
    for plains in plains {
        game.activate_mana_ability(PlayerId(0), plains, Color::White)
            .expect("Plains produces White mana");
    }
}

#[test]
fn auratouched_mage_has_a_bounded_deterministic_aura_search_trigger() {
    let mage = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AURATOUCHED-MAGE")
        .expect("Auratouched Mage definition exists");
    assert_eq!(mage.name, "Auratouched Mage");
    assert_eq!(mage.mana_cost, ManaCost::with_colors(5, [Color::White]));
    assert_eq!(mage.colors, BTreeSet::from([Color::White]));
    assert_eq!(mage.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((mage.power, mage.toughness), (Some(3), Some(3)));
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&mage.id),
        "the deterministic compatibility selector must not claim full fidelity"
    );
    assert!(mage.supported_rules.contains(&"etb-aura-search-and-attach"));
    assert!(
        mage.supported_rules
            .contains(&"deterministic-first-compatible-aura-selection")
    );
    assert!(rav_triggered_ability_bindings().into_iter().any(|binding| {
        binding.card_definition == mage.id && binding.ability.id == "etb-search-and-attach-aura"
    }));
}

#[test]
fn mage_fetches_the_first_compatible_aura_attaches_it_and_replays_its_etb() {
    let mut game = rav_game();
    let fists = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Library)
        .expect("first compatible Aura enters library");
    let later_aura = game
        .add_card(PlayerId(0), "RAV-FAITHS-FETTERS", Zone::Library)
        .expect("later compatible Aura enters library");
    let mage = game
        .add_card(PlayerId(0), "RAV-AURATOUCHED-MAGE", Zone::Hand)
        .expect("Mage enters hand");
    add_and_activate_mage_mana(&mut game);

    cast_and_resolve_mage(&mut game, mage);

    assert_eq!(game.zone_of(mage), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(fists), Some(Zone::Battlefield));
    assert_eq!(
        game.object(fists).expect("Fists exists").attached_to,
        Some(mage)
    );
    assert_eq!(game.zone_of(later_aura), Some(Zone::Library));
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::LibrarySearchResolved {
                source,
                found: Some(found),
                ..
            },
            GameEvent::LibraryShuffled { player, .. }
        ] if *source == mage && *found == fists && *player == PlayerId(0)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AuraAttached { aura, target } if *aura == fists && *target == mage
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == fists && *ability == "etb-two-saprolings"
    )));

    game.pass_priority(PlayerId(0))
        .expect("Fists trigger passes");
    game.pass_priority(PlayerId(1))
        .expect("Fists trigger resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("Mage Aura search preserves attachment and stack invariants");
}

#[test]
fn mage_shuffles_and_records_a_failed_search_when_no_compatible_aura_exists() {
    let mut game = rav_game();
    let unrelated = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("unrelated creature enters library");
    let mage = game
        .add_card(PlayerId(0), "RAV-AURATOUCHED-MAGE", Zone::Hand)
        .expect("Mage enters hand");
    add_and_activate_mage_mana(&mut game);

    cast_and_resolve_mage(&mut game, mage);

    assert_eq!(game.zone_of(unrelated), Some(Zone::Library));
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::LibrarySearchResolved {
                source,
                found: None,
                ..
            },
            GameEvent::LibraryShuffled { player, .. }
        ] if *source == mage && *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("empty Mage Aura search preserves invariants");
}
