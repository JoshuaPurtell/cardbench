//! Red discovery probe for Seismic Spike's complete front-face effect.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};
use cardbench_magic_rav::{
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn seismic_spike_requires_land_destruction_and_two_red_mana() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEISMIC-SPIKE")
        .expect("Seismic Spike definition exists");
    assert_eq!(definition.mana_cost.generic, 3);
    assert_eq!(definition.mana_cost.colored, vec![Color::Red]);
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"destroy-target-land"));
    assert!(definition.supported_rules.contains(&"add-two-red-mana"));
}

#[test]
fn seismic_spike_destroys_the_land_then_adds_two_red_mana() {
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
    let spike = game
        .add_card(PlayerId(0), "RAV-SEISMIC-SPIKE", Zone::Hand)
        .expect("Seismic Spike enters hand");
    let land = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("target Mountain enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 4)
        .expect("spell mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spike,
            targets: vec![Target::Permanent(land)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seismic Spike casts at sorcery speed");
    game.pass_priority(PlayerId(0))
        .expect("caster passes to opponent");
    game.pass_priority(PlayerId(1))
        .expect("Seismic Spike resolves");

    assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    assert_eq!(
        game.player(PlayerId(0))
            .expect("caster exists")
            .mana_pool
            .amount(Color::Red),
        2
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == spike && *card == land
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ManaAdded { player, color: Color::Red, amount: 2 }
            if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("Seismic trace is invariant-valid");
}
