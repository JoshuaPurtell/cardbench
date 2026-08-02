//! Red regression for the public Stasis Cell semantic slice.
//!
//! The green milestone must use the shared Aura attachment lifecycle and a
//! regular target-bearing reattachment activation rather than a card-name
//! branch.

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn stasis_cell_is_full_fidelity_aura() {
    let cell = definition("RAV-STASIS-CELL");
    assert_eq!(cell.mana_cost, ManaCost::with_colors(1, [Color::Blue]));
    assert_eq!(
        cell.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&cell.id),
        "Stasis Cell must be promoted only with its attachment and reattachment behavior"
    );
}

fn stasis_game() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("attachment bindings register");
    game
}

#[test]
#[allow(clippy::too_many_lines)] // The attachment lifecycle trace is intentionally reviewed end-to-end.
fn stasis_cell_reattaches_and_moves_its_restrictions() {
    let mut game = stasis_game();
    let first_target = game
        .put_on_battlefield(PlayerId(1), "RAV-DIMIR-GUILDMAGE")
        .expect("first creature enters");
    let second_target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("second creature enters");
    let cell = game
        .add_card(PlayerId(0), "RAV-STASIS-CELL", Zone::Hand)
        .expect("Cell enters hand");
    let islands = (0..5)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Island enters")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("opponent pass");
    }
    for island in islands.iter().take(2) {
        game.activate_mana_ability(PlayerId(0), *island, Color::Blue)
            .expect("blue mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cell,
            targets: vec![Target::Permanent(first_target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Cell casts");
    game.pass_priority(PlayerId(0)).expect("Cell pass");
    game.pass_priority(PlayerId(1)).expect("Cell resolves");
    assert_eq!(
        game.object(cell).expect("Cell lives").attached_to,
        Some(first_target)
    );
    assert!(
        game.characteristics(first_target)
            .expect("first characteristics")
            .keywords
            .contains(&Keyword::CannotAttackOrBlock)
    );

    game.pass_priority(PlayerId(0))
        .expect("pass to first target controller");
    let before_rejected_activation = game.event_log.clone();
    assert!(
        game.activate_ability(
            PlayerId(1),
            AbilityActivation {
                source: first_target,
                ability_id: "target-player-discard",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(PlayerId(0))],
            },
        )
        .is_err()
    );
    assert_eq!(
        game.event_log, before_rejected_activation,
        "rejection is atomic"
    );
    game.pass_priority(PlayerId(1))
        .expect("pass back to Cell controller");

    for island in islands.iter().skip(2) {
        game.activate_mana_ability(PlayerId(0), *island, Color::Blue)
            .expect("generic activation mana");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: cell,
            ability_id: "reattach-to-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(second_target)],
        },
    )
    .expect("reattachment activates");
    game.pass_priority(PlayerId(0)).expect("activation pass");
    game.pass_priority(PlayerId(1))
        .expect("activation resolves");

    assert_eq!(
        game.object(cell).expect("Cell lives").attached_to,
        Some(second_target)
    );
    assert!(
        !game
            .characteristics(first_target)
            .expect("first characteristics")
            .keywords
            .contains(&Keyword::CannotAttackOrBlock)
    );
    let second = game
        .characteristics(second_target)
        .expect("second characteristics");
    assert!(second.keywords.contains(&Keyword::CannotAttackOrBlock));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == cell && *ability == "reattach-to-target-creature"
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::AuraAttached { aura, .. } if *aura == cell))
            .count(),
        2,
        "the initial attachment and reattachment both have receipts"
    );
    println!("Stasis Cell trace: {:?}", game.event_log);
    game.validate_invariants().expect("Aura state is valid");
}
