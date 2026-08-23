//! Red discovery contract for Sunforger's Equipment and library-cast ability.
//!
//! This intentionally names the observable card contract before the engine
//! grows the search-and-cast continuation.  It must fail while Sunforger is
//! catalog-only, rather than silently treating the printed ability as a
//! generic Equipment chassis.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, AttachmentKind, CardType, Color, DecisionKind,
    DecisionSelection, Game, GameEvent, GeneralizedAbilityActivation, ManaCost, PlayerId, Step,
    Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
};

#[test]
fn sunforger_requires_equipment_and_red_white_library_cast_contracts() {
    let sunforger = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUNFORGER")
        .expect("Sunforger definition exists");

    assert_eq!(sunforger.name, "Sunforger");
    assert_eq!(sunforger.mana_cost, ManaCost::new(3));
    assert_eq!(sunforger.colors, BTreeSet::<Color>::new());
    assert_eq!(sunforger.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sunforger.id),
        "Sunforger must not be labeled full fidelity until both its equip and library cast are live"
    );
    assert!(sunforger
        .supported_rules
        .contains(&"equip-three-and-detach-search-red-or-white-instant-mana-value-at-most-four-cast-without-mana"));

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == sunforger.id)
        .expect("Sunforger has an Equipment attachment binding");
    assert_eq!(attachment.kind, AttachmentKind::Equipment);
    assert_eq!(attachment.target, TargetRequirement::ControlledCreature);

    let equip = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == sunforger.id
                && binding.ability.id == "equip-plus-four-plus-zero"
        })
        .expect("Sunforger has its sorcery-speed equip ability");
    assert_eq!(equip.ability.mana_cost, ManaCost::new(3));
    assert!(equip.ability.sorcery_speed);
    assert_eq!(
        equip.ability.targets,
        vec![TargetRequirement::ControlledCreature]
    );

    let cast = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == sunforger.id
                && binding.ability.id == "red-white-detach-search-and-cast-instant"
        })
        .expect("Sunforger has its red-white detach search-and-cast ability");
    assert_eq!(
        cast.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Red, Color::White])
    );
    assert!(!cast.ability.sorcery_speed);
    assert!(cast.ability.targets.is_empty());
}

fn game() -> Game {
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
        .expect("Equipment bindings register before the game starts");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("generalized activation costs register before the game starts");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The event-order assertions deliberately cover the complete atomic continuation.
fn sunforger_detaches_then_privately_selects_and_casts_its_instant() {
    let mut game = game();
    let sunforger = game
        .put_on_battlefield(PlayerId(0), "RAV-SUNFORGER")
        .expect("Sunforger begins on the battlefield");
    let bearer = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled bearer begins on the battlefield");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Library)
        .expect("qualifying instant begins in the private library");
    game.begin_game().expect("fixture begins");
    advance_to_precombat_main(&mut game);

    game.add_mana_from_action(PlayerId(0), Color::Colorless, 3)
        .expect("controller can pay the equip cost");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: sunforger,
            ability_id: "equip-plus-four-plus-zero",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(bearer)],
        },
    )
    .expect("Sunforger equips at sorcery speed");
    pass_pair(&mut game);
    assert_eq!(
        game.object(sunforger)
            .expect("Equipment exists")
            .attached_to,
        Some(bearer)
    );
    assert_eq!(
        game.characteristics(bearer).expect("bearer exists").power,
        Some(7)
    );

    game.add_mana_from_action(PlayerId(0), Color::Red, 1)
        .expect("red activation mana is available");
    game.add_mana_from_action(PlayerId(0), Color::White, 1)
        .expect("white activation mana is available");
    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: sunforger,
                ability_id: "red-white-detach-search-and-cast-instant",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment::default(),
            mana_payment_selection: None,
        },
    )
    .expect("attached Sunforger can pay red-white and detach atomically");
    assert_eq!(
        game.object(sunforger)
            .expect("Equipment exists")
            .attached_to,
        None,
        "detachment is an activation cost before opponents can respond"
    );
    assert_eq!(
        game.characteristics(bearer).expect("bearer exists").power,
        Some(3)
    );

    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller has a view")
        .pending_decision
        .expect("Sunforger resolution opens a private library choice");
    assert_eq!(decision.kind, DecisionKind::LibrarySearchAndCast);
    assert!(
        decision
            .candidates
            .iter()
            .any(|candidate| candidate.id == helix)
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent has a view")
            .pending_decision
            .is_none(),
        "the opponent cannot see private library candidates"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::LibrarySearchAndCast {
            selected: Some(helix),
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("controller selects Helix and its ordinary player target");
    assert_eq!(
        game.zone_of(helix),
        None,
        "selected instant is now on the stack"
    );
    pass_pair(&mut game);

    println!("Sunforger trace: {:#?}", game.canonical_event_log());
    assert_eq!(
        game.player(PlayerId(0)).expect("controller exists").life,
        23
    );
    assert_eq!(game.player(PlayerId(1)).expect("opponent exists").life, 17);
    assert_eq!(game.zone_of(helix), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttachmentDetached { attachment, kind: AttachmentKind::Equipment, reason: "paid as activated ability cost", .. }
            if *attachment == sunforger
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved { source, found: Some(found), destination: cardbench_magic_engine::LibrarySearchDestination::CastWithoutPayingManaCost, .. }
            if *source == sunforger && *found == helix
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCastFromPermission { card, from: Zone::Library, .. } if *card == helix
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sunforger && *ability == "red-white-detach-search-and-cast-instant"
    )));
    game.validate_invariants()
        .expect("Sunforger's detach/search/cast state machine remains valid");
}

#[test]
fn sunforger_search_activation_rejects_an_unattached_equipment_without_mutation() {
    let mut game = game();
    let sunforger = game
        .put_on_battlefield(PlayerId(0), "RAV-SUNFORGER")
        .expect("Sunforger begins unattached");
    game.begin_game().expect("fixture begins");
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Red, 1)
        .expect("red mana is available");
    game.add_mana_from_action(PlayerId(0), Color::White, 1)
        .expect("white mana is available");
    let events_before = game.event_log.len();
    let result = game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: sunforger,
                ability_id: "red-white-detach-search-and-cast-instant",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment::default(),
            mana_payment_selection: None,
        },
    );
    assert!(
        result.is_err(),
        "an unattached Sunforger cannot pay its detach cost"
    );
    assert_eq!(game.event_log.len(), events_before);
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .mana_pool
            .amount(Color::Red),
        1
    );
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .mana_pool
            .amount(Color::White),
        1
    );
    game.validate_invariants()
        .expect("rejected detach payment leaves the game unchanged");
}
