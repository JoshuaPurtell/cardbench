//! Full-game diagnostic authority for RAV Stasis Cell.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, Color, ContinuousChange, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-STASIS-CELL";
const CARD_NAME: &str = "Stasis Cell";
const GUILDMAGE_ID: &str = "RAV-DIMIR-GUILDMAGE";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const REATTACH_ID: &str = "reattach-to-target-creature";
const DRAW_ID: &str = "target-player-draw";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/stasis_cell.rs";
const MAX_TRACE_SEED: u64 = 32;
const MAX_TURNS: u32 = 64;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/stasis_cell.rs"]
mod candidate_stasis_cell;

const PINNED_CANDIDATE_SOURCE: &str = r##"//! Candidate-owned RAV Stasis Cell definition and Aura bindings.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding, AttachmentKind, CardDefinition,
    CardType, Color, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-STASIS-CELL",
        name: "Stasis Cell",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "enchanted-creature-cannot-attack-or-block",
            "pay-three-generic-blue-to-reattach-to-target-creature",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: attachment_changes(),
        }],
    }
}

/// Pay {3}{U} to attach Stasis Cell to a target creature.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-STASIS-CELL",
        ability: ActivatedAbility {
            id: "reattach-to-target-creature",
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: attachment_changes(),
            }],
        },
    }
}

/// Stasis Cell only prevents its enchanted creature from attacking or blocking.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-STASIS-CELL",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: attachment_changes(),
        granted_activated_abilities: vec![],
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)]
}
"##;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Stasis Cell PvP result: {error}");
                    return ExitCode::FAILURE;
                }
            }
            println!(
                "{}",
                serde_json::to_string(&report).expect("report serializes")
            );
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Stasis Cell PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    let source_guard = validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Stasis Cell full-game replay differed".to_owned());
    }
    let mutations = json!({
        "attack_block_restriction_removed_detected_in_game": mutation_detected(seed, Mutation::NoRestriction),
        "activated_ability_suppression_added_detected_in_game": mutation_detected(seed, Mutation::SuppressAbilities),
        "blue_reattachment_cost_removed_detected_in_game": mutation_detected(seed, Mutation::WrongColor),
        "reattachment_effect_removed_detected_in_game": mutation_detected(seed, Mutation::NoReattachment),
        "sealed_mutation_authority": false
    });
    for key in [
        "attack_block_restriction_removed_detected_in_game",
        "activated_ability_suppression_added_detected_in_game",
        "blue_reattachment_cost_removed_detected_in_game",
        "reattachment_effect_removed_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Stasis Cell gameplay mutation escaped the trace oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":66,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["303.4 Auras","601.2 Casting spells","602.2 Activating abilities","608.2 Resolving spells and abilities","613 Continuous effects","508 Declare Attackers","509 Declare Blockers"],
            "source_contract":"RAV printed Stasis Cell #66 is a blue {4}{U} Aura; its enchanted creature cannot attack or block; pay {3}{U} to attach it to target creature. Other activated abilities remain available.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "deterministic_replay":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "candidate_source_validation":source_guard,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoRestriction,
    SuppressAbilities,
    WrongColor,
    NoReattachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Observation {
    phase: &'static str,
    object: ObjectId,
    definition: String,
    can_attack: bool,
    can_block: bool,
    has_attack_block_restriction: bool,
    nonmana_activated_abilities_suppressed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    cell: Option<ObjectId>,
    first_target: Option<ObjectId>,
    second_target: Option<ObjectId>,
    final_attachment: Option<ObjectId>,
    cell_zone: Option<Zone>,
    first_target_zone: Option<Zone>,
    second_target_zone: Option<Zone>,
    cast_payment: Vec<Color>,
    reattach_cost: Option<ManaCost>,
    reattachment_resolved: bool,
    guildmage_ability_resolved_while_attached: bool,
    observations: Vec<Observation>,
    first_target_restored: bool,
    second_target_restricted: bool,
    object_incarnations: Vec<(u64, u64, u64)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_STASIS_CELL_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_STASIS_CELL_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last = "no seed attempted".to_owned();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no Stasis Cell PvP trace found in bounded seeds: {last}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition_index = definitions
        .iter()
        .position(|definition| definition.id == CARD_ID)
        .ok_or("Stasis Cell definition is missing from the RAV registry")?;
    definitions[definition_index] = candidate_stasis_cell::definition();
    if mutation == Mutation::NoRestriction {
        strip_attack_block_restriction(&mut definitions[definition_index]);
    } else if mutation == Mutation::SuppressAbilities {
        add_activation_suppression(&mut definitions[definition_index]);
    }

    let mut abilities = rav_activated_ability_bindings();
    let ability_index = abilities
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Stasis Cell reattachment ability is missing from the RAV registry")?;
    abilities[ability_index] = candidate_stasis_cell::activated_ability();
    match mutation {
        Mutation::WrongColor => {
            abilities[ability_index].ability.mana_cost = ManaCost::new(3);
        }
        Mutation::NoReattachment => abilities[ability_index].ability.effects.clear(),
        _ => {}
    }

    let mut attachments = rav_attachment_bindings();
    let attachment_index = attachments
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Stasis Cell Aura binding is missing from the RAV registry")?;
    attachments[attachment_index] = candidate_stasis_cell::attachment_binding();
    match mutation {
        Mutation::NoRestriction => attachments[attachment_index].changes.retain(|change| {
            !matches!(
                change,
                ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)
            )
        }),
        Mutation::SuppressAbilities => attachments[attachment_index]
            .changes
            .push(ContinuousChange::SuppressNonManaActivatedAbilities),
        _ => {}
    }
    if matches!(
        mutation,
        Mutation::NoRestriction | Mutation::SuppressAbilities
    ) {
        update_attachment_effect(&mut abilities[ability_index], mutation);
    }

    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(attachments)
        .map_err(|error| error.to_string())?;
    let (deck0, deck1) = decks();
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(StasisPolicy::new(PlayerId(0))),
        Box::new(StasisPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut observations = Vec::new();
    while !game.is_game_over() {
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if game.turn > MAX_TURNS || accepted_moves >= MAX_MOVES {
            return Err(format!(
                "Stasis Cell trace exceeded run bound: turn={} accepted={} player={:?} step={:?} hand={:?} own={:?} opponent={:?}",
                game.turn,
                accepted_moves,
                player,
                view.step,
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.own_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.opponent_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
            ));
        }
        capture_declaration_observation(&game, &view, &mut observations)?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else if let Some(action) = special_action(&game, &view, mutation)? {
            action
        } else {
            policy.propose_move(&view)
        };
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Stasis Cell policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
    }

    let cell = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let first_target = cell.and_then(|cell| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::AuraAttached { aura, target } if *aura == cell => Some(*target),
            _ => None,
        })
    });
    let second_target = first_target.and_then(|first| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::AbilityResolved {
                source,
                ability: REATTACH_ID,
                ..
            } if Some(*source) == cell => game.object(*source).ok().and_then(|object| {
                (object.attached_to != Some(first))
                    .then_some(object.attached_to)
                    .flatten()
            }),
            _ => None,
        })
    });
    let final_attachment = cell.and_then(|object| {
        game.object(object)
            .ok()
            .and_then(|object| object.attached_to)
    });
    let cast_payment = cell.map_or_else(Vec::new, |card| {
        game.event_log
            .iter()
            .find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card: paid,
                    colors,
                } if *paid == card => Some(colors.clone()),
                _ => None,
            })
            .unwrap_or_default()
    });
    let reattach_cost = cell.and_then(|cell| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: REATTACH_ID,
                mana_cost,
            } if *source == cell => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let guildmage_ability_resolved_while_attached = game.event_log.iter().any(|event| {
        matches!(event, GameEvent::AbilityResolved { source, ability: DRAW_ID, .. }
            if game.card_definition(*source).is_ok_and(|definition| definition.id == GUILDMAGE_ID)
                && game.event_log.iter().any(|attached| matches!(attached,
                    GameEvent::AuraAttached { aura, target }
                    if *target == *source
                        && game.card_definition(*aura).is_ok_and(|definition| definition.id == CARD_ID))))
    });
    let first_target_restored = first_target.is_some_and(|target| {
        game.zone_of(target) == Some(Zone::Battlefield)
            && game.characteristics(target).is_ok_and(|characteristics| {
                !characteristics
                    .keywords
                    .contains(&Keyword::CannotAttackOrBlock)
            })
    });
    let second_target_restricted = final_attachment.is_some_and(|target| {
        game.characteristics(target).is_ok_and(|characteristics| {
            characteristics
                .keywords
                .contains(&Keyword::CannotAttackOrBlock)
        })
    });
    let incarnation = |object: Option<ObjectId>| {
        object.and_then(|object| game.object(object).ok().map(|state| state.incarnation))
    };
    let object_incarnations = match (
        incarnation(cell),
        incarnation(first_target),
        incarnation(second_target),
    ) {
        (Some(cell), Some(first), Some(second)) => vec![(cell, first, second)],
        _ => vec![],
    };
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        cell,
        first_target,
        second_target,
        final_attachment,
        cell_zone: cell.and_then(|object| game.zone_of(object)),
        first_target_zone: first_target.and_then(|object| game.zone_of(object)),
        second_target_zone: second_target.and_then(|object| game.zone_of(object)),
        cast_payment,
        reattach_cost,
        reattachment_resolved: cell.is_some_and(|cell| {
            game.event_log.iter().any(|event| {
                matches!(event,
            GameEvent::AbilityResolved { source, ability: REATTACH_ID, .. } if *source == cell)
            })
        }),
        guildmage_ability_resolved_while_attached,
        observations,
        first_target_restored,
        second_target_restricted,
        object_incarnations,
    })
}

fn special_action(
    game: &Game,
    view: &GameView,
    mutation: Mutation,
) -> Result<Option<PolicyAction>, String> {
    let cell = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let first_target = cell.and_then(|cell| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::AuraAttached { aura, target } if *aura == cell => Some(*target),
            _ => None,
        })
    });
    let has_draw_activation = game.event_log.iter().any(|event| {
        matches!(event,
        GameEvent::AbilityActivated { source, ability: DRAW_ID, .. }
            if game.card_definition(*source).is_ok_and(|definition| definition.id == GUILDMAGE_ID))
    });
    let has_reattachment = cell.is_some_and(|cell| {
        game.event_log.iter().any(|event| {
            matches!(event,
        GameEvent::AbilityResolved { source, ability: REATTACH_ID, .. } if *source == cell)
        })
    });

    if view.player == PlayerId(1)
        && view.priority == PlayerId(1)
        && view.active_player == PlayerId(1)
        && view.step.is_main()
        && view.stack_depth == 0
        && !has_draw_activation
        && first_target.is_some_and(|target| {
            game.card_definition(target)
                .is_ok_and(|definition| definition.id == GUILDMAGE_ID)
        })
    {
        let cost = ManaCost::with_colors(3, [Color::Blue]);
        if payment(view, &cost, &[]).is_some() {
            let source = first_target.ok_or("Stasis Cell first target disappeared")?;
            return Ok(Some(PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source,
                    ability_id: DRAW_ID,
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![Target::Player(PlayerId(0))],
                },
            }));
        }
        if let Some(action) = make_mana(view, &cost, &[]) {
            return Ok(Some(action));
        }
    }

    if view.player == PlayerId(0)
        && view.priority == PlayerId(0)
        && view.active_player == PlayerId(0)
        && view.step.is_main()
        && view.stack_depth == 0
        && first_target.is_some()
        && !has_reattachment
    {
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID));
        if let Some(target) = target {
            let cost = candidate_stasis_cell::activated_ability().ability.mana_cost;
            if payment(view, &cost, &[]).is_some() {
                let source = cell.ok_or("Stasis Cell source disappeared before reattachment")?;
                return Ok(Some(PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source,
                        ability_id: REATTACH_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                }));
            }
            if let Some(action) = make_mana(view, &cost, &[]) {
                return Ok(Some(action));
            }
        }
    }
    let _ = mutation;
    Ok(None)
}

fn capture_declaration_observation(
    game: &Game,
    view: &GameView,
    output: &mut Vec<Observation>,
) -> Result<(), String> {
    let phase = match view.step {
        Step::DeclareAttackers if view.active_player == PlayerId(1) => "declare-attackers",
        Step::DeclareBlockers if view.decision_player == PlayerId(1) => "declare-blockers",
        _ => return Ok(()),
    };
    let cell = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let Some(cell) = cell else { return Ok(()) };
    let attached = game
        .object(cell)
        .map_err(|error| error.to_string())?
        .attached_to;
    for card in view
        .own_battlefield
        .iter()
        .chain(&view.opponent_battlefield)
    {
        if Some(card.id) == attached && card.controller == PlayerId(1) {
            let observation = Observation {
                phase,
                object: card.id,
                definition: card.definition.unwrap_or("unknown").to_owned(),
                can_attack: card.can_attack,
                can_block: card.can_block,
                has_attack_block_restriction: card.keywords.contains(&Keyword::CannotAttackOrBlock),
                nonmana_activated_abilities_suppressed: card.nonmana_activated_abilities_suppressed,
            };
            if !output.contains(&observation) {
                output.push(observation);
            }
        }
    }
    Ok(())
}

struct StasisPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    cell_cast: bool,
    guildmage_cast: bool,
    watchwolf_cast: bool,
}

impl StasisPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            cell_cast: false,
            guildmage_cast: false,
            watchwolf_cast: false,
        }
    }

    fn cast_action(
        &mut self,
        view: &GameView,
        definition: &str,
        cost: &ManaCost,
        target: Option<ObjectId>,
        hybrid: &[Color],
    ) -> Option<PolicyAction> {
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))
        {
            if let Some(mana_selection) = payment(view, cost, hybrid) {
                if definition == CARD_ID {
                    self.cell_cast = true;
                }
                if definition == GUILDMAGE_ID {
                    self.guildmage_cast = true;
                }
                if definition == WATCHWOLF_ID {
                    self.watchwolf_cast = true;
                }
                return Some(PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: target
                            .map_or_else(Vec::new, |target| vec![Target::Permanent(target)]),
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                });
            }
            if let Some(action) = make_mana(view, cost, hybrid) {
                return Some(action);
            }
        }
        None
    }
}

impl CodePolicy for StasisPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.stasis-cell-pilot.v1"
        } else {
            "rav.stasis-cell-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(1) || !self.cell_cast {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.active_player != self.player
            || view.priority != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: card.id };
            }
        }
        if self.player == PlayerId(1) && !self.guildmage_cast {
            if let Some(cost) = card_definitions()
                .into_iter()
                .find(|definition| definition.id == GUILDMAGE_ID)
                .map(|definition| definition.mana_cost)
            {
                if let Some(action) =
                    self.cast_action(view, GUILDMAGE_ID, &cost, None, &[Color::Blue, Color::Blue])
                {
                    return action;
                }
            }
        }
        if self.player == PlayerId(1) && !self.watchwolf_cast {
            let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
            if let Some(action) = self.cast_action(view, WATCHWOLF_ID, &cost, None, &[]) {
                return action;
            }
        }
        if self.player == PlayerId(0) && !self.cell_cast {
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(GUILDMAGE_ID));
            if let Some(target) = target {
                let cost = candidate_stasis_cell::definition().mana_cost;
                if let Some(action) = self.cast_action(view, CARD_ID, &cost, Some(target.id), &[]) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(0) {
            for creature in [WATCHWOLF_ID, "RAV-TATTERED-DRAKE"] {
                let already_cast = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(creature));
                if !already_cast {
                    let cost = card_definitions()
                        .into_iter()
                        .find(|definition| definition.id == creature)
                        .map(|definition| definition.mana_cost);
                    if let Some(cost) = cost
                        && let Some(action) = self.cast_action(view, creature, &cost, None, &[])
                    {
                        return action;
                    }
                }
            }
        }
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn decks() -> (DeckList, DeckList) {
    let p0 = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry(WATCHWOLF_ID, 4),
            entry("RAV-TATTERED-DRAKE", 4),
            entry("RAV-ISLAND", 16),
            entry("RAV-FOREST", 16),
            entry("RAV-PLAINS", 16),
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            entry(GUILDMAGE_ID, 4),
            entry(WATCHWOLF_ID, 4),
            entry("RAV-TATTERED-DRAKE", 4),
            entry("RAV-ISLAND", 16),
            entry("RAV-FOREST", 16),
            entry("RAV-PLAINS", 16),
        ],
        sideboard: vec![],
    };
    (p0, p1)
}

fn entry(card: &str, count: u8) -> DeckEntry {
    DeckEntry {
        card: card.to_owned(),
        count,
    }
}

fn payment(view: &GameView, cost: &ManaCost, hybrid: &[Color]) -> Option<ManaPaymentSelection> {
    if hybrid.len() != cost.hybrid.len() {
        return None;
    }
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = &mut available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?
            .1;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    for (symbol, selected) in cost.hybrid.iter().zip(hybrid) {
        if *selected != symbol.first && *selected != symbol.second {
            return None;
        }
        let amount = &mut available
            .iter_mut()
            .find(|(candidate, _)| candidate == selected)?
            .1;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        // Preserve blue for the required {U} symbol whenever another color can
        // pay a generic point. This makes the cast receipt show the colored
        // requirement directly instead of consuming spare blue as generic.
        let index = available
            .iter()
            .position(|(color, amount)| *color != Color::Blue && *amount > 0)
            .or_else(|| available.iter().position(|(_, amount)| *amount > 0))?;
        let (color, amount) = &mut available[index];
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: hybrid.to_vec(),
    })
}

fn make_mana(view: &GameView, cost: &ManaCost, hybrid: &[Color]) -> Option<PolicyAction> {
    let mut required = cost.colored.clone();
    required.extend_from_slice(hybrid);
    if let Some(color) = required.iter().find(|color| {
        view.mana_pool.amount(**color)
            < required.iter().filter(|other| *other == *color).count() as u8
    }) {
        return land_for(view, Some(*color));
    }
    if view.mana_pool.total() < cost.generic + required.len() as u8 {
        return land_for(view, None);
    }
    None
}

fn land_for(view: &GameView, desired: Option<Color>) -> Option<PolicyAction> {
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && desired.is_none_or(|color| card.mana_colors.contains(&color))
        })
        .or_else(|| {
            view.own_battlefield
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
        })?;
    let color = desired
        .filter(|color| land.mana_colors.contains(color))
        .or_else(|| land.mana_colors.iter().next().copied())?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn strip_attack_block_restriction(definition: &mut CardDefinition) {
    if let Some(Effect::AttachSourceToTarget { changes, .. }) = definition.effects.first_mut() {
        changes.retain(|change| {
            !matches!(
                change,
                ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)
            )
        });
    }
}

fn add_activation_suppression(definition: &mut CardDefinition) {
    if let Some(Effect::AttachSourceToTarget { changes, .. }) = definition.effects.first_mut() {
        changes.push(ContinuousChange::SuppressNonManaActivatedAbilities);
    }
}

fn update_attachment_effect(
    binding: &mut cardbench_magic_engine::ActivatedAbilityBinding,
    mutation: Mutation,
) {
    if let Some(Effect::AttachSourceToTarget { changes, .. }) = binding.ability.effects.first_mut()
    {
        match mutation {
            Mutation::NoRestriction => changes.retain(|change| {
                !matches!(
                    change,
                    ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)
                )
            }),
            Mutation::SuppressAbilities => {
                changes.push(ContinuousChange::SuppressNonManaActivatedAbilities)
            }
            _ => {}
        }
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.accepted_moves == 0
        || capture.rejected_moves != 0
    {
        return Err("Stasis Cell duel did not reach a clean terminal pilot win".to_owned());
    }
    let cell = capture.cell.ok_or("Stasis Cell was not cast")?;
    let first = capture
        .first_target
        .ok_or("Stasis Cell did not attach on entry")?;
    let second = capture
        .second_target
        .ok_or("Stasis Cell did not resolve a retargeting activation")?;
    if capture.cell_zone != Some(Zone::Battlefield)
        || capture.final_attachment != Some(second)
        || capture.first_target_zone != Some(Zone::Battlefield)
        || capture.second_target_zone != Some(Zone::Battlefield)
        || first == second
        || capture.cast_payment.len() != 5
        || !capture.cast_payment.contains(&Color::Blue)
        || capture.reattach_cost.as_ref() != Some(&ManaCost::with_colors(3, [Color::Blue]))
        || !capture.reattachment_resolved
        || !capture.guildmage_ability_resolved_while_attached
        || !capture.first_target_restored
        || !capture.second_target_restricted
        || capture.object_incarnations.len() != 1
    {
        return Err("Stasis Cell payment, Aura attachment, reattachment, or incarnation receipt differs from RAV #66".to_owned());
    }
    let attacker_observation = capture.observations.iter().find(|observation| {
        observation.object == first && observation.phase == "declare-attackers"
    });
    let blocker_observation = capture.observations.iter().find(|observation| {
        observation.object == second && observation.phase == "declare-blockers"
    });
    if !attacker_observation.is_some_and(|observation| {
        observation.definition == GUILDMAGE_ID
            && !observation.can_attack
            && observation.has_attack_block_restriction
            && !observation.nonmana_activated_abilities_suppressed
    }) || !blocker_observation.is_some_and(|observation| {
        observation.definition == WATCHWOLF_ID
            && !observation.can_block
            && observation.has_attack_block_restriction
            && !observation.nonmana_activated_abilities_suppressed
    }) {
        return Err(
            "Stasis Cell did not restrict both combat declarations without suppressing abilities"
                .to_owned(),
        );
    }
    let cast = position(
        &capture.events,
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == cell),
    )?;
    let attached = position_after(
        &capture.events,
        cast,
        |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == cell && *target == first),
    )?;
    let mage_activation = position_after(
        &capture.events,
        attached,
        |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(1), source, ability: DRAW_ID, .. } if *source == first),
    )?;
    let reattach_payment = position_after(
        &capture.events,
        attached,
        |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: REATTACH_ID, mana_cost } if *source == cell && *mana_cost == ManaCost::with_colors(3,[Color::Blue])),
    )?;
    let reattach_resolution = position_after(
        &capture.events,
        reattach_payment,
        |event| matches!(event, GameEvent::AbilityResolved { source, ability: REATTACH_ID, .. } if *source == cell),
    )?;
    let terminal = position_after(&capture.events, reattach_resolution, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })?;
    if !(cast < attached
        && attached < mage_activation
        && attached < reattach_payment
        && reattach_payment < reattach_resolution
        && reattach_resolution < terminal)
    {
        return Err("Stasis Cell event ordering was invalid".to_owned());
    }
    let projected = project_events(&capture.events);
    let event_kinds: Vec<String> = projected
        .iter()
        .map(|event| format!("{:?}", event.kind))
        .collect();
    Ok(json!({
        "id":"rav-stasis-cell-colored-reattachment-combat-restriction-full-game",
        "seed":seed,
        "passed":true,
        "terminal":capture.terminal,
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted_moves,
        "rejected_moves":capture.rejected_moves,
        "mechanics":{
            "definition_id":CARD_ID,
            "mana_cost":"{4}{U}",
            "paid_mana_colors":capture.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "aura_object":cell.0,
            "first_target_object":first.0,
            "first_target_definition":GUILDMAGE_ID,
            "second_target_object":second.0,
            "second_target_definition":WATCHWOLF_ID,
            "final_attachment_object":capture.final_attachment.map(|object|object.0),
            "attachment_incarnations":capture.object_incarnations,
            "first_target_restored_after_reattachment":capture.first_target_restored,
            "second_target_cannot_attack_or_block":capture.second_target_restricted,
            "reattachment_cost":"{3}{U}",
            "reattachment_mana_cost_receipt":format!("{:?}",capture.reattach_cost),
            "reattachment_resolved":capture.reattachment_resolved,
            "nonmana_ability_resolved_while_attached":capture.guildmage_ability_resolved_while_attached,
            "declaration_observations":capture.observations,
            "event_order_indices":{"cell_cast":cast,"entry_attachment":attached,"guildmage_activation":mage_activation,"reattachment_paid":reattach_payment,"reattachment_resolved":reattach_resolution,"game_end":terminal},
            "projected_event_kinds":event_kinds,
            "terminal_pilot_win":capture.winner==Some(0)
        },
        "ordered_event_sequence":[cast,attached,mage_activation,reattach_payment,reattach_resolution,terminal],
        "events":projected
    }))
}

fn position(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Result<usize, String> {
    events
        .iter()
        .position(predicate)
        .ok_or_else(|| "required Stasis Cell event is absent".to_owned())
}

fn position_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Result<usize, String> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
        .ok_or_else(|| "ordered Stasis Cell event is absent".to_owned())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_stasis_cell::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Stasis Cell definition missing from RAV registry")?;
    if definition != registered
        || definition.mana_cost != ManaCost::with_colors(4, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)],
            }]
    {
        return Err(
            "Stasis Cell candidate definition differs from RAV printed identity".to_owned(),
        );
    }
    let ability = candidate_stasis_cell::activated_ability();
    if !rav_activated_ability_bindings().contains(&ability)
        || ability.ability.id != REATTACH_ID
        || ability.ability.mana_cost != ManaCost::with_colors(3, [Color::Blue])
        || ability.ability.sorcery_speed
        || ability.ability.targets != [TargetRequirement::Creature]
        || ability.ability.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)],
            }]
    {
        return Err(
            "Stasis Cell reattachment binding differs from printed {3}{U} effect".to_owned(),
        );
    }
    let attachment = candidate_stasis_cell::attachment_binding();
    if !rav_attachment_bindings().contains(&attachment)
        || attachment.kind != cardbench_magic_engine::AttachmentKind::Aura
        || attachment.target != TargetRequirement::Creature
        || attachment.changes != [ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)]
    {
        return Err(
            "Stasis Cell Aura attachment differs from printed combat restriction".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Stasis Cell candidate: {error}"))?;
    validate_candidate_source_text(&source)?;
    let sentinel = std::env::temp_dir().join(format!(
        "cardbench-stasis-cell-side-effect-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sentinel);
    let injection = format!(
        "pub(crate) fn definition() -> CardDefinition {{\n    let _ = std::fs::write({:?}, b\"executed\");",
        sentinel.to_string_lossy()
    );
    let malicious = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        &injection,
        1,
    );
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Stasis Cell side-effect candidate could not be constructed".to_owned(),
        );
    }
    let rejected = validate_candidate_source_text(&malicious).is_err();
    let sentinel_absent = !sentinel.exists();
    if !rejected || !sentinel_absent {
        return Err("Stasis Cell source gate failed to reject side-effect injection before candidate evaluation".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls":true,
        "compile_valid_injected_side_effect_fixture_parsed":true,
        "compile_valid_injected_side_effect_candidate_rejected":true,
        "injected_candidate_calls_before_rejection":0,
        "side_effect_sentinel_absent_after_rejection":sentinel_absent
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Stasis Cell source is not valid Rust".to_owned())?;
    if source != PINNED_CANDIDATE_SOURCE
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 6
    {
        return Err("Stasis Cell source differs from the pinned literal module".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if function.sig.ident == "attachment_changes"
                    && function.attrs.is_empty()
                    && matches!(&function.vis, Visibility::Inherited)
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none() =>
            {
                if !return_type_is_vec(&function.sig.output, "ContinuousChange")
                    || !matches!(
                        function.block.stmts.as_slice(),
                        [Stmt::Expr(Expr::Vec(_), None)]
                    )
                    || !functions.insert("attachment_changes".to_owned())
                {
                    return Err(
                        "Stasis Cell source helper AST differs from the task contract".to_owned(),
                    );
                }
            }
            Item::Fn(function)
                if function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(&function.vis, Visibility::Restricted(_))
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none() =>
            {
                let (name, ty) = match function.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "activated_ability" => ("activated_ability", "ActivatedAbilityBinding"),
                    "attachment_binding" => ("attachment_binding", "AttachmentBinding"),
                    _ => {
                        return Err("Stasis Cell source contains an unexpected function".to_owned());
                    }
                };
                if !return_type_is(&function.sig.output, ty)
                    || !matches!(function.block.stmts.as_slice(),[Stmt::Expr(Expr::Struct(literal),None)] if path_is_type(&literal.path,ty) && literal.rest.is_none())
                    || !functions.insert(name.to_owned())
                {
                    return Err("Stasis Cell source AST differs from the task contract".to_owned());
                }
            }
            _ => return Err("Stasis Cell source contains an executable top-level item".to_owned()),
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "activated_ability".to_owned(),
            "attachment_binding".to_owned(),
            "attachment_changes".to_owned(),
        ])
    {
        return Err("Stasis Cell source AST has missing required functions".to_owned());
    }
    Ok(())
}

fn return_type_is(output: &ReturnType, expected: &str) -> bool {
    matches!(output,ReturnType::Type(_,ty) if matches!(&**ty,syn::Type::Path(path)if path.qself.is_none()&&path_is_type(&path.path,expected)))
}

fn return_type_is_vec(output: &ReturnType, expected: &str) -> bool {
    matches!(output, ReturnType::Type(_, ty)
        if matches!(&**ty, syn::Type::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == "Vec"
                && matches!(&path.path.segments[0].arguments,
                    syn::PathArguments::AngleBracketed(arguments)
                    if arguments.args.len() == 1
                        && matches!(arguments.args.first(), Some(syn::GenericArgument::Type(syn::Type::Path(inner)))
                            if inner.qself.is_none() && path_is_type(&inner.path, expected)))))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect::<Vec<_>>();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from source contract".to_owned());
    }
    Ok(cards)
}

fn card_status(card: &CatalogCard) -> Value {
    let status = card.semantic_status;
    let executable = status.is_executable();
    json!({"name":card.name,"collector_number":card.collector_number,"semantic_status":format!("{status:?}"),"trace_status":if card.name==CARD_NAME{"passed"}else if executable{"not_traced_in_this_single_card_task"}else{"not_full_rules_fidelity"},"passed":card.name==CARD_NAME})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate_definition_and_aura_bindings"
    } else {
        "reference_definition_and_aura_bindings"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::var_os("CARDBENCH_TASK_OUTPUT_DIR").map(Into::into)
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}
