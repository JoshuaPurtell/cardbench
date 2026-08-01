use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn netherborn_phalanx_etb_counts_each_opponents_live_creatures_at_resolution() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger game builds");
    let phalanx = game
        .add_card(caster, "RAV-NETHERBORN-PHALANX", Zone::Hand)
        .expect("Phalanx setup");
    game.put_on_battlefield(opponent, "RAV-WATCHWOLF")
        .expect("first opponent creature");
    game.put_on_battlefield(opponent, "RAV-GLASS-GOLEM")
        .expect("second opponent creature");
    game.grant_mana(caster, Color::Black, 6)
        .expect("fixture mana");
    game.clear_event_log();

    game.cast_spell(
        caster,
        CastRequest {
            card: phalanx,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Phalanx cast");
    game.pass_priority(caster).expect("caster passes spell");
    game.pass_priority(opponent)
        .expect("opponent passes and ETB trigger stacks");
    assert_eq!(game.stack.len(), 1, "ETB trigger waits on the stack");
    game.pass_priority(caster).expect("caster passes trigger");
    game.pass_priority(opponent)
        .expect("opponent passes and trigger resolves");

    println!("Netherborn Phalanx trace: {:?}", game.event_log);
    assert_eq!(game.player(opponent).unwrap().life, 18);
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::LifeLost { source, player, amount } if *source == phalanx && *player == opponent && *amount == 2)
    }));
    game.validate_invariants()
        .expect("dynamic ETB trace remains valid");
}

#[test]
fn netherborn_phalanx_is_positive_manifest_with_transmute() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NETHERBORN-PHALANX")
        .expect("Phalanx definition");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.keywords,
        vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Black, Color::Black],
        ))]
    );
}
