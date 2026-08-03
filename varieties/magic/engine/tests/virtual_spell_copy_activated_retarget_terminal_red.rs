//! Red regression: a virtual copied spell that retargets an activated ability
//! must complete its decision with a virtual terminal receipt.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, DecisionKind, DecisionSelection, Effect, Game, GameEvent, ManaCost, ObjectId,
    PlayerId, Target, TargetRequirement, Zone,
};

const PINGER: &str = "TST-VIRTUAL-COPY-ACTIVATED-RETARGET-PINGER";
const REROUTE: &str = "TST-VIRTUAL-COPY-ACTIVATED-RETARGET-REROUTE";
const COPY: &str = "TST-VIRTUAL-COPY-ACTIVATED-RETARGET-COPY";
const DRAW: &str = "TST-VIRTUAL-COPY-ACTIVATED-RETARGET-DRAW";
const PING_ABILITY: &str = "ping-player";

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
        supported_rules: &["virtual-spell-copy-activated-retarget-terminal-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
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

#[test]
#[allow(clippy::too_many_lines)] // One nested spell/ability transcript proves exact virtual stack lifecycle.
fn virtual_copy_can_complete_activated_ability_retarget_decision() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            definition(PINGER, CardType::Creature, vec![]),
            definition(
                REROUTE,
                CardType::Instant,
                vec![
                    Effect::ChangeTargetOfTargetActivatedAbility,
                    Effect::DrawController,
                ],
            ),
            definition(
                COPY,
                CardType::Instant,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(DRAW, CardType::Artifact, vec![]),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: PINGER,
            ability: ActivatedAbility {
                id: PING_ABILITY,
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    let pinger = game
        .put_on_battlefield(copy_controller, PINGER)
        .expect("pinger begins on copy controller battlefield");
    let reroute = game
        .add_card(caster, REROUTE, Zone::Hand)
        .expect("physical retarget spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let drawn = game
        .add_card(copy_controller, DRAW, Zone::Library)
        .expect("copy controller has a draw card");
    game.begin_game().expect("game begins");

    game.pass_priority(caster)
        .expect("caster passes to ability controller");
    game.activate_ability(
        copy_controller,
        AbilityActivation {
            source: pinger,
            ability_id: PING_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(caster)],
        },
    )
    .expect("one-target activated ability stacks");
    let activated = game.stack.last().expect("ability stacked").id;
    game.pass_priority(copy_controller)
        .expect("ability controller passes to reroute caster");
    game.cast_spell(
        caster,
        request(reroute, vec![Target::ActivatedAbility(activated)]),
    )
    .expect("physical retarget spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(reroute)]))
        .expect("copy spell targets physical retarget spell");
    resolve_top(&mut game).expect("copy instruction makes virtual retarget spell");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == reroute => Some(*copy),
            _ => None,
        })
        .expect("virtual retarget spell receipt exists");
    resolve_top(&mut game).expect("virtual retarget spell opens public decision");
    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller view")
        .pending_decision
        .expect("virtual retarget spell opens a decision");
    assert_eq!(decision.kind, DecisionKind::RetargetActivatedAbility);
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Targets(vec![Target::Player(copy_controller)]),
    );
    eprintln!(
        "virtual-copy activated-retarget red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "virtual retarget spell must terminate after the chosen activated-ability target change"
    );
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == reroute
    )));
    game.validate_invariants()
        .expect("virtual activated-retarget terminal lifecycle remains auditable");
}
