//! Red regression: a replacement choice must suspend the exact damage
//! instruction in a multi-effect spell, without skipping the affected
//! player's CR 616 ordering choice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DecisionKind, Effect, Game, ManaCost, PlayerId, Target, TargetRequirement,
    Zone,
};

const REDIRECTOR: &str = "TST-MULTI-DAMAGE-REDIRECTOR";
const TARGET: &str = "TST-MULTI-DAMAGE-TARGET";
const SHIELD: &str = "TST-MULTI-DAMAGE-SHIELD";
const SPELL: &str = "TST-MULTI-DAMAGE-SPELL";

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
        supported_rules: &["multi-instruction-damage-replacement-red"],
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
#[allow(clippy::too_many_lines)] // The replacement state and full stack trace are one regression.
fn a_middle_damage_instruction_opens_the_affected_players_replacement_decision() {
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
                SPELL,
                BTreeSet::from([CardType::Instant]),
                vec![
                    Effect::GainLifeController { amount: 1 },
                    Effect::DealDamage {
                        amount: 2,
                        target: TargetRequirement::Creature,
                    },
                    Effect::GainLifeController { amount: 2 },
                ],
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
    let spell = game
        .add_card(caster, SPELL, Zone::Hand)
        .expect("multi-instruction spell starts in hand");
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
    .expect("damage shield spell casts");
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
    .expect("damage redirection ability activates");
    pass_pair(&mut game);

    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("multi-instruction damage spell casts");
    pass_pair(&mut game);

    eprintln!(
        "multi-instruction damage replacement red trace: stack={:?}; pending={:?}; events={:?}",
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
        .expect("the middle damage instruction must open a replacement decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(decision.replacement_candidates.len(), 2);
    assert_eq!(game.stack.len(), 1, "the resolving spell remains live");
    assert_eq!(game.player(caster).expect("caster exists").life, 21);
    game.validate_invariants()
        .expect("the paused replacement boundary is state-machine valid");
}
