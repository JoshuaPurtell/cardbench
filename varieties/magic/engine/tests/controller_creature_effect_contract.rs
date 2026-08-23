//! Expansion-neutral contracts for combat-count damage and controller-wide P/T.
//!
//! These synthetic cards exercise the reusable semantics without making a
//! claim about any specific expansion. They keep combat selection, target
//! revalidation, layer-7 lifecycle, and state-machine progression auditable.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    Step, Target, TargetRequirement, Zone,
};

const DOGPILE_STYLE: &str = "TST-ATTACKING-CREATURE-DAMAGE";
const WIDE_BOOST: &str = "TST-CONTROLLER-WIDE-BOOST";
const CREATURE: &str = "TST-CREATURE";
const ARTIFACT: &str = "TST-ARTIFACT";
const DRAW_FILLER: &str = "TST-DRAW-FILLER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: DOGPILE_STYLE,
            name: DOGPILE_STYLE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["attacking-creature-count-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageEqualToAttackingCreatures {
                target: TargetRequirement::Any,
            }],
        },
        CardDefinition {
            id: WIDE_BOOST,
            name: WIDE_BOOST,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["controller-creature-layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                power: 3,
                toughness: 3,
            }],
        },
        creature(CREATURE),
        CardDefinition {
            id: ARTIFACT,
            name: ARTIFACT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["artifact-characteristics"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: DRAW_FILLER,
            name: DRAW_FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["fixture-library-filler"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn game() -> Game {
    Game::new(definitions(), 2).expect("controller-creature contract game initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player resolves the spell");
}

fn advance_one_policy_action(game: &mut Game) {
    let active = game.active_player;
    if game
        .view_for_player(active)
        .expect("active player has a view")
        .draw_replacement_pending
    {
        game.draw_card(active, None)
            .expect("ordinary draw resolves before priority");
        return;
    }
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(active)
            .expect("active player has combat view")
            .attackers_declared
    {
        game.declare_attackers(active, &[])
            .expect("empty attackers advance test turn structure");
        return;
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("defending player has combat view")
            .blockers_declared
    {
        let defender = game.next_policy_player();
        game.declare_blockers(defender, &[])
            .expect("empty blockers advance test turn structure");
        return;
    }
    let player = game.priority;
    game.pass_priority(player)
        .expect("priority holder advances test turn structure");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    while game.turn != turn || game.step != step {
        advance_one_policy_action(game);
        game.validate_invariants()
            .expect("every ordinary transition preserves invariants");
        assert!(game.turn <= turn, "turn machine advanced beyond target");
    }
}

#[test]
fn attacking_creature_damage_uses_the_resolution_time_combat_set() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), DOGPILE_STYLE, Zone::Hand)
        .expect("damage spell enters hand");
    let first = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("first attacker enters before game start");
    let second = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("second attacker enters before game start");
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..3 {
            game.add_card(player, DRAW_FILLER, Zone::Library)
                .expect("fixture draw filler enters library");
        }
    }
    game.begin_game().expect("game starts");
    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[first, second])
        .expect("both established creatures attack on turn three");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("combat-count damage spell casts after attackers are declared");
    resolve_top(&mut game);

    assert_eq!(game.players[1].life, 18);
    assert_eq!(
        game.event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::DamageDealtToPlayer {
                    source,
                    player,
                    amount,
                } if *source == spell && *player == PlayerId(1) => Some(*amount),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![2],
        "one receipt must carry the current number of attacking creatures"
    );
    game.validate_invariants()
        .expect("combat-count damage leaves a valid game state");
}

#[test]
fn attacking_creature_damage_records_no_damage_when_no_creature_is_attacking() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), DOGPILE_STYLE, Zone::Hand)
        .expect("damage spell enters hand");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell remains legal outside combat");
    resolve_top(&mut game);

    assert_eq!(game.players[1].life, 20);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer { source, .. } if *source == spell
        )),
        "zero damage must not manufacture a damage receipt"
    );
    game.validate_invariants()
        .expect("zero-count resolution leaves a valid game state");
}

#[test]
fn controller_wide_modifier_selects_only_own_creatures_and_expires_at_cleanup() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), WIDE_BOOST, Zone::Hand)
        .expect("boost enters hand");
    let first = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("first own creature enters");
    let second = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("second own creature enters");
    let opponent = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("opposing creature enters");
    let artifact = game
        .put_on_battlefield(PlayerId(0), ARTIFACT)
        .expect("own noncreature enters");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller-wide boost casts without a target");
    resolve_top(&mut game);

    for creature in [first, second] {
        let characteristics = game.characteristics(creature).expect("creature remains");
        assert_eq!(characteristics.power, Some(5));
        assert_eq!(characteristics.toughness, Some(5));
    }
    assert_eq!(
        game.characteristics(opponent)
            .expect("opponent remains")
            .power,
        Some(2)
    );
    assert_eq!(
        game.characteristics(artifact)
            .expect("artifact remains")
            .power,
        None
    );
    let recipients = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::ContinuousEffectCreated { source, target, .. } if *source == spell => {
                Some(*target)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(recipients, vec![first, second]);

    advance_to(&mut game, 2, Step::Upkeep);
    for creature in [first, second] {
        let characteristics = game.characteristics(creature).expect("creature remains");
        assert_eq!(characteristics.power, Some(2));
        assert_eq!(characteristics.toughness, Some(2));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target, .. }
            if *source == spell && (*target == first || *target == second)
    )));
    game.validate_invariants()
        .expect("controller-wide modifier lifecycle preserves invariants");
}
