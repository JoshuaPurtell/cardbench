//! Private transition tests complement public M5 combat tests.
use super::*;

#[test]
fn counter_to_hand_replay_requires_its_exact_replacement_receipt() {
    let (mut game, card) = fixture();
    let source = ObjectId(999);
    game.event_log = vec![
        GameEvent::CounteredSpellHandReplacement { card, source },
        GameEvent::SpellCountered { card, source },
        GameEvent::CardMoved { card, to: Zone::Hand },
    ];
    game.validate_stack_terminal_event_order().unwrap();
    let valid = game.event_log.clone();
    game.event_log.remove(0);
    assert!(game.validate_stack_terminal_event_order().is_err());
    game.event_log = valid.clone();
    game.event_log[0] = GameEvent::CounteredSpellHandReplacement { card, source: ObjectId(1000) };
    assert!(game.validate_stack_terminal_event_order().is_err());
    game.event_log = valid;
    game.event_log[2] = GameEvent::CardMoved { card, to: Zone::Command };
    assert!(game.validate_stack_terminal_event_order().is_err(), "command return needs its owner-choice receipt");
}

#[test]
fn a_countered_command_zone_cast_offers_a_return_and_keeps_its_tax() {
    let (mut game, commander) = fixture();
    let source = game.put_on_battlefield(PlayerId(1), "EDH-TEST").unwrap();
    game.move_to_zone(commander, Zone::Command).unwrap();
    game.begin_game().unwrap();
    game.step = Step::PrecombatMain;
    game.refresh_public_state_integrity();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: commander,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .unwrap();
    game.atomic_transition(|game| {
        game.counter_target_spell(source, commander)?;
        game.check_state_based_actions_impl()
    })
    .unwrap();
    assert_eq!(game.zone_of(commander), Some(Zone::Graveyard));
    game.check_state_based_actions().unwrap();
    answer(&mut game, vec![commander]);
    assert_eq!(game.zone_of(commander), Some(Zone::Command));
    assert_eq!(game.commander_tax(commander), 2);
    game.validate_invariants().unwrap();
}

fn fixture() -> (Game, ObjectId) {
    let definition = CardDefinition {
        id: "EDH-TEST",
        name: "EDH test commander",
        set_code: "TST",
        mana_cost: crate::ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["fixture"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    };
    let mut game = Game::new([definition], 4).unwrap();
    game.configure_commander_format(40, 21).unwrap();
    let card = game.put_on_battlefield(PlayerId(0), "EDH-TEST").unwrap();
    game.designate_commander(PlayerId(0), card).unwrap();
    (game, card)
}

fn answer(game: &mut Game, selected: Vec<ObjectId>) {
    let decision = game.pending_decision.clone().unwrap();
    game.submit_decision(
        decision.player,
        decision.id,
        DecisionSelection::Objects(selected),
    )
    .unwrap();
}

#[test]
fn repeated_trigger_flush_preserves_pending_apnap_order_choice() {
    let (mut game, card) = fixture();
    let opponent = game.put_on_battlefield(PlayerId(1), "EDH-TEST").unwrap();
    let event = |source, controller, id| PendingTriggeredAbilityEvent {
        source, controller, source_incarnation: game.objects[&source].incarnation,
        source_colors: BTreeSet::new(),
        ability: crate::TriggeredAbility { id, condition: TriggerCondition::Dies,
            mana_cost: crate::ManaCost::new(0), optional: false, targets: vec![], effects: vec![] },
        payload: TriggerEventPayload::None,
    };
    let events = vec![event(card, PlayerId(0), "first"), event(card, PlayerId(0), "second"),
        event(opponent, PlayerId(1), "third"), event(opponent, PlayerId(1), "fourth")];
    game.pending_trigger_events = events;
    game.flush_pending_trigger_events().unwrap();
    let first = game.pending_decision.clone().unwrap();
    game.flush_pending_damage_triggers();
    game.advance_pending_trigger_placements().unwrap();
    assert_eq!(game.pending_decision.as_ref().unwrap().id, first.id);
    let order = Game::decision_trigger_candidates(&first);
    game.resolve_triggered_ability_order_decision(&first, PlayerId(0), order).unwrap();
    let second = game.pending_decision.clone().unwrap();
    assert_eq!(second.player, PlayerId(1));
    assert_ne!(second.id, first.id);
    let order = Game::decision_trigger_candidates(&second);
    game.resolve_triggered_ability_order_decision(&second, PlayerId(1), order).unwrap();
    assert!(game.pending_decision.is_none());
    assert_eq!(game.stack.iter().map(|item| item.controller).collect::<Vec<_>>(),
        vec![PlayerId(0), PlayerId(0), PlayerId(1), PlayerId(1)]);
}

#[test]
fn combat_mill_receipts_survive_later_departure_but_reject_damage_after_loss() {
    let (mut game, commander) = fixture();
    game.register_damage_replacement_effect_bindings([crate::DamageReplacementEffectBinding {
        source_definition: "EDH-TEST",
        effect: DamageReplacementEffect::ReplaceCombatDamageToPlayerWithMillAndCounters,
    }]).unwrap();
    game.begin_game().unwrap();
    game.atomic_transition(|game| {
        game.deal_combat_damage_to_player(commander, PlayerId(1), 2)?;
        game.lose_player(PlayerId(1), "unit-test departure")
    }).unwrap();
    game.validate_invariants().unwrap();
    let receipt = game.event_log.iter().find(|event| matches!(
        event, GameEvent::CombatDamageReplacedWithMillAndCounters { .. }
    )).unwrap().clone();
    // Deliberate replay corruption: a second otherwise-identical receipt now
    // follows PlayerLost. Historical validation must still reject this.
    game.event_log.push(receipt);
    assert!(game.validate_combat_damage_mill_counter_replacement_events().unwrap_err()
        .to_string().contains("invalid provenance"));
}

#[test]
fn command_zone_is_public_but_is_not_a_hand_and_exposes_tax() {
    let (mut game, commander) = fixture();
    game.move_to_zone(commander, Zone::Command).unwrap();
    for seat in [PlayerId(0), PlayerId(1)] {
        let view = game.view_for_player(seat).unwrap();
        assert!(view.hand.is_empty());
        assert_eq!(view.command_zone.len(), 1);
        assert_eq!(view.command_zone[0].id, commander);
        assert_eq!(view.commander_taxes.get(&commander), Some(&0));
    }
}

#[test]
fn owner_can_accept_or_decline_graveyard_and_exile_arrivals_from_every_zone() {
    for source in [Zone::Battlefield, Zone::Hand, Zone::Library, Zone::Command] {
        for destination in [Zone::Graveyard, Zone::Exile] {
            for accept in [false, true] {
                let (mut game, card) = fixture();
                game.move_to_zone(card, source).unwrap();
                game.move_to_zone(card, destination).unwrap();
                assert_eq!(game.zone_of(card), Some(destination));
                assert!(
                    game.pending_decision.is_none(),
                    "no choice inside a resolving instruction"
                );
                game.check_state_based_actions().unwrap();
                assert_eq!(
                    game.pending_decision.as_ref().unwrap().kind,
                    DecisionKind::CommanderReturn
                );
                answer(&mut game, if accept { vec![card] } else { vec![] });
                assert_eq!(
                    game.zone_of(card),
                    Some(if accept { Zone::Command } else { destination })
                );
                game.check_state_based_actions().unwrap();
                assert!(
                    game.pending_decision.is_none(),
                    "declined arrival must not be offered again"
                );
            }
        }
    }
}

#[test]
fn blink_before_sba_does_not_offer_a_stale_exile_return() {
    let (mut game, card) = fixture();
    game.move_to_zone(card, Zone::Exile).unwrap();
    game.move_to_zone(card, Zone::Battlefield).unwrap();
    game.check_state_based_actions().unwrap();
    assert!(game.pending_decision.is_none());
    assert_eq!(game.zone_of(card), Some(Zone::Battlefield));
}

#[test]
fn a_later_arrival_gets_a_new_choice_and_rejects_the_old_answer() {
    let (mut game, card) = fixture();
    game.move_to_zone(card, Zone::Graveyard).unwrap();
    game.check_state_based_actions().unwrap();
    let old = game.pending_decision.as_ref().unwrap().id;
    answer(&mut game, vec![]);
    game.move_to_zone(card, Zone::Battlefield).unwrap();
    game.move_to_zone(card, Zone::Graveyard).unwrap();
    game.check_state_based_actions().unwrap();
    assert_ne!(game.pending_decision.as_ref().unwrap().id, old);
    assert!(
        game.submit_decision(PlayerId(0), old, DecisionSelection::Objects(vec![card]))
            .is_err()
    );
    answer(&mut game, vec![card]);
}

#[test]
fn another_player_cannot_answer_the_owners_choice() {
    let (mut game, card) = fixture();
    game.move_to_zone(card, Zone::Exile).unwrap();
    game.check_state_based_actions().unwrap();
    let id = game.pending_decision.as_ref().unwrap().id;
    assert!(
        game.submit_decision(PlayerId(1), id, DecisionSelection::Objects(vec![card]))
            .is_err()
    );
    assert_eq!(game.zone_of(card), Some(Zone::Exile));
    answer(&mut game, vec![card]);
}

#[test]
fn commander_death_preserves_its_dies_trigger_after_owner_returns_it() {
    let (mut game, card) = fixture();
    game.triggered_abilities.insert(
        "EDH-TEST",
        BTreeMap::from([(
            "dies",
            crate::TriggeredAbility {
                id: "dies",
                condition: TriggerCondition::Dies,
                mana_cost: crate::ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        )]),
    );
    game.objects.get_mut(&card).unwrap().damage = 2;
    game.refresh_public_state_integrity();
    game.check_state_based_actions().unwrap();
    assert_eq!(game.zone_of(card), Some(Zone::Graveyard));
    assert!(
        game.stack.is_empty(),
        "return choice precedes trigger placement"
    );
    answer(&mut game, vec![card]);
    assert_eq!(game.zone_of(card), Some(Zone::Command));
    assert!(
        game.stack
            .iter()
            .any(|item| item.card == card && item.ability_id == Some("dies"))
    );
    game.validate_invariants().unwrap();
}

#[test]
fn commander_return_precedes_simultaneous_dies_trigger_ordering() {
    let (mut game, commander) = fixture();
    let other = game.put_on_battlefield(PlayerId(0), "EDH-TEST").unwrap();
    game.triggered_abilities.insert("EDH-TEST", BTreeMap::from([("dies", crate::TriggeredAbility {
        id: "dies", condition: TriggerCondition::Dies, mana_cost: crate::ManaCost::new(0),
        optional: false, targets: vec![], effects: vec![],
    })]));
    for card in [commander, other] { game.objects.get_mut(&card).unwrap().damage = 2; }
    game.refresh_public_state_integrity();
    game.check_state_based_actions().unwrap();
    let first = game.pending_decision.as_ref().unwrap().id;
    game.flush_pending_dies_triggers().unwrap();
    assert_eq!(game.pending_decision.as_ref().unwrap().id, first);
    assert!(game.pending_trigger_placements.is_empty());
    game.validate_invariants().unwrap();
    answer(&mut game, vec![commander]);
    let decision = game.pending_decision.clone().unwrap();
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    let order = Game::decision_trigger_candidates(&decision);
    game.submit_decision(PlayerId(0), decision.id, DecisionSelection::TriggerOrder(order)).unwrap();
    assert_eq!(game.stack.len(), 2);
    game.validate_invariants().unwrap();
}

#[test]
fn commander_return_choices_use_apnap_owner_order() {
    let (mut game, first) = fixture();
    let second = game.put_on_battlefield(PlayerId(2), "EDH-TEST").unwrap();
    game.designate_commander(PlayerId(2), second).unwrap();
    game.active_player = PlayerId(1);
    game.move_to_zone(first, Zone::Exile).unwrap();
    game.move_to_zone(second, Zone::Exile).unwrap();
    game.check_state_based_actions().unwrap();
    assert_eq!(game.pending_decision.as_ref().unwrap().player, PlayerId(2));
    answer(&mut game, vec![second]);
    assert_eq!(game.pending_decision.as_ref().unwrap().player, PlayerId(0));
    assert_eq!(
        game.zone_of(second),
        Some(Zone::Exile),
        "collect all choices before moving any commander"
    );
    answer(&mut game, vec![first]);
    assert_eq!(game.zone_of(second), Some(Zone::Command));
    assert_eq!(game.zone_of(first), Some(Zone::Command));
}

#[test]
fn strict_loader_seats_a_hundred_card_deck_and_rejects_invalid_setup_atomically() {
    let (mut game, _) = fixture();
    let rules = crate::CommanderCardRules {
        name: "EDH test commander".into(),
        color_identity: BTreeSet::new(),
        basic_land_colors: BTreeSet::new(),
        maximum_copies: Some(1),
        legal: true,
        implemented: true,
        can_be_commander: true,
        co_commanders: BTreeSet::new(),
    };
    let mut land = game.catalog["EDH-TEST"].clone();
    land.id = "EDH-LAND";
    land.name = "Test land";
    land.is_basic_land = true;
    land.card_types = BTreeSet::from([CardType::Land]);
    land.power = None;
    land.toughness = None;
    game.catalog.insert(land.id, land);
    let metadata = crate::CommanderCatalog {
        revision: "test".into(),
        cards: BTreeMap::from([
            ("EDH-TEST".into(), rules.clone()),
            (
                "EDH-LAND".into(),
                crate::CommanderCardRules {
                    name: "Test land".into(),
                    maximum_copies: None,
                    can_be_commander: false,
                    ..rules
                },
            ),
        ]),
    };
    let mut deck = crate::CommanderDeck {
        commanders: vec!["EDH-TEST".into()],
        deck: DeckList {
            mainboard: vec![
                crate::DeckEntry {
                    card: "EDH-TEST".into(),
                    count: 1,
                },
                crate::DeckEntry {
                    card: "EDH-LAND".into(),
                    count: 98,
                },
            ],
            sideboard: vec![],
        },
    };
    game.refresh_public_state_integrity();
    let before = game.canonical_event_log();
    assert!(
        game.load_commander_deck(PlayerId(1), &deck, &metadata)
            .is_err()
    );
    assert_eq!(before, game.canonical_event_log());
    assert!(game.players[1].library.is_empty());
    deck.deck.mainboard[1].count = 99;
    game.load_commander_deck(PlayerId(1), &deck, &metadata)
        .unwrap();
    assert_eq!(game.players[1].library.len(), 99);
    assert_eq!(game.players[1].command.len(), 1);
    assert_eq!(game.players[1].life, 40);
    game.validate_invariants().unwrap();
}
