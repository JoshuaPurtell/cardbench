//! Red regression for an end-of-turn creature-source damage replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DamageReplacementChoice, DecisionKind,
    DecisionSelection, Effect, Game, GameEvent, ManaCost, PlayerId, ReplacementChoice, Step,
    Target, TargetRequirement, Zone,
};

const CHANT: &str = "TST-CREATURE-DAMAGE-CHANT";
const SHIELD: &str = "TST-CREATURE-DAMAGE-SHIELD";
const CREATURE_BOLT: &str = "TST-CREATURE-DAMAGE-BOLT-CREATURE";
const NONCREATURE_BOLT: &str = "TST-CREATURE-DAMAGE-BOLT-INSTANT";
const ATTACKER: &str = "TST-CREATURE-DAMAGE-ATTACKER";
const LAND: &str = "TST-CREATURE-DAMAGE-LAND";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types = card_types.into_iter().collect::<BTreeSet<_>>();
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["global-creature-damage-prevention-red"],
        power: creature.then_some(if id == ATTACKER { 3 } else { 1 }),
        toughness: creature.then_some(if id == ATTACKER { 3 } else { 1 }),
        keywords: vec![],
        effects,
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        definition(
            CHANT,
            [CardType::Instant],
            vec![Effect::PreventAllCreatureDamageAndGainLifeEqualToDamagePreventedUntilEndOfTurn],
        ),
        definition(
            SHIELD,
            [CardType::Instant],
            vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
        ),
        definition(
            CREATURE_BOLT,
            [CardType::Creature, CardType::Instant],
            vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::Player,
            }],
        ),
        definition(
            NONCREATURE_BOLT,
            [CardType::Instant],
            vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Player,
            }],
        ),
        definition(ATTACKER, [CardType::Creature], vec![]),
        CardDefinition {
            id: LAND,
            name: LAND,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::White]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["opening-library-fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, LAND, Zone::Library)
            .expect("opening library fixture card exists");
    }
}

fn advance_to_opponent_declare_attackers(game: &mut Game) {
    while game.active_player != PlayerId(1) || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn creature_damage_is_prevented_and_life_tracks_only_the_remaining_amount() {
    let controller = PlayerId(0);
    let damaged_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    let chant = game.add_card(controller, CHANT, Zone::Hand).expect("chant");
    let shield = game
        .add_card(controller, SHIELD, Zone::Hand)
        .expect("shield");
    let creature_bolt = game
        .add_card(damaged_player, CREATURE_BOLT, Zone::Hand)
        .expect("creature damage spell");
    let noncreature_bolt = game
        .add_card(damaged_player, NONCREATURE_BOLT, Zone::Hand)
        .expect("noncreature damage spell");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: chant,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("creature damage prevention resolves");
    pass_pair(&mut game);

    game.cast_spell(
        controller,
        CastRequest {
            card: shield,
            targets: vec![Target::Player(damaged_player)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("one point shield resolves");
    pass_pair(&mut game);
    game.pass_priority(controller)
        .expect("controller passes for the opponent's instant");
    game.cast_spell(
        damaged_player,
        CastRequest {
            card: creature_bolt,
            targets: vec![Target::Player(damaged_player)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("creature source damage spell casts");
    pass_pair(&mut game);

    let decision = game
        .view_for_player(damaged_player)
        .expect("affected player view")
        .pending_decision
        .expect("the affected player orders the shield and Chant replacements");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    let shield_choice = decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| {
            matches!(
                choice,
                ReplacementChoice::Damage(DamageReplacementChoice::TargetedShield { .. })
            )
        })
        .expect("targeted shield is an available replacement");
    game.submit_decision(
        damaged_player,
        decision.id,
        DecisionSelection::Replacements(vec![shield_choice]),
    )
    .expect("the affected player orders the finite shield first");

    assert_eq!(
        game.player(damaged_player).expect("damaged player").life,
        20
    );
    assert_eq!(game.player(controller).expect("Chant controller").life, 22);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CreatureDamagePrevented {
            source,
            prevented_by,
            controller: gained_player,
            target: Target::Player(target),
            amount: 2,
            ..
        } if *source == creature_bolt
            && *prevented_by == chant
            && *gained_player == controller
            && *target == damaged_player
    )));
    let prevented_at = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CreatureDamagePrevented { amount: 2, .. }))
        .expect("the typed prevented-damage receipt exists");
    assert!(matches!(
        game.event_log.get(prevented_at + 1),
        Some(GameEvent::LifeGained { player, amount: 2 }) if *player == controller
    ));

    game.pass_priority(controller)
        .expect("controller passes for noncreature damage");
    game.cast_spell(
        damaged_player,
        CastRequest {
            card: noncreature_bolt,
            targets: vec![Target::Player(controller)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("noncreature damage spell casts");
    pass_pair(&mut game);
    assert_eq!(game.player(controller).expect("controller").life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount: 2 }
            if *source == noncreature_bolt && *player == controller
    )));
    game.validate_invariants()
        .expect("creature prevention receipts preserve invariants");
}

#[test]
fn creature_combat_damage_is_prevented_and_becomes_life() {
    let controller = PlayerId(0);
    let attacker_controller = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    add_opening_library(&mut game, controller);
    add_opening_library(&mut game, attacker_controller);
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker setup");
    let chant = game.add_card(controller, CHANT, Zone::Hand).expect("chant");
    game.begin_game().expect("game begins");
    advance_to_opponent_declare_attackers(&mut game);

    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker declares");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes for the response");
    game.cast_spell(
        controller,
        CastRequest {
            card: chant,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Chant resolves before combat damage");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(controller, &[])
        .expect("no blockers declared");
    pass_pair(&mut game);

    assert_eq!(game.player(controller).expect("controller").life, 23);
    assert_eq!(game.player(attacker_controller).expect("attacker").life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CreatureDamagePrevented {
            source,
            prevented_by,
            controller: gained_player,
            target: Target::Player(target),
            amount: 3,
            ..
        } if *source == attacker
            && *prevented_by == chant
            && *gained_player == controller
            && *target == controller
    )));
    game.validate_invariants()
        .expect("combat prevention receipts preserve invariants");
}
