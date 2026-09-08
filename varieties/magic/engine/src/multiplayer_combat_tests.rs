//! Engine unit tests, not scored benchmark worlds or format authority.
use super::*;

fn combat() -> (Game, [ObjectId; 4]) {
    let bear = CardDefinition {
        id: "TEST-BEAR", name: "Combat unit-test creature", set_code: "TST",
        mana_cost: ManaCost::new(2), colors: BTreeSet::new(), mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]), is_basic_land: false,
        supported_rules: &["unit-test-vanilla"], power: Some(2), toughness: Some(2), keywords: vec![], effects: vec![],
    };
    let mut game = Game::new([bear], 4).unwrap();
    let a = game.put_on_battlefield(PlayerId(0), "TEST-BEAR").unwrap();
    let b = game.put_on_battlefield(PlayerId(0), "TEST-BEAR").unwrap();
    let x = game.put_on_battlefield(PlayerId(1), "TEST-BEAR").unwrap();
    let y = game.put_on_battlefield(PlayerId(2), "TEST-BEAR").unwrap();
    game.turn = 2;
    game.step = Step::DeclareAttackers;
    game.combat = Some(CombatState::default());
    game.refresh_public_state_integrity();
    game.declare_attackers_against(PlayerId(0), &[
        (a, DefenderChoice::Player(PlayerId(2))), (b, DefenderChoice::Player(PlayerId(1)))
    ]).unwrap();
    game.step = Step::DeclareBlockers;
    game.refresh_public_state_integrity();
    (game, [a, b, x, y])
}

#[test]
fn apnap_blocking_authorities_cannot_block_for_each_other() {
    let (mut game, [a, b, x, y]) = combat();
    assert_eq!(game.view_for_player(PlayerId(0)).unwrap().decision_player, PlayerId(1));
    assert!(game.declare_blockers(PlayerId(2), &[]).is_err());
    assert!(game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: a, blocker: x }]).is_err());
    assert!(game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: a, blocker: y }]).is_err());
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: b, blocker: x }]).unwrap();
    assert!(!game.combat.as_ref().unwrap().blockers_declared);
    assert_eq!(game.submitted_defender_blocks().len(), 1);
    assert!(game.pass_priority(PlayerId(0)).is_err());
    assert_eq!(game.view_for_player(PlayerId(0)).unwrap().decision_player, PlayerId(2));
    game.declare_blockers(PlayerId(2), &[CombatBlock { attacker: a, blocker: y }]).unwrap();
    assert!(game.combat.as_ref().unwrap().blockers_declared);
    assert!(game.submitted_defender_blocks().is_empty());
    game.validate_invariants().unwrap();
}

#[test]
fn unblocked_damage_is_assigned_to_each_declared_defender() {
    let (mut game, [a, b, _, _]) = combat();
    assert_eq!(game.attacking_defender(a), Some(DefenderChoice::Player(PlayerId(2))));
    assert_eq!(game.attacking_defender(b), Some(DefenderChoice::Player(PlayerId(1))));
    game.declare_blockers(PlayerId(1), &[]).unwrap();
    game.declare_blockers(PlayerId(2), &[]).unwrap();
    game.atomic_transition(|game| game.resolve_combat_damage(false)).unwrap();
    assert_eq!(game.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 18, 18, 20]);
    game.validate_invariants().unwrap();
}

#[test]
fn one_defender_blocking_does_not_shield_another_defender() {
    let (mut game, [_, b, x, _]) = combat();
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: b, blocker: x }]).unwrap();
    game.declare_blockers(PlayerId(2), &[]).unwrap();
    game.atomic_transition(|game| game.resolve_combat_damage(false)).unwrap();
    assert_eq!(game.players[1].life, 20);
    assert_eq!(game.players[2].life, 18);
    assert_eq!(game.zone_of(b), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(x), Some(Zone::Graveyard));
}

#[test]
fn departed_defender_is_not_retargeted_or_asked_for_blocks() {
    let (mut game, [a, b, _, _]) = combat();
    game.atomic_transition(|game| game.lose_player(PlayerId(1), "test-concession")).unwrap();
    assert_eq!(game.attacking_defender(b), None);
    assert_eq!(game.attacking_defender(a), Some(DefenderChoice::Player(PlayerId(2))));
    assert_eq!(game.view_for_player(PlayerId(0)).unwrap().decision_player, PlayerId(2));
    game.declare_blockers(PlayerId(2), &[]).unwrap();
    game.atomic_transition(|game| game.resolve_combat_damage(false)).unwrap();
    assert_eq!(game.players[2].life, 18);
    game.validate_invariants().unwrap();
}

#[test]
fn teammate_is_public_but_never_projected_or_targeted_as_an_opponent() {
    let (mut game, _) = combat();
    game.configure_teams(&[vec![PlayerId(0), PlayerId(3)], vec![PlayerId(1), PlayerId(2)]], 30).unwrap();
    let ally = game.put_on_battlefield(PlayerId(3), "TEST-BEAR").unwrap();
    let view = game.view_for_player(PlayerId(0)).unwrap();
    assert_eq!(view.teammates, vec![PlayerId(3)]);
    assert!(view.teammate_battlefield.iter().any(|card| card.id == ally));
    assert!(!view.opponent_battlefield.iter().any(|card| card.id == ally));
    assert_eq!(view.opponent_life.iter().map(|(seat, _)| *seat).collect::<Vec<_>>(), vec![PlayerId(1), PlayerId(2)]);
    assert!(!game.target_matches_for_controller(PlayerId(0), Target::Player(PlayerId(3)), TargetRequirement::Opponent));
    assert!(!game.target_matches_for_controller(PlayerId(0), Target::Permanent(ally), TargetRequirement::OpponentCreature));
}

fn team_combat() -> (Game, [ObjectId; 4]) {
    let (mut game, cards) = combat();
    game.configure_teams(&[vec![PlayerId(0), PlayerId(3)], vec![PlayerId(1), PlayerId(2)]], 30).unwrap();
    game.combat = Some(CombatState::default());
    game.step = Step::DeclareAttackers;
    for card in &cards[..2] { game.objects.get_mut(card).unwrap().tapped = false; }
    game.refresh_public_state_integrity();
    game.declare_attackers_against(PlayerId(0), &[
        (cards[0], DefenderChoice::Player(PlayerId(1))),
        (cards[1], DefenderChoice::Player(PlayerId(2))),
    ]).unwrap();
    game.step = Step::DeclareBlockers;
    game.refresh_public_state_integrity();
    (game, cards)
}

#[test]
fn team_can_cross_block_individually_named_defenders_in_one_declaration() {
    let (mut game, [a, b, x, y]) = team_combat();
    game.declare_blockers(PlayerId(1), &[
        CombatBlock { attacker: a, blocker: y },
        CombatBlock { attacker: b, blocker: x },
    ]).unwrap();
    assert!(game.combat.as_ref().unwrap().blockers_declared);
    game.atomic_transition(|game| game.resolve_combat_damage(false)).unwrap();
    assert_eq!(game.players[1].life, 30);
    assert_eq!(game.players[2].life, 30);
    game.validate_invariants().unwrap();
}

#[test]
fn team_landwalk_uses_attacked_players_lands_not_blocker_controllers_lands() {
    let (mut game, [a, b, x, y]) = team_combat();
    // This is a rules unit fixture, never a scored benchmark setup.
    game.catalog.get_mut("TEST-BEAR").unwrap().keywords.push(Keyword::Landwalk(BasicLandType::Forest));
    let forest = CardDefinition {
        id: "TEST-FOREST", name: "Forest", set_code: "TST",
        mana_cost: ManaCost::new(0), colors: BTreeSet::new(), mana_colors: BTreeSet::from([Color::Green]),
        card_types: BTreeSet::from([CardType::Land]), is_basic_land: true,
        supported_rules: &["basic-land"], power: None, toughness: None, keywords: vec![], effects: vec![],
    };
    game.catalog.insert(forest.id, forest);
    game.basic_land_types.insert("TEST-FOREST", BasicLandType::Forest);
    game.put_on_battlefield(PlayerId(1), "TEST-FOREST").unwrap();
    // A attacks player 1 (Forest): even player 2's creature cannot block A.
    assert!(game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: a, blocker: y }]).is_err());
    // B attacks player 2 (no Forest): player 1's Forest does not prevent X blocking B.
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker: b, blocker: x }]).unwrap();
    game.validate_invariants().unwrap();
}

#[test]
fn shared_cleanup_collects_both_private_hands_before_discarding_either() {
    let (fixture, _) = combat();
    let mut game = Game::new(fixture.catalog.values().cloned(), 4).unwrap();
    game.configure_teams(&[vec![PlayerId(0), PlayerId(2)], vec![PlayerId(1), PlayerId(3)]], 30).unwrap();
    for seat in [0, 2] {
        game.load_deck_into_library(PlayerId(seat), &DeckList {
            mainboard: vec![crate::DeckEntry { card: "TEST-BEAR".into(), count: 8 }], sideboard: vec![],
        }).unwrap();
        game.draw_opening_hand(PlayerId(seat), 8).unwrap();
    }
    game.step = Step::Cleanup;
    game.refresh_public_state_integrity();
    game.atomic_transition(|game| game.open_cleanup_discard_decision()).unwrap();
    let first = game.pending_decision.clone().unwrap();
    assert_eq!(first.player, PlayerId(0));
    let first_card = game.players[0].hand[0];
    game.submit_decision(PlayerId(0), first.id, DecisionSelection::Objects(vec![first_card])).unwrap();
    assert_eq!(game.zone_of(first_card), Some(Zone::Hand));
    let second = game.pending_decision.clone().unwrap();
    assert_eq!(second.player, PlayerId(2));
    let second_card = game.players[2].hand[0];
    assert!(game.submit_decision(PlayerId(0), second.id, DecisionSelection::Objects(vec![second_card])).is_err());
    game.submit_decision(PlayerId(2), second.id, DecisionSelection::Objects(vec![second_card])).unwrap();
    assert_eq!(game.zone_of(first_card), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(second_card), Some(Zone::Graveyard));
    assert_eq!(game.players[0].hand.len(), 7);
    assert_eq!(game.players[2].hand.len(), 7);
    assert!(game.pending_cleanup_discards.is_empty());
    game.validate_invariants().unwrap();
}
