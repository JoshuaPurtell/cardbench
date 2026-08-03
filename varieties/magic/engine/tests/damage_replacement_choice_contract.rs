//! Focused contracts for the bounded prospective damage-replacement pipeline.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DamageReplacementChoice, Effect, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Target, TargetRequirement, Zone,
};

const REDIRECTOR: &str = "TST-DAMAGE-REPLACEMENT-REDIRECTOR";
const TARGET: &str = "TST-DAMAGE-REPLACEMENT-TARGET";
const SHIELD: &str = "TST-DAMAGE-REPLACEMENT-SHIELD";
const BOLT: &str = "TST-DAMAGE-REPLACEMENT-BOLT";

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
        supported_rules: &["prospective-damage-replacement-probe"],
        power: if id == TARGET || id == REDIRECTOR {
            Some(4)
        } else {
            None
        },
        toughness: if id == TARGET || id == REDIRECTOR {
            Some(4)
        } else {
            None
        },
        keywords: vec![],
        effects,
    }
}

fn game() -> (
    Game,
    PlayerId,
    PlayerId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        vec![
            definition(REDIRECTOR, BTreeSet::from([CardType::Creature]), vec![]),
            definition(TARGET, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
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
        .expect("redirector enters");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("target enters");
    (game, caster, opponent, redirector, target)
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

fn cast_and_resolve_shield(
    game: &mut Game,
    caster: PlayerId,
    target: cardbench_magic_engine::ObjectId,
) {
    let shield = game
        .add_card(caster, SHIELD, Zone::Hand)
        .expect("shield in hand");
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
    pass_pair(game);
}

fn activate_and_resolve_redirect(
    game: &mut Game,
    caster: PlayerId,
    opponent: PlayerId,
    redirector: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) {
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
    pass_pair(game);
}

fn cast_bolt_to_choice(
    game: &mut Game,
    caster: PlayerId,
    target: cardbench_magic_engine::ObjectId,
) -> cardbench_magic_engine::DamageReplacementChoiceView {
    let bolt = game
        .add_card(caster, BOLT, Zone::Hand)
        .expect("bolt in hand");
    game.cast_spell(
        caster,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bolt casts");
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass opens replacement decision");
    game.view_for_player(caster)
        .expect("affected player view")
        .damage_replacement_choice
        .expect("two applicable replacements require an explicit choice")
}

fn choose(
    game: &mut Game,
    player: PlayerId,
    choice: &cardbench_magic_engine::DamageReplacementChoiceView,
    replacement: DamageReplacementChoice,
) {
    game.submit_policy_move(
        player,
        "damage-replacement-contract",
        PolicyAction::ChooseDamageReplacement {
            decision: choice.decision,
            source: choice.source,
            source_incarnation: choice.source_incarnation,
            target: choice.target,
            replacement,
        },
    )
    .expect("affected player selects replacement");
}

#[test]
fn affected_player_can_prevent_instead_of_redirecting() {
    let (mut game, caster, opponent, redirector, target) = game();
    cast_and_resolve_shield(&mut game, caster, target);
    activate_and_resolve_redirect(&mut game, caster, opponent, redirector, target);
    let choice = cast_bolt_to_choice(&mut game, caster, target);
    assert_eq!(choice.target, Target::Permanent(target));
    assert_eq!(choice.amount, 2);
    assert_eq!(choice.replacements.len(), 2);
    assert!(
        game.pass_priority(caster).is_err(),
        "choice blocks priority"
    );
    let shield = choice
        .replacements
        .iter()
        .copied()
        .find(|replacement| matches!(replacement, DamageReplacementChoice::TargetedShield { .. }))
        .expect("targeted shield is one live choice");
    choose(&mut game, caster, &choice, shield);

    eprintln!("prevent-first trace: {:?}", game.canonical_event_log());
    assert_eq!(game.object(target).expect("target").damage, 0);
    assert_eq!(game.player(opponent).expect("opponent").life, 20);
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::DamageReplacementApplied { replacement, .. },
            GameEvent::DamagePrevented { target: Target::Permanent(card), amount: 2, .. },
        ] if *replacement == shield && *card == target
    )));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::DamageDealtToPermanent { .. }))
    );
    game.validate_invariants().expect("choice trace is valid");
}

#[test]
fn affected_player_can_redirect_instead_of_preventing() {
    let (mut game, caster, opponent, redirector, target) = game();
    cast_and_resolve_shield(&mut game, caster, target);
    activate_and_resolve_redirect(&mut game, caster, opponent, redirector, target);
    let choice = cast_bolt_to_choice(&mut game, caster, target);
    let redirect = choice
        .replacements
        .iter()
        .copied()
        .find(|replacement| matches!(replacement, DamageReplacementChoice::Redirect { .. }))
        .expect("redirection is one live choice");
    choose(&mut game, caster, &choice, redirect);

    eprintln!("redirect-first trace: {:?}", game.canonical_event_log());
    assert_eq!(game.object(target).expect("target").damage, 0);
    assert_eq!(game.player(opponent).expect("opponent").life, 18);
    assert!(game.event_log.windows(3).any(|events| matches!(
        events,
        [
            GameEvent::DamageReplacementApplied { replacement, .. },
            GameEvent::DamageRedirected { from, to: Target::Player(player), amount: 2, .. },
            GameEvent::DamageDealtToPlayer { player: dealt_to, amount: 2, .. },
        ] if *replacement == redirect && *from == target && *player == opponent && *dealt_to == opponent
    )));
    game.validate_invariants().expect("choice trace is valid");
}
