//! Red regression: an untargeted "deal damage to each player" instruction
//! must retain a concurrent replacement choice for each affected player.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DamageReplacementChoice, DamageReplacementEffect,
    DamageReplacementEffectBinding, DecisionKind, DecisionSelection, Effect, Game, GameEvent,
    ManaCost, PlayerId, ReplacementChoice, Target, Zone,
};

const WAVE: &str = "TST-GLOBAL-EACH-PLAYER-WAVE";
const HALVER: &str = "TST-GLOBAL-EACH-PLAYER-HALVER";
const SHIELD: &str = "TST-GLOBAL-EACH-PLAYER-SHIELD";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["global-each-player-damage-replacement-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // One test owns the complete affected-player choice and resumption trace.
fn each_player_damage_pauses_for_the_shielded_players_replacement_order() {
    let caster = PlayerId(0);
    let affected = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                WAVE,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamageToEachPlayer { amount: 2 }],
            ),
            definition(HALVER, BTreeSet::from([CardType::Enchantment]), vec![]),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
        ],
        2,
    )
    .expect("fixture constructs");
    game.register_damage_replacement_effect_bindings([DamageReplacementEffectBinding {
        source_definition: HALVER,
        effect: DamageReplacementEffect::HalveDamage,
    }])
    .expect("halver binding registers");
    game.put_on_battlefield(caster, HALVER)
        .expect("halver enters before game start");
    let shield = game
        .add_card(affected, SHIELD, Zone::Hand)
        .expect("shield starts in hand");
    let wave = game
        .add_card(caster, WAVE, Zone::Hand)
        .expect("wave starts in hand");
    game.begin_game().expect("game begins");

    game.pass_priority(caster)
        .expect("caster passes for the shield");
    game.cast_spell(
        affected,
        CastRequest {
            card: shield,
            targets: vec![Target::Player(affected)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("affected player creates a shield");
    pass_pair(&mut game);
    game.cast_spell(
        caster,
        CastRequest {
            card: wave,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("wave casts");
    pass_pair(&mut game);

    let view = game
        .view_for_player(affected)
        .expect("affected player view");
    eprintln!(
        "global each-player replacement red trace: pending={:?}; lives=({}, {}); events={:?}",
        view.pending_decision,
        game.player(caster).expect("caster exists").life,
        game.player(affected).expect("affected exists").life,
        game.canonical_event_log(),
    );
    let decision = view
        .pending_decision
        .expect("shielded player must order concurrent global-damage replacements");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(game.next_policy_player(), affected);
    assert_eq!(game.player(affected).expect("affected exists").life, 20);
    game.validate_invariants()
        .expect("suspended each-player damage packet remains valid");

    let shield_replacement = decision
        .replacement_candidates
        .iter()
        .copied()
        .find_map(|choice| match choice {
            ReplacementChoice::Damage(
                replacement @ DamageReplacementChoice::TargetedShield { .. },
            ) => Some(replacement),
            ReplacementChoice::Damage(_) | ReplacementChoice::Quantity { .. } => None,
        })
        .expect("the public options include the targeted shield");
    game.submit_decision(
        affected,
        decision.id,
        DecisionSelection::Replacements(vec![ReplacementChoice::Damage(shield_replacement)]),
    )
    .expect("affected player chooses prevention");

    eprintln!(
        "global each-player replacement green trace: {:?}",
        game.canonical_event_log()
    );
    assert!(game.stack.is_empty(), "the global spell resolves once");
    assert_eq!(game.player(caster).expect("caster exists").life, 19);
    assert_eq!(game.player(affected).expect("affected exists").life, 20);
    assert_eq!(game.zone_of(wave), Some(Zone::Graveyard));
    assert!(game.event_log.windows(3).any(|events| matches!(
        events,
        [
            GameEvent::DamageReplacementApplied {
                affected_player: PlayerId(1),
                target: Target::Player(PlayerId(1)),
                replacement: DamageReplacementChoice::TargetedShield { .. },
            },
            GameEvent::DamagePrevented {
                target: Target::Player(PlayerId(1)),
                amount: 2,
                ..
            },
            GameEvent::DecisionCompleted {
                decision: completed,
                kind: DecisionKind::Replacement,
                ..
            },
        ] if *completed == decision.id
    )));
    game.validate_invariants()
        .expect("resumed each-player damage packet remains valid");
}
