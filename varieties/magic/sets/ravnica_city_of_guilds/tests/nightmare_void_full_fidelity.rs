use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn nightmare_void_targets_one_player_and_discards_that_players_oldest_hand_card() {
    let caster = PlayerId(0);
    let target = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let nightmare_void = game
        .add_card(caster, "RAV-NIGHTMARE-VOID", Zone::Hand)
        .expect("Nightmare Void setup");
    let discarded = game
        .add_card(target, "RAV-WATCHWOLF", Zone::Hand)
        .expect("target hand setup");
    game.grant_mana(caster, Color::Black, 4)
        .expect("fixture mana");
    game.clear_event_log();

    game.cast_spell(
        caster,
        CastRequest {
            card: nightmare_void,
            targets: vec![Target::Player(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted discard spell casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(target)
        .expect("target passes and resolves");

    println!("Nightmare Void trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CardDiscarded { player, card } if *player == target && *card == discarded)
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellResolved { card } if *card == nightmare_void)
    }));
    game.validate_invariants()
        .expect("targeted discard trace stays valid");
}

#[test]
fn nightmare_void_definition_preserves_its_dredge_and_target_slot() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NIGHTMARE-VOID")
        .expect("Nightmare Void definition");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black])
    );
    assert_eq!(definition.keywords, vec![Keyword::Dredge(2)]);
    assert_eq!(
        definition.effects,
        vec![Effect::DiscardTargetPlayer { count: 1 }]
    );
    assert_eq!(
        definition
            .effects
            .iter()
            .filter_map(Effect::target_requirement)
            .collect::<Vec<_>>(),
        vec![TargetRequirement::Player]
    );
}
