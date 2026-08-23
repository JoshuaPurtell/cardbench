//! Red discovery contract for Pariah's Shield's persistent Equipment damage
//! replacement.  This must remain source-attached rather than becoming a
//! card-name branch or a finite temporary shield.

use cardbench_magic_engine::{
    AbilityActivation, AttachmentKind, Color, ContinuousChange, Game, GameEvent, ManaCost,
    PlayerId, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_attachment_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn pariahs_shield_requires_persistent_equipped_damage_redirection() {
    let shield = definition("RAV-PARIAHS-SHIELD");
    assert_eq!(shield.mana_cost, ManaCost::new(5));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&shield.id),
        "Pariah's Shield must only be full fidelity with its persistent redirection"
    );
    assert!(
        shield
            .supported_rules
            .contains(&"equipment-all-damage-to-equipped-creature-to-controller")
    );

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == shield.id)
        .expect("Pariah's Shield attachment binding exists");
    assert_eq!(attachment.kind, AttachmentKind::Equipment);
    assert_eq!(attachment.target, TargetRequirement::ControlledCreature);
    assert_eq!(
        attachment.changes,
        vec![ContinuousChange::RedirectDamageToAttachmentController],
        "the replacement remains live only while this exact Equipment is attached"
    );

    let equip = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == shield.id && binding.ability.id == "equip-damage-redirection"
        })
        .expect("Pariah's Shield has its ordinary equip activation");
    assert_eq!(equip.ability.mana_cost, ManaCost::new(3));
    assert!(equip.ability.sorcery_speed);
    assert_eq!(
        equip.ability.targets,
        vec![TargetRequirement::ControlledCreature]
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn equipped_creature_damage_is_redirected_to_live_equipment_controller() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        cardbench_magic_rav::rav_mana_ability_bindings(),
        cardbench_magic_rav::rav_basic_land_type_bindings(),
        cardbench_magic_rav::rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("attachment bindings register before the game starts");
    let shield = game
        .put_on_battlefield(PlayerId(0), "RAV-PARIAHS-SHIELD")
        .expect("Shield starts on battlefield");
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature starts on battlefield");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("opponent holds Char");
    game.begin_game().expect("fixture starts game");
    pass_pair(&mut game);
    pass_pair(&mut game);

    game.add_mana_from_action(PlayerId(0), Color::Colorless, 3)
        .expect("controller can pay the ordinary equip cost");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shield,
            ability_id: "equip-damage-redirection",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(creature)],
        },
    )
    .expect("Shield equips the controlled creature at sorcery speed");
    pass_pair(&mut game);

    assert_eq!(
        game.object(shield).expect("Shield exists").attached_to,
        Some(creature)
    );
    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.add_mana_from_action(PlayerId(1), Color::Colorless, 2)
        .expect("opponent produces generic mana");
    game.add_mana_from_action(PlayerId(1), Color::Red, 2)
        .expect("opponent produces red mana");
    game.cast_spell(
        PlayerId(1),
        cardbench_magic_engine::CastRequest {
            card: char,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char targets the equipped creature");
    pass_pair(&mut game);

    println!("Pariah's Shield trace: {:#?}", game.canonical_event_log());
    assert_eq!(
        game.object(creature).expect("creature exists").damage,
        0,
        "damage must not be marked on the equipped creature"
    );
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 16);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == shield && *ability == "equip-damage-redirection"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageRedirected { source, from, to, amount }
            if *source == char
                && *from == creature
                && *to == Target::Player(PlayerId(0))
                && *amount == 4
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount }
            if *source == char && *player == PlayerId(0) && *amount == 4
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == char && *permanent == creature
    )));
    game.validate_invariants()
        .expect("attached redirection preserves state-machine invariants");
}
