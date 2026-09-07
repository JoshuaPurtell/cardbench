//! M5 format fixtures: Two-Headed Giant and mini-Commander.
//!
//! These are the deterministic tests that back flipping
//! `FormatCapability::TwoHeadedGiant` and `FormatCapability::Commander` from
//! `Absent` to `Supported` in the protocol capability manifest. They mirror
//! the Harbor-grade Python gold in `varieties/magic/formats_gold/`:
//!
//! - `test_engine.py::test_two_headed_giant_damage_hits_shared_team_life`
//! - `test_engine.py::test_commander_unblocked_swing_reaches_21`
//!
//! Card names, power, and toughness are the public Ravnica facts the gold
//! uses. Mana is generic and the lists are mini-lists, not 100-card EDH: deck
//! construction is deliberately out of this slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding, CardDefinition, CardType,
    CastPaymentManaAbility, CastRequest, Color, CombatBlock, DefenderChoice, Game, GameEvent,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TeamId, Zone,
};

const FILLER: &str = "M5-FILLER";
const WATCHWOLF: &str = "M5-WATCHWOLF";
const CHORUS: &str = "M5-CHORUS-OF-THE-CONCLAVE";
const RAZIA: &str = "M5-RAZIA-BOROS-ARCHANGEL";
const TOLSIMIR: &str = "M5-TOLSIMIR-WOLFBLOOD";
const SZADEK: &str = "M5-SZADEK-LORD-OF-SECRETS";
const AGRUS: &str = "M5-AGRUS-KOS-WOJEK-VETERAN";
const FOREST: &str = "M5-FOREST";

const POLICY: &str = "test.formats-m5.v1";

fn creature(
    id: &'static str,
    name: &'static str,
    power: i16,
    toughness: i16,
    generic_cost: u8,
) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: "RAV",
        mana_cost: ManaCost::new(generic_cost),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(power),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: FILLER,
            name: FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        creature(WATCHWOLF, "Watchwolf", 3, 3, 2),
        creature(CHORUS, "Chorus of the Conclave", 3, 8, 6),
        creature(RAZIA, "Razia, Boros Archangel", 6, 3, 8),
        creature(TOLSIMIR, "Tolsimir Wolfblood", 3, 4, 5),
        creature(SZADEK, "Szadek, Lord of Secrets", 5, 5, 7),
        creature(AGRUS, "Agrus Kos, Wojek Veteran", 3, 3, 5),
        CardDefinition {
            id: FOREST,
            name: "Forest",
            set_code: "RAV",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["basic-land-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn forest_binding() -> BasicLandTypeBinding {
    BasicLandTypeBinding {
        card_definition: FOREST,
        land_type: BasicLandType::Forest,
    }
}

fn tapping(lands: &[ObjectId]) -> Vec<CastPaymentManaAbility> {
    lands
        .iter()
        .map(|land| {
            CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                land: *land,
                color: Color::Green,
            })
        })
        .collect()
}

/// Stocks every seat's library so no fixture turn ends in a draw loss.
fn stock_libraries(game: &mut Game, seats: usize, count: usize) {
    for seat in 0..seats {
        for _ in 0..count {
            game.add_card(PlayerId(seat), FILLER, Zone::Library)
                .expect("fixture library card is added before the game begins");
        }
    }
}

/// Advances the public turn machine to a turn and step, answering every
/// automatic decision with the most passive legal move.
fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..512 {
        if game.turn == turn && game.step == step {
            return;
        }
        assert!(!game.is_game_over(), "fixture ended before turn {turn}");
        let decision_seat = game.next_policy_player();
        let view = game
            .view_for_player(decision_seat)
            .expect("current decision view is available");
        if view.draw_replacement_pending {
            game.submit_policy_move(
                decision_seat,
                POLICY,
                PolicyAction::Draw {
                    decision: view
                        .draw_replacement_decision
                        .expect("pending draw has a decision identity"),
                    dredge: None,
                },
            )
            .expect("fixture has a card for each ordinary draw");
            continue;
        }
        if game.step == Step::DeclareAttackers && !view.attackers_declared {
            game.submit_policy_move(
                decision_seat,
                POLICY,
                PolicyAction::DeclareAttackers { attackers: vec![] },
            )
            .expect("empty attacker declaration is explicit");
            continue;
        }
        if game.step == Step::DeclareBlockers && !view.blockers_declared {
            game.submit_policy_move(
                decision_seat,
                POLICY,
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                },
            )
            .expect("empty blocker declaration is explicit");
            continue;
        }
        game.submit_policy_move(game.priority, POLICY, PolicyAction::PassPriority)
            .expect("the living priority holder advances the fixture");
    }
    panic!("fixture did not reach turn {turn}, step {step:?}");
}

/// Passes priority once for every living seat, which resolves one step.
fn pass_living_players(game: &mut Game) {
    let count = game.players.iter().filter(|player| !player.lost).count();
    for _ in 0..count {
        if game.is_game_over() {
            return;
        }
        let player = game.priority;
        game.submit_policy_move(player, POLICY, PolicyAction::PassPriority)
            .expect("each living player may pass once");
    }
}

/// Passes until combat damage has been dealt and the combat has ended.
fn resolve_combat(game: &mut Game) {
    for _ in 0..64 {
        if game.is_game_over() || game.step == Step::EndOfCombat {
            return;
        }
        let decision_seat = game.next_policy_player();
        let view = game
            .view_for_player(decision_seat)
            .expect("current decision view is available");
        if game.step == Step::DeclareBlockers && !view.blockers_declared {
            game.submit_policy_move(
                decision_seat,
                POLICY,
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                },
            )
            .expect("the defending seat declines to block");
            continue;
        }
        pass_living_players(game);
    }
    panic!("combat did not reach end of combat");
}

// ---------------------------------------------------------------------------
// Two-Headed Giant
// ---------------------------------------------------------------------------

fn two_headed_giant_game(team_life: i64) -> Game {
    let mut game = Game::new(definitions(), 4).expect("four-seat game initializes");
    game.configure_teams(
        &[
            vec![PlayerId(0), PlayerId(1)],
            vec![PlayerId(2), PlayerId(3)],
        ],
        team_life,
    )
    .expect("two teams of two is a legal Two-Headed Giant seating");
    game
}

/// Mirrors `formats_gold/test_engine.py::test_two_headed_giant_damage_hits_shared_team_life`.
///
/// One unblocked 3/3 attacking the opposing team reduces the *team's* shared
/// life, and taking it to zero eliminates both of that team's seats at once.
#[test]
fn two_headed_giant_unblocked_damage_hits_shared_team_life() {
    let mut game = two_headed_giant_game(30);
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 begins with a Watchwolf");
    // The opposing team is one Watchwolf swing from zero.
    game.set_fixture_player_life(PlayerId(2), 3)
        .expect("the opposing team's shared life is seeded");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    assert_eq!(
        game.team_life(TeamId(0)),
        Some(30),
        "team 0 starts on its configured shared life"
    );
    assert_eq!(
        game.team_life(TeamId(1)),
        Some(3),
        "seeding one seat seeds the whole team's shared total"
    );
    assert_eq!(
        game.players[PlayerId(3).0].life,
        3,
        "a teammate reports the same shared life total, not its own"
    );

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(watchwolf, DefenderChoice::Team(TeamId(1)))],
        },
    )
    .expect("a team attacks the opposing team, not a single seat");
    resolve_combat(&mut game);

    assert_eq!(
        game.team_life(TeamId(1)),
        Some(0),
        "unblocked combat damage came off the opposing team's shared life"
    );
    assert!(
        game.players[PlayerId(2).0].lost && game.players[PlayerId(3).0].lost,
        "a team at zero life loses as a team; seats: {:?}",
        game.players
            .iter()
            .map(|player| (player.id, player.life, player.lost))
            .collect::<Vec<_>>()
    );
    assert!(
        !game.players[PlayerId(0).0].lost && !game.players[PlayerId(1).0].lost,
        "the attacking team is untouched"
    );
    assert_eq!(
        game.winning_team(),
        Some(TeamId(0)),
        "one living team is the winner"
    );
    assert!(game.is_game_over(), "one living team ends the game");
    game.validate_invariants()
        .expect("the team-loss transition leaves a valid state");
}

/// Damage dealt to *either* seat on a team comes off the one shared total, and
/// a teammate's own life is never a separate number.
#[test]
fn two_headed_giant_damage_to_either_teammate_moves_one_total() {
    let mut game = two_headed_giant_game(30);
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 begins with a Watchwolf");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(watchwolf, DefenderChoice::Team(TeamId(1)))],
        },
    )
    .expect("the team attack is declared");
    resolve_combat(&mut game);

    assert_eq!(game.team_life(TeamId(1)), Some(27), "30 - 3 = 27");
    assert_eq!(
        game.players[PlayerId(2).0].life,
        game.players[PlayerId(3).0].life,
        "teammates never carry separate life totals"
    );
    assert_eq!(
        game.team_life(TeamId(0)),
        Some(30),
        "the attacking team's total is untouched"
    );
}

/// The turn is shared: both teammates untap and take a land drop on the team's
/// turn, and the next turn belongs to the *other* team rather than to the
/// active player's teammate.
#[test]
fn two_headed_giant_turn_is_shared_by_the_team() {
    let mut game = two_headed_giant_game(30);
    let mine = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 has a creature");
    let teammates = game
        .put_on_battlefield(PlayerId(1), SZADEK)
        .expect("seat 1 has a creature");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    assert_eq!(
        game.active_player,
        PlayerId(0),
        "the starting team's first seat takes the first turn"
    );
    advance_to(&mut game, 2, Step::Upkeep);
    assert_eq!(
        game.active_player,
        PlayerId(2),
        "the turn passes to the opposing team, not to the active player's teammate"
    );

    // Both teammates may attack on their shared turn.
    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![
                (mine, DefenderChoice::Team(TeamId(1))),
                (teammates, DefenderChoice::Team(TeamId(1))),
            ],
        },
    )
    .expect("a shared team turn lets both teammates attack");
    resolve_combat(&mut game);
    assert_eq!(
        game.team_life(TeamId(1)),
        Some(22),
        "3 + 5 unblocked damage came off one shared total"
    );

    // Both teammates untapped together at the shared team turn's untap step.
    assert_eq!(
        game.active_player,
        PlayerId(0),
        "seat 0 is the active seat of the attacking team"
    );
    advance_to(&mut game, 5, Step::Upkeep);
    assert!(
        [mine, teammates].iter().all(|card| !game
            .object(*card)
            .expect("fixture creature persists")
            .tapped),
        "both teammates untapped at the shared team turn's untap step"
    );
}

/// The starting team skips its shared first draw, and every later team turn
/// draws once for each living teammate.
#[test]
fn two_headed_giant_starting_team_skips_its_first_draw_then_each_teammate_draws() {
    let mut game = two_headed_giant_game(30);
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    advance_to(&mut game, 1, Step::PrecombatMain);
    assert_eq!(
        game.players[PlayerId(0).0].hand.len(),
        0,
        "the starting team skips its first draw"
    );
    assert_eq!(
        game.players[PlayerId(1).0].hand.len(),
        0,
        "the skip applies to the whole starting team, not just its first seat"
    );

    advance_to(&mut game, 2, Step::PrecombatMain);
    assert_eq!(
        game.players[PlayerId(2).0].hand.len(),
        1,
        "the second team's turn draws for its active seat"
    );
    assert_eq!(
        game.players[PlayerId(3).0].hand.len(),
        1,
        "a team does not share its draw: each living teammate draws for itself"
    );
}

/// A teammate is never a legal defender.
#[test]
fn two_headed_giant_cannot_attack_a_teammate() {
    let mut game = two_headed_giant_game(30);
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 has a creature");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");
    advance_to(&mut game, 3, Step::DeclareAttackers);

    let rejected = game.declare_attackers_against(
        PlayerId(0),
        &[(watchwolf, DefenderChoice::Player(PlayerId(1)))],
    );
    assert!(
        rejected.is_err(),
        "a seat cannot attack its own teammate: {rejected:?}"
    );
    assert!(
        !game
            .view_for_player(PlayerId(0))
            .expect("view")
            .attackers_declared,
        "a rejected declaration leaves no partial combat state"
    );
    game.validate_invariants()
        .expect("the rejected declaration was atomic");
}

/// Either teammate's creature may block an attack aimed at the team.
#[test]
fn two_headed_giant_either_teammate_may_block_a_team_attack() {
    let mut game = two_headed_giant_game(30);
    let attacker = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 attacks");
    let wall = game
        .put_on_battlefield(PlayerId(3), CHORUS)
        .expect("the far teammate holds the wall");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(attacker, DefenderChoice::Team(TeamId(1)))],
        },
    )
    .expect("the team attack is declared");
    advance_to(&mut game, 3, Step::DeclareBlockers);
    let declaring_seat = game.next_policy_player();
    assert_eq!(
        declaring_seat,
        PlayerId(2),
        "the attacked team's first living seat makes the team's declaration"
    );
    game.submit_policy_move(
        declaring_seat,
        POLICY,
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock {
                attacker,
                blocker: wall,
            }],
        },
    )
    .expect("a 3/8 controlled by the other teammate blocks for the team");
    resolve_combat(&mut game);

    assert_eq!(
        game.team_life(TeamId(1)),
        Some(30),
        "a blocked attacker deals no damage to the team"
    );
}

// ---------------------------------------------------------------------------
// mini-Commander
// ---------------------------------------------------------------------------

fn commander_game() -> Game {
    let mut game = Game::new_with_basic_land_types(definitions(), 4, [forest_binding()])
        .expect("four-seat game initializes");
    game.configure_commander_format(40, 21)
        .expect("Commander is 40 life and 21 commander damage");
    game
}

/// Seats a commander that is already in play. Designation leaves a
/// battlefield object where it is, exactly as a Commander game does after the
/// commander has been cast.
fn designate_on_battlefield(game: &mut Game, seat: PlayerId, definition: &'static str) -> ObjectId {
    let card = game
        .put_on_battlefield(seat, definition)
        .expect("the commander begins on the battlefield");
    game.designate_commander(seat, card)
        .expect("an owned object may be designated as a commander");
    assert_eq!(
        game.zone_of(card),
        Some(Zone::Battlefield),
        "designating an in-play commander does not move it"
    );
    card
}

/// Designates a commander that starts in the command zone.
fn designate_in_command_zone(
    game: &mut Game,
    seat: PlayerId,
    definition: &'static str,
) -> ObjectId {
    let card = game
        .add_card(seat, definition, Zone::Hand)
        .expect("the commander card exists");
    game.designate_commander(seat, card)
        .expect("the owner designates its commander");
    card
}

/// Mirrors `formats_gold/test_engine.py::test_commander_unblocked_swing_reaches_21`.
///
/// Seat 1's commander has already dealt 15. One unblocked 6-power swing takes
/// the running total to 21, which eliminates seat 0 outright — its life total
/// is still 34 at the time.
#[test]
fn commander_unblocked_swing_reaches_21_and_eliminates() {
    let mut game = commander_game();
    let razia = designate_on_battlefield(&mut game, PlayerId(1), RAZIA);
    designate_in_command_zone(&mut game, PlayerId(0), TOLSIMIR);
    game.set_fixture_commander_damage(PlayerId(0), razia, 15)
        .expect("this table has already taken 15 from that commander");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    // Seat 1 takes turn 2 in a four-seat free-for-all.
    advance_to(&mut game, 2, Step::DeclareAttackers);
    assert_eq!(game.active_player, PlayerId(1));
    game.submit_policy_move(
        PlayerId(1),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(razia, DefenderChoice::Player(PlayerId(0)))],
        },
    )
    .expect("a free-for-all attacker chooses which seat it attacks");
    resolve_combat(&mut game);

    assert_eq!(
        game.commander_damage(PlayerId(0), razia),
        21,
        "15 prior + 6 unblocked is 21 commander damage"
    );
    assert!(
        game.players[PlayerId(0).0].lost,
        "21 commander damage from one commander eliminates a seat"
    );
    assert_eq!(
        game.players[PlayerId(0).0].life,
        34,
        "the seat left with a positive life total: commander damage is its own loss condition"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::PlayerLost { player, reason }
                if *player == PlayerId(0) && *reason == "took lethal commander damage"
        )),
        "the loss receipt names commander damage; events: {:?}",
        game.event_log
    );
    assert!(
        !game.is_game_over(),
        "three seats remain in the free-for-all"
    );
    game.validate_invariants()
        .expect("the commander-damage loss leaves a valid state");
}

/// The commander-damage ledger is real accumulation across separate combats,
/// not a fixture constant: two unblocked 11-point swings from the same
/// commander object reach the threshold with no seeding at all.
#[test]
fn commander_damage_accumulates_across_combats_without_seeding() {
    let mut game = commander_game();
    let szadek = designate_on_battlefield(&mut game, PlayerId(1), SZADEK);
    stock_libraries(&mut game, 4, 40);
    game.begin_game().expect("game starts");

    for turn in [2, 6] {
        advance_to(&mut game, turn, Step::DeclareAttackers);
        assert_eq!(game.active_player, PlayerId(1));
        game.submit_policy_move(
            PlayerId(1),
            POLICY,
            PolicyAction::DeclareAttackersAgainst {
                attackers: vec![(szadek, DefenderChoice::Player(PlayerId(0)))],
            },
        )
        .expect("the commander attacks the same seat each turn");
        resolve_combat(&mut game);
    }

    assert_eq!(
        game.commander_damage(PlayerId(0), szadek),
        10,
        "two unblocked 5-power swings accumulate on one ledger entry"
    );
    assert_eq!(
        game.players[PlayerId(0).0].life,
        30,
        "commander damage is dealt in addition to, not instead of, life loss"
    );
    assert!(
        !game.players[PlayerId(0).0].lost,
        "10 is below the 21 threshold"
    );
    assert_eq!(
        game.commander_damage(PlayerId(2), szadek),
        0,
        "commander damage is tracked per damaged seat"
    );
}

/// CR 903.8: casting a commander from the command zone costs {2} more for each
/// prior command-zone cast of that commander. The second cast here is charged
/// the real extra mana, not merely reported as taxed.
#[test]
#[allow(clippy::too_many_lines)] // Cast, kill, recast is one tax transaction to audit.
fn commander_tax_adds_two_generic_per_prior_command_zone_cast() {
    let mut game = commander_game();
    // Agrus Kos costs five, so an untaxed cast needs five Forests and the
    // first taxed cast needs seven.
    let agrus = designate_in_command_zone(&mut game, PlayerId(0), AGRUS);
    let forests = (0..7)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), FOREST)
                .expect("Forest setup")
        })
        .collect::<Vec<_>>();
    let wall = game
        .put_on_battlefield(PlayerId(1), CHORUS)
        .expect("seat 1 holds the blocker that will kill the commander");
    stock_libraries(&mut game, 4, 24);

    assert_eq!(
        game.command_zone(PlayerId(0)),
        &[agrus],
        "a designated commander begins in the command zone"
    );
    assert_eq!(game.commander_tax(agrus), 0, "no prior casts, no tax");

    game.begin_game().expect("game starts");
    advance_to(&mut game, 1, Step::PrecombatMain);

    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: agrus,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: tapping(&forests[..5]),
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Green; 5],
                hybrid: vec![],
            },
        },
    )
    .expect("a seat may cast its own commander from the command zone for its printed cost");

    assert_eq!(
        game.commander_tax(agrus),
        2,
        "one prior command-zone cast taxes the next by two generic"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CommanderCastFromCommandZone { commander, tax, .. }
                if *commander == agrus && *tax == 0
        )),
        "the first cast records a zero tax; events: {:?}",
        game.event_log
    );
    assert!(
        game.command_zone(PlayerId(0)).is_empty(),
        "the commander left the command zone for the stack"
    );

    // Resolve it, then send it back to the command zone the way a real game
    // does: the fixture blocks it to death on the opposing seat's turn.
    pass_living_players(&mut game);
    assert_eq!(game.zone_of(agrus), Some(Zone::Battlefield));
    advance_to(&mut game, 5, Step::DeclareAttackers);
    assert_eq!(game.active_player, PlayerId(0));
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(agrus, DefenderChoice::Player(PlayerId(1)))],
        },
    )
    .expect("the commander attacks seat 1");
    advance_to(&mut game, 5, Step::DeclareBlockers);
    game.submit_policy_move(
        PlayerId(1),
        POLICY,
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock {
                attacker: agrus,
                blocker: wall,
            }],
        },
    )
    .expect("the 3/8 blocks and kills the 3/3 commander");
    resolve_combat(&mut game);
    assert_eq!(
        game.zone_of(agrus),
        Some(Zone::Command),
        "the dead commander went back to the command zone"
    );

    // Five Forests is now short: the second command-zone cast costs seven.
    advance_to(&mut game, 9, Step::PrecombatMain);
    assert_eq!(game.active_player, PlayerId(0));
    let underpaid = game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: agrus,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: tapping(&forests[..5]),
        },
        ManaPaymentSelection {
            generic: vec![Color::Green; 5],
            hybrid: vec![],
        },
    );
    assert!(
        underpaid.is_err(),
        "the printed cost alone no longer pays for a taxed commander: {underpaid:?}"
    );
    assert_eq!(
        game.command_zone(PlayerId(0)),
        &[agrus],
        "the rejected cast left the commander in the command zone"
    );
    assert!(
        forests
            .iter()
            .all(|land| !game.object(*land).expect("fixture land persists").tapped),
        "a rejected cast taps nothing"
    );

    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: agrus,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: tapping(&forests),
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Green; 7],
                hybrid: vec![],
            },
        },
    )
    .expect("five printed plus two tax pays for the second command-zone cast");
    assert_eq!(
        game.commander_tax(agrus),
        4,
        "two prior command-zone casts tax the next by four generic"
    );
}

/// Only the owner may cast a commander, and only from its own command zone.
#[test]
fn only_the_owner_casts_its_commander_from_the_command_zone() {
    let mut game = commander_game();
    let tolsimir = designate_in_command_zone(&mut game, PlayerId(0), TOLSIMIR);
    let forests = (0..5)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), FOREST)
                .expect("Forest setup")
        })
        .collect::<Vec<_>>();
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");
    advance_to(&mut game, 2, Step::PrecombatMain);
    assert_eq!(game.active_player, PlayerId(1));

    let rejected = game.cast_spell_with_mana_spend(
        PlayerId(1),
        CastRequest {
            card: tolsimir,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: tapping(&forests),
        },
        ManaPaymentSelection {
            generic: vec![Color::Green; 5],
            hybrid: vec![],
        },
    );
    assert!(
        rejected.is_err(),
        "another seat cannot cast a commander out of its owner's command zone: {rejected:?}"
    );
    assert_eq!(
        game.command_zone(PlayerId(0)),
        &[tolsimir],
        "the rejected cast left the command zone untouched"
    );
}

/// CR 903.9a: a commander that would die goes to the command zone instead, and
/// its next cast is taxed.
#[test]
fn a_dying_commander_is_replaced_into_the_command_zone() {
    let mut game = commander_game();
    let agrus = designate_on_battlefield(&mut game, PlayerId(1), AGRUS);
    let wall = game
        .put_on_battlefield(PlayerId(0), CHORUS)
        .expect("seat 0 holds a 3/8");
    stock_libraries(&mut game, 4, 24);
    game.begin_game().expect("game starts");

    advance_to(&mut game, 2, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(1),
        POLICY,
        PolicyAction::DeclareAttackersAgainst {
            attackers: vec![(agrus, DefenderChoice::Player(PlayerId(0)))],
        },
    )
    .expect("the commander attacks seat 0");
    advance_to(&mut game, 2, Step::DeclareBlockers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock {
                attacker: agrus,
                blocker: wall,
            }],
        },
    )
    .expect("the 3/8 blocks the 3/3 commander");
    resolve_combat(&mut game);

    assert_eq!(
        game.zone_of(agrus),
        Some(Zone::Command),
        "a commander that would die goes to the command zone instead"
    );
    assert_eq!(
        game.command_zone(PlayerId(1)),
        &[agrus],
        "the replacement puts it into its owner's command zone"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CommanderReturnedToCommandZone { commander, player }
                if *commander == agrus && *player == PlayerId(1)
        )),
        "the replacement is on the record; events: {:?}",
        game.event_log
    );
    assert_eq!(
        game.commander_damage(PlayerId(0), agrus),
        0,
        "a blocked commander dealt no damage to the seat"
    );
    game.validate_invariants()
        .expect("the command-zone replacement leaves a valid state");
}

// ---------------------------------------------------------------------------
// Regression: the two-seat duel is untouched
// ---------------------------------------------------------------------------

/// The load-bearing two-seat path. A duel still derives its defender as the
/// next living seat with no defender choice submitted at all, and its combat
/// damage still lands on that seat.
#[test]
fn two_player_duel_still_derives_the_defender_as_the_next_seat() {
    let mut game = Game::new(definitions(), 2).expect("two-seat duel initializes");
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), WATCHWOLF)
        .expect("seat 0 begins with a Watchwolf");
    stock_libraries(&mut game, 2, 24);
    game.begin_game().expect("game starts");

    assert_eq!(game.team_of(PlayerId(0)), None, "a duel has no teams");
    assert!(
        game.winning_team().is_none(),
        "a duel never reports a winning team"
    );

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        PlayerId(0),
        POLICY,
        PolicyAction::DeclareAttackers {
            attackers: vec![watchwolf],
        },
    )
    .expect("a duel declares attackers without naming a defender");
    resolve_combat(&mut game);

    assert_eq!(
        game.players[PlayerId(1).0].life,
        17,
        "the derived defender is the next seat and took the damage"
    );
    assert_eq!(
        game.players[PlayerId(0).0].life,
        20,
        "the attacking seat is untouched"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer { player, amount, .. }
                if *player == PlayerId(1) && *amount == 3
        )),
        "the duel damage receipt is unchanged; events: {:?}",
        game.event_log
    );
}

/// The starting player of a *two-player* game still skips its first draw. The
/// arity check is CR 103.8a itself, so generalising it would be a rules bug.
#[test]
fn two_player_first_draw_skip_survives_the_team_work() {
    let mut game = Game::new(definitions(), 2).expect("two-seat duel initializes");
    stock_libraries(&mut game, 2, 24);
    game.begin_game().expect("game starts");
    advance_to(&mut game, 1, Step::PrecombatMain);
    assert_eq!(
        game.players[PlayerId(0).0].hand.len(),
        0,
        "the two-player starting seat skips its first draw"
    );

    let mut pod = Game::new(definitions(), 3).expect("three-seat pod initializes");
    stock_libraries(&mut pod, 3, 24);
    pod.begin_game().expect("game starts");
    advance_to(&mut pod, 1, Step::PrecombatMain);
    assert_eq!(
        pod.players[PlayerId(0).0].hand.len(),
        1,
        "a multiplayer game does not skip the first draw"
    );
}
