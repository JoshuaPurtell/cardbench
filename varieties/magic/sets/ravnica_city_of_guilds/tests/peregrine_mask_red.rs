//! Red discovery contract for Peregrine Mask's Equipment lifecycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AttachmentKind, CardType, Color, ContinuousChange, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn peregrine_mask_requires_exact_equipment_keyword_binding() {
    let mask = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PEREGRINE-MASK")
        .expect("Peregrine Mask definition exists");
    assert_eq!(mask.name, "Peregrine Mask");
    assert_eq!(mask.mana_cost, ManaCost::new(1));
    assert_eq!(mask.colors, BTreeSet::<Color>::new());
    assert_eq!(mask.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&mask.id));
    assert!(
        mask.supported_rules
            .contains(&"equipment-defender-flying-first-strike")
    );
    assert!(rav_attachment_bindings().iter().any(|binding| {
        binding.card_definition == mask.id
            && binding.kind == AttachmentKind::Equipment
            && binding.changes
                == vec![
                    ContinuousChange::AddKeyword(Keyword::Defender),
                    ContinuousChange::AddKeyword(Keyword::Flying),
                    ContinuousChange::AddKeyword(Keyword::FirstStrike),
                ]
    }));
}

#[test]
fn peregrine_mask_catalog_mapping_names_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(268),
        Ok("RAV-PEREGRINE-MASK")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-PEREGRINE-MASK")
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn peregrine_mask_equip_creates_all_three_keyword_layers() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("Mask attachment binding registers before game start");
    let mask = game
        .put_on_battlefield(PlayerId(0), "RAV-PEREGRINE-MASK")
        .expect("Mask begins on battlefield");
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature begins on battlefield");
    game.begin_game().expect("fixture starts game");
    pass_pair(&mut game);
    pass_pair(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Colorless, 2)
        .expect("generic equip payment is a legal action");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: mask,
            ability_id: "equip-defender-flying-first-strike",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(creature)],
        },
    )
    .expect("Mask equips controlled creature at sorcery speed");
    pass_pair(&mut game);

    println!("Peregrine Mask trace: {:#?}", game.canonical_event_log());
    let characteristics = game
        .characteristics(creature)
        .expect("equipped creature exists");
    for keyword in [Keyword::Defender, Keyword::Flying, Keyword::FirstStrike] {
        assert!(characteristics.keywords.contains(&keyword));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::EquipmentAttached { equipment, target, .. } if *equipment == mask && *target == creature
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, target, .. }
                    if *source == mask && *target == creature
            ))
            .count(),
        3
    );
    game.validate_invariants()
        .expect("Equipment keyword layers preserve invariant state");
}
