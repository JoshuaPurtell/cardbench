//! Public contract for the narrow RAV Signet compatibility slice.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    card_definitions, executable_definition_id_for_collector, rav_mana_ability_bindings,
};

const SIGNETS: [(u16, &str, &str); 4] = [
    (255, "RAV-BOROS-SIGNET", "Boros Signet"),
    (260, "RAV-DIMIR-SIGNET", "Dimir Signet"),
    (262, "RAV-GOLGARI-SIGNET", "Golgari Signet"),
    (270, "RAV-SELESNYA-SIGNET", "Selesnya Signet"),
];

#[test]
fn rav_signets_are_two_mana_artifacts_that_cast_to_the_battlefield() {
    let definitions = card_definitions();
    for (collector_number, definition_id, name) in SIGNETS {
        assert_eq!(
            executable_definition_id_for_collector(collector_number),
            Ok(definition_id),
            "the public catalog maps {name} at its verified collector number"
        );
        let definition = definitions
            .iter()
            .find(|definition| definition.id == definition_id)
            .expect("every named Signet has a public compatibility definition");
        assert_eq!(definition.name, name);
        assert_eq!(definition.mana_cost, ManaCost::new(2));
        assert_eq!(definition.card_types.len(), 1);
        assert!(definition.card_types.contains(&CardType::Artifact));
        assert_eq!(
            definition.supported_rules,
            ["artifact-casting", "paid-fixed-two-color-mana-ability"]
        );

        let mut game =
            Game::new_with_mana_abilities(definitions.clone(), 2, rav_mana_ability_bindings())
                .expect("RAV Signet bindings initialize");
        let signet = game
            .add_card(PlayerId(0), definition_id, Zone::Hand)
            .expect("fixture puts Signet in hand");
        game.grant_mana(PlayerId(0), Color::Blue, 2)
            .expect("fixture grants the generic casting cost");
        game.clear_event_log();

        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: signet,
                targets: vec![],
                convoke: vec![],
            },
        )
        .expect("two generic mana casts each Signet artifact");
        game.pass_priority(PlayerId(0))
            .expect("caster passes to resolve the artifact");
        game.pass_priority(PlayerId(1))
            .expect("both players pass and the artifact resolves");

        assert_eq!(game.zone_of(signet), Some(Zone::Battlefield));
        assert!(
            game.event_log.iter().any(
                |event| matches!(event, GameEvent::SpellResolved { card, .. } if *card == signet)
            ),
            "{name} has a canonical artifact-resolution event"
        );
        game.validate_invariants()
            .expect("Signet artifact casting preserves engine invariants");
    }
}
