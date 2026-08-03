//! Red regression: a counter-unless payment spell must target a virtual spell
//! copy through stack provenance, not a physical card lookup.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, Target, TargetRequirement, Zone,
};

const PING: &str = "TST-VIRTUAL-COUNTER-UNLESS-PING";
const COPY: &str = "TST-VIRTUAL-COUNTER-UNLESS-COPY";
const COUNTER: &str = "TST-VIRTUAL-COUNTER-UNLESS";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-counter-unless-red"],
        power: None,
        toughness: None,
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

fn game_with_counter_unless_at_virtual_copy() -> (Game, ObjectId, ObjectId, PlayerId) {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                COUNTER,
                vec![Effect::CounterTargetSpellUnlessControllerPays {
                    mana_cost: ManaCost::new(1),
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let ping = game
        .add_card(caster, PING, Zone::Hand)
        .expect("ping enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy effect enters hand");
    let counter = game
        .add_card(caster, COUNTER, Zone::Hand)
        .expect("counter enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(ping, vec![Target::Player(copy_controller)]))
        .expect("ping casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(ping)]))
        .expect("copy effect casts");
    resolve_top(&mut game).expect("copy effect resolves");
    let virtual_copy = game
        .event_log
        .iter()
        .rev()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == ping => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual spell");

    game.cast_spell(caster, request(counter, vec![Target::Spell(virtual_copy)]))
        .expect("counter-unless may target a virtual spell copy");
    let result = resolve_top(&mut game);
    eprintln!(
        "virtual counter-unless red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "counter-unless must use the virtual target's stack incarnation rather than UnknownCard"
    );
    (game, counter, virtual_copy, copy_controller)
}

#[test]
fn counter_unless_pays_opens_its_choice_for_a_virtual_copy_controller() {
    let (game, _counter, _virtual_copy, copy_controller) =
        game_with_counter_unless_at_virtual_copy();
    let decision = game
        .view_for_player(copy_controller)
        .expect("virtual copy controller view")
        .pending_decision
        .expect("virtual copy controller receives counter-unless choice");
    assert_eq!(decision.kind, DecisionKind::CounterUnlessPaysMana);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 0);
    game.validate_invariants()
        .expect("virtual target counter-unless decision is auditable");
}

#[test]
fn counter_unless_decline_counters_a_virtual_copy_without_zone_move() {
    let (mut game, counter, virtual_copy, copy_controller) =
        game_with_counter_unless_at_virtual_copy();
    let decision = game
        .view_for_player(copy_controller)
        .expect("virtual copy controller view")
        .pending_decision
        .expect("virtual copy controller receives counter-unless choice");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::CounterUnlessPaysMana {
            pay: false,
            mana_abilities: vec![],
            mana_selection: ManaPaymentSelection::default(),
        },
    );
    eprintln!(
        "virtual counter-unless terminal red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "declining payment must counter the virtual copy through its stack provenance"
    );
    assert_eq!(game.zone_of(virtual_copy), None, "copy remains zoneless");
    assert_eq!(game.zone_of(counter), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyCountered { copy, source, .. }
            if *copy == virtual_copy && *source == counter
    )));
    game.validate_invariants()
        .expect("counter-unless virtual-copy terminal lifecycle remains auditable");
}
