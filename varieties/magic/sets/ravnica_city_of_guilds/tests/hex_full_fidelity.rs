use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hex_rejects_a_repeated_target_atomically_then_destroys_six_distinct_creatures() {
    let caster = PlayerId(0);
    let target_controller = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let hex = game
        .add_card(caster, "RAV-HEX", Zone::Hand)
        .expect("Hex setup");
    let targets = (0..6)
        .map(|_| {
            game.put_on_battlefield(target_controller, "RAV-WATCHWOLF")
                .expect("distinct creature setup")
        })
        .collect::<Vec<_>>();
    game.grant_mana(caster, Color::Black, 6)
        .expect("fixture mana");
    game.clear_event_log();

    let duplicate = game.cast_spell(
        caster,
        CastRequest {
            card: hex,
            targets: vec![Target::Permanent(targets[0]); 6],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    println!("Hex repeated-target result: {duplicate:?}");
    assert!(duplicate.is_err());
    assert_eq!(game.zone_of(hex), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());

    game.cast_spell(
        caster,
        CastRequest {
            card: hex,
            targets: targets.iter().copied().map(Target::Permanent).collect(),
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("six distinct targets cast");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(target_controller)
        .expect("opponent passes and Hex resolves");

    println!("Hex six-target trace: {:?}", game.event_log);
    assert!(
        targets
            .iter()
            .all(|target| game.zone_of(*target) == Some(Zone::Graveyard))
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardDestroyed { .. }))
            .count(),
        6
    );
    game.validate_invariants()
        .expect("six-target resolution remains valid");
}

#[test]
fn hex_is_positive_manifest_with_six_distinct_target_effects() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HEX")
        .expect("Hex definition");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Black, Color::Black])
    );
    assert_eq!(
        definition.effects,
        vec![Effect::DestroyDistinctTargetCreature; 6]
    );
    assert_eq!(
        definition
            .effects
            .iter()
            .filter_map(Effect::target_requirement)
            .collect::<Vec<_>>(),
        vec![TargetRequirement::DistinctCreature; 6]
    );
}
