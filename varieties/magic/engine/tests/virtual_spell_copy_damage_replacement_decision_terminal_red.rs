//! Red regression: a virtual copied damage spell must not bypass a genuinely
//! competing affected-player shield-choice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DamageReplacementChoice, DecisionKind,
    DecisionSelection, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId, ReplacementChoice,
    Target, TargetRequirement, Zone,
};

const TARGET: &str = "TST-VIRTUAL-COPY-DAMAGE-CHOICE-TARGET";
const SHIELD: &str = "TST-VIRTUAL-COPY-DAMAGE-CHOICE-SHIELD";
const BOLT: &str = "TST-VIRTUAL-COPY-DAMAGE-CHOICE-BOLT";
const COPY: &str = "TST-VIRTUAL-COPY-DAMAGE-CHOICE-COPY";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-damage-replacement-decision-terminal-red"],
        power: creature.then_some(4),
        toughness: creature.then_some(4),
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[allow(clippy::too_many_lines)] // The two shield receipts and pending-choice boundary form one atomic contract.
fn game_with_virtual_copy_at_competing_damage_replacement_choice()
-> (Game, ObjectId, ObjectId, ObjectId, PlayerId) {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(TARGET, CardType::Creature, vec![]),
            definition(
                SHIELD,
                CardType::Instant,
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
            definition(
                BOLT,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
            ),
            definition(
                COPY,
                CardType::Instant,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let target = game
        .put_on_battlefield(copy_controller, TARGET)
        .expect("damage target begins on battlefield");
    let first_shield = game
        .add_card(copy_controller, SHIELD, Zone::Hand)
        .expect("first shield enters hand");
    let second_shield = game
        .add_card(copy_controller, SHIELD, Zone::Hand)
        .expect("second shield enters hand");
    let bolt = game
        .add_card(caster, BOLT, Zone::Hand)
        .expect("bolt enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy effect enters hand");
    game.begin_game().expect("game begins");

    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        copy_controller,
        request(first_shield, vec![Target::Permanent(target)]),
    )
    .expect("first shield casts");
    resolve_top(&mut game).expect("first shield resolves");
    game.pass_priority(caster)
        .expect("caster passes to copy controller for second shield");
    game.cast_spell(
        copy_controller,
        request(second_shield, vec![Target::Permanent(target)]),
    )
    .expect("second shield casts");
    resolve_top(&mut game).expect("second shield resolves");

    game.cast_spell(caster, request(bolt, vec![Target::Permanent(target)]))
        .expect("bolt casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(bolt)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual bolt");
    resolve_top(&mut game).expect("virtual bolt opens replacement decision");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == bolt => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual bolt");
    (game, target, bolt, virtual_copy, copy_controller)
}

#[test]
fn virtual_copy_opens_competing_damage_replacement_decision() {
    let (game, _target, bolt, virtual_copy, copy_controller) =
        game_with_virtual_copy_at_competing_damage_replacement_choice();
    let decision = game
        .view_for_player(copy_controller)
        .expect("affected player view")
        .pending_decision;
    eprintln!(
        "virtual-copy competing-shields precondition: decision={decision:?}; events={:?}",
        game.canonical_event_log()
    );
    let decision = decision.expect("two live shields open an affected-player decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(decision.replacement_candidates.len(), 2);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopied { copy, original, .. } if *copy == virtual_copy && *original == bolt
    )));
    game.validate_invariants()
        .expect("an open virtual-copy replacement decision remains auditable");
}

#[test]
fn virtual_copy_completes_damage_replacement_decision_without_zone_move() {
    let (mut game, target, bolt, virtual_copy, copy_controller) =
        game_with_virtual_copy_at_competing_damage_replacement_choice();
    let decision = game
        .view_for_player(copy_controller)
        .expect("affected player view")
        .pending_decision
        .expect("two live shields open an affected-player decision");
    let shield = decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| {
            matches!(
                choice,
                ReplacementChoice::Damage(DamageReplacementChoice::TargetedShield { .. })
            )
        })
        .expect("a shield is a legal replacement choice");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Replacements(vec![shield]),
    );
    eprintln!(
        "virtual-copy damage-replacement terminal red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "virtual copy must end with SpellCopyResolved, never a physical terminal-zone move"
    );
    assert_eq!(game.object(target).expect("target exists").damage, 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == bolt
    )));
    game.validate_invariants()
        .expect("virtual damage-replacement terminal lifecycle remains auditable");
}
