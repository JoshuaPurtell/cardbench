//! Red discovery contract for Twisted Justice's target-player sacrifice and
//! power-derived draw resolution.
//!
//! The assertions use only CardBench semantic names and event receipts; they
//! intentionally do not reproduce printed card text or artwork.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn twisted_justice_definition_declares_target_player_sacrifice_then_power_draw() {
    let justice = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TWISTED-JUSTICE")
        .expect("Twisted Justice definition exists");

    assert_eq!(justice.name, "Twisted Justice");
    assert_eq!(justice.mana_cost, ManaCost::with_colors(4, [Color::Blue, Color::Black]));
    assert_eq!(justice.card_types, [CardType::Sorcery].into());
    assert!(
        justice
            .supported_rules
            .contains(&"target-player-sacrifice-creature-power-derived-draw")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&justice.id));
}

#[test]
fn twisted_justice_retains_target_players_sacrifice_choice_and_draws_from_sacrificed_power() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let justice = game
        .add_card(PlayerId(0), "RAV-TWISTED-JUSTICE", Zone::Hand)
        .expect("Twisted Justice enters hand");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target player has a creature");
    let drawn = (0..3)
        .map(|_| {
            game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
                .expect("draw card enters library")
        })
        .collect::<Vec<_>>();
    for color in [Color::Blue, Color::Black] {
        game.grant_mana(PlayerId(0), color, 1)
            .expect("colored payment mana");
    }
    game.grant_mana(PlayerId(0), Color::Colorless, 4)
        .expect("generic payment mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: justice,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Twisted Justice casts at target player");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell starts resolving");

    println!("Twisted Justice red trace: {:?}", game.event_log);
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("target player view")
            .pending_decision
            .is_some(),
        "target player chooses a creature"
    );
    assert_eq!(game.stack.len(), 1, "choice retains the resolving spell");
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert!(drawn.iter().all(|card| game.zone_of(*card) == Some(Zone::Library)));
}
