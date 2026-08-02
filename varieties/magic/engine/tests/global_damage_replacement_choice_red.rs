//! Red regression: every prospective damage packet from a global spell must
//! preserve the affected player's replacement ordering choice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DecisionKind, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const REDIRECTOR: &str = "TST-GLOBAL-DAMAGE-REDIRECTOR";
const TARGET: &str = "TST-GLOBAL-DAMAGE-TARGET";
const SHIELD: &str = "TST-GLOBAL-DAMAGE-SHIELD";
const SWEEP: &str = "TST-GLOBAL-DAMAGE-SWEEP";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["global-damage-replacement-choice-red"],
        power: (id == TARGET || id == REDIRECTOR).then_some(4),
        toughness: (id == TARGET || id == REDIRECTOR).then_some(4),
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
fn every_global_damage_recipient_preserves_replacement_choice() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            definition(REDIRECTOR, BTreeSet::from([CardType::Creature]), vec![]),
            definition(TARGET, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
            definition(
                SWEEP,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 2 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: REDIRECTOR,
            ability: ActivatedAbility {
                id: "redirect-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    TargetRequirement::Creature,
                    TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 2 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        }],
    )
    .expect("fixture initializes");
    let redirector = game
        .put_on_battlefield(caster, REDIRECTOR)
        .expect("redirector enters before game start");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("damage target enters before game start");
    let shield = game
        .add_card(caster, SHIELD, Zone::Hand)
        .expect("shield starts in hand");
    let sweep = game
        .add_card(caster, SWEEP, Zone::Hand)
        .expect("global spell starts in hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        caster,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("shield casts");
    pass_pair(&mut game);
    game.activate_ability(
        caster,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target), Target::Player(opponent)],
        },
    )
    .expect("redirection activates");
    pass_pair(&mut game);

    game.cast_spell(
        caster,
        CastRequest {
            card: sweep,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("global damage spell casts");
    pass_pair(&mut game);

    eprintln!(
        "global damage replacement red trace: stack={:?}; pending={:?}; events={:?}",
        game.stack,
        game.view_for_player(caster)
            .expect("affected-player view")
            .pending_decision,
        game.canonical_event_log(),
    );
    let decision = game
        .view_for_player(caster)
        .expect("affected-player view")
        .pending_decision
        .expect("the protected global-damage packet must open a replacement decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(decision.replacement_candidates.len(), 2);
    assert_eq!(game.stack.len(), 1, "the global spell remains unresolved");
    assert!(
        !game.event_log.iter().any(|event| {
            matches!(event, GameEvent::DamageRedirected { from, .. } if *from == target)
                || matches!(event, GameEvent::DamagePrevented { target: Target::Permanent(permanent), .. } if *permanent == target)
        }),
        "no deterministic replacement may commit before the affected player chooses"
    );
    game.validate_invariants()
        .expect("paused global replacement boundary is state-machine valid");
}
