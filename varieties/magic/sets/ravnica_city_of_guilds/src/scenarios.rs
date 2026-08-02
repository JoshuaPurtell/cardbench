//! Public, fixture-driven RAV scenario parser and executor.
//!
//! The format intentionally uses a constrained TOML subset so the reference runner
//! has no parser dependency and each evaluated setup/action/assertion is visible in
//! versioned data rather than hidden in a Rust test body.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, BasicLandManaAbilityActivation, CastPaymentManaAbility,
    CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, DecisionKind,
    DecisionSelection, Effect, Game, GeneralizedAbilityActivation, ManaAbilityActivation,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, RulesError, Target, Zone,
};

use crate::{
    ScenarioResult, card_definitions, event_digest, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_generalized_activated_ability_cost_bindings,
    rav_mana_ability_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings, set_root,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ScenarioSpec {
    id: String,
    seed: u64,
    description: String,
    triggers: bool,
    cards: Vec<CardSetup>,
    mana: Vec<ManaSetup>,
    actions: Vec<ActionSpec>,
    expected: ExpectedState,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CardSetup {
    label: String,
    owner: usize,
    definition: String,
    zone: String,
    tapped: bool,
    entered_turn: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ManaSetup {
    player: usize,
    color: String,
    amount: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ActionSpec {
    kind: String,
    player: usize,
    card: String,
    /// One target occurrence for legacy single-target fixtures. New scenarios
    /// use `targets` so a stack object preserves one slot for every distinct
    /// target word in its executable effect model.
    target: String,
    targets: Vec<String>,
    /// Explicit permanent selections paid as an activated ability's sacrifice
    /// cost. This preserves the policy-visible selection boundary instead of
    /// letting a scenario executor choose a permanent implicitly.
    sacrifice_sources: Vec<String>,
    /// Explicit controlled creature selections paid as an activated ability's
    /// additional tap cost. This preserves the same policy-visible choice
    /// boundary as direct `AbilityActivation` submission.
    additional_tap_creatures: Vec<String>,
    /// Explicit source or controlled-permanent selections paid as generalized
    /// counter-removal costs. This stays separate from spell/ability targets.
    counter_sources: Vec<String>,
    convoke: Vec<String>,
    /// `source_label:ability_id` definition-bound entries or
    /// `source_label:basic-land` typed intrinsic entries activated only while
    /// this cast pays its mana cost. The constrained fixture grammar
    /// deliberately exposes no general activated-ability path here.
    payment_mana: Vec<String>,
    /// Explicit colors chosen for the remaining generic or hybrid symbols in
    /// one cast. Entries use `generic:color` or `hybrid:color`.
    mana_spend: Vec<String>,
    /// One explicit nonnegative X value submitted with a `cast` policy
    /// action. This is intentionally separate from mana-spend entries: the
    /// engine validates the amount, applies it to the spell's generic cost,
    /// and retains it on the stack for resolution-time target legality.
    chosen_x: Option<u8>,
    attackers: Vec<String>,
    /// Explicit controller ordering for a simultaneous triggered-ability
    /// group. Entries use `source_label:ability_id`, keeping the public
    /// policy decision boundary visible in the fixture rather than relying on
    /// source/binding insertion order.
    trigger_order: Vec<String>,
    ability: String,
    color: String,
    pay: bool,
    dredge: String,
    found: String,
    expected_error: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ExpectedState {
    life: Vec<i64>,
    zones: Vec<String>,
    powers: Vec<String>,
    toughnesses: Vec<String>,
    colors: Vec<String>,
    tapped: Vec<String>,
    token_count: Option<usize>,
    mana: Vec<String>,
    mana_receipts: Vec<String>,
    /// Public top-library identities, encoded as `library_owner:card_label`.
    /// Every seated policy view must receive the exact same projection.
    revealed_library_tops: Vec<String>,
    stack_size: Option<usize>,
    priority: Option<usize>,
    event_markers: Vec<String>,
    event_absent: Vec<String>,
    digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Section {
    Root,
    Scenario,
    Card,
    Mana,
    Action,
    Expected,
}

pub(crate) fn run_public_scenarios() -> Result<Vec<ScenarioResult>, String> {
    let path = set_root().join("scenarios/public/train_scenarios.toml");
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let specifications =
        parse_scenarios(&contents).map_err(|error| format!("{}: {error}", path.display()))?;
    specifications
        .iter()
        .map(execute_scenario)
        .collect::<Result<Vec<_>, _>>()
}

fn parse_scenarios(contents: &str) -> Result<Vec<ScenarioSpec>, String> {
    let mut scenarios = Vec::new();
    let mut section = Section::Root;
    for (line_number, raw_line) in contents.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "[[scenario]]" => {
                scenarios.push(ScenarioSpec::default());
                section = Section::Scenario;
                continue;
            }
            "[[scenario.card]]" => {
                scenarios
                    .last_mut()
                    .ok_or_else(|| line_error(line_number, "card table before scenario"))?
                    .cards
                    .push(CardSetup::default());
                section = Section::Card;
                continue;
            }
            "[[scenario.mana]]" => {
                scenarios
                    .last_mut()
                    .ok_or_else(|| line_error(line_number, "mana table before scenario"))?
                    .mana
                    .push(ManaSetup::default());
                section = Section::Mana;
                continue;
            }
            "[[scenario.action]]" => {
                scenarios
                    .last_mut()
                    .ok_or_else(|| line_error(line_number, "action table before scenario"))?
                    .actions
                    .push(ActionSpec::default());
                section = Section::Action;
                continue;
            }
            "[scenario.expected]" => {
                if scenarios.is_empty() {
                    return Err(line_error(line_number, "expected table before scenario"));
                }
                section = Section::Expected;
                continue;
            }
            _ => {}
        }
        let (key, value) = line
            .split_once('=')
            .map(|(key, value)| (key.trim(), value.trim()))
            .ok_or_else(|| line_error(line_number, "expected `key = value`"))?;
        if section == Section::Root {
            if key != "schema_version" && key != "set" && key != "visibility" && key != "format" {
                return Err(line_error(line_number, "unexpected root field"));
            }
            let _ = value;
            continue;
        }
        let scenario = scenarios
            .last_mut()
            .ok_or_else(|| line_error(line_number, "field before scenario"))?;
        match section {
            Section::Scenario => set_scenario_field(scenario, key, value, line_number)?,
            Section::Card => set_card_field(scenario, key, value, line_number)?,
            Section::Mana => set_mana_field(scenario, key, value, line_number)?,
            Section::Action => set_action_field(scenario, key, value, line_number)?,
            Section::Expected => set_expected_field(scenario, key, value, line_number)?,
            Section::Root => unreachable!("root fields are handled before scenario lookup"),
        }
    }
    if scenarios.is_empty() {
        return Err("scenario fixture contains no scenarios".to_owned());
    }
    for scenario in &scenarios {
        if scenario.id.is_empty()
            || scenario.actions.is_empty()
            || scenario.expected.digest.is_empty()
        {
            return Err(format!(
                "scenario requires id, at least one action, and a baseline digest: {:?}",
                scenario.id
            ));
        }
    }
    Ok(scenarios)
}

fn set_scenario_field(
    scenario: &mut ScenarioSpec,
    key: &str,
    value: &str,
    line_number: usize,
) -> Result<(), String> {
    match key {
        "id" => scenario.id = parse_string(value, line_number)?,
        "seed" => scenario.seed = parse_number(value, line_number)?,
        "description" => scenario.description = parse_string(value, line_number)?,
        "triggers" => scenario.triggers = parse_bool(value, line_number)?,
        _ => return Err(line_error(line_number, "unknown scenario field")),
    }
    Ok(())
}

fn set_card_field(
    scenario: &mut ScenarioSpec,
    key: &str,
    value: &str,
    line_number: usize,
) -> Result<(), String> {
    let card = scenario
        .cards
        .last_mut()
        .ok_or_else(|| line_error(line_number, "card field without card table"))?;
    match key {
        "label" => card.label = parse_string(value, line_number)?,
        "owner" => card.owner = parse_number(value, line_number)?,
        "definition" => card.definition = parse_string(value, line_number)?,
        "zone" => card.zone = parse_string(value, line_number)?,
        "tapped" => card.tapped = parse_bool(value, line_number)?,
        "entered_turn" => card.entered_turn = Some(parse_number(value, line_number)?),
        _ => return Err(line_error(line_number, "unknown card field")),
    }
    Ok(())
}

fn set_mana_field(
    scenario: &mut ScenarioSpec,
    key: &str,
    value: &str,
    line_number: usize,
) -> Result<(), String> {
    let mana = scenario
        .mana
        .last_mut()
        .ok_or_else(|| line_error(line_number, "mana field without mana table"))?;
    match key {
        "player" => mana.player = parse_number(value, line_number)?,
        "color" => mana.color = parse_string(value, line_number)?,
        "amount" => mana.amount = parse_number(value, line_number)?,
        _ => return Err(line_error(line_number, "unknown mana field")),
    }
    Ok(())
}

fn set_action_field(
    scenario: &mut ScenarioSpec,
    key: &str,
    value: &str,
    line_number: usize,
) -> Result<(), String> {
    let action = scenario
        .actions
        .last_mut()
        .ok_or_else(|| line_error(line_number, "action field without action table"))?;
    match key {
        "kind" => action.kind = parse_string(value, line_number)?,
        "player" => action.player = parse_number(value, line_number)?,
        "card" => action.card = parse_string(value, line_number)?,
        "target" => action.target = parse_string(value, line_number)?,
        "targets" => action.targets = parse_string_array(value, line_number)?,
        "sacrifices" => action.sacrifice_sources = parse_string_array(value, line_number)?,
        "additional_taps" => {
            action.additional_tap_creatures = parse_string_array(value, line_number)?;
        }
        "counter_sources" => action.counter_sources = parse_string_array(value, line_number)?,
        "convoke" => action.convoke = parse_string_array(value, line_number)?,
        "payment_mana" => action.payment_mana = parse_string_array(value, line_number)?,
        "mana_spend" => action.mana_spend = parse_string_array(value, line_number)?,
        "chosen_x" => action.chosen_x = Some(parse_number(value, line_number)?),
        "attackers" => action.attackers = parse_string_array(value, line_number)?,
        "trigger_order" => action.trigger_order = parse_string_array(value, line_number)?,
        "ability" => action.ability = parse_string(value, line_number)?,
        "color" => action.color = parse_string(value, line_number)?,
        "pay" => action.pay = parse_bool(value, line_number)?,
        "dredge" => action.dredge = parse_string(value, line_number)?,
        "found" => action.found = parse_string(value, line_number)?,
        "expected_error" => action.expected_error = parse_string(value, line_number)?,
        _ => return Err(line_error(line_number, "unknown action field")),
    }
    Ok(())
}

fn set_expected_field(
    scenario: &mut ScenarioSpec,
    key: &str,
    value: &str,
    line_number: usize,
) -> Result<(), String> {
    match key {
        "life" => scenario.expected.life = parse_number_array(value, line_number)?,
        "zones" => scenario.expected.zones = parse_string_array(value, line_number)?,
        "powers" => scenario.expected.powers = parse_string_array(value, line_number)?,
        "toughnesses" => scenario.expected.toughnesses = parse_string_array(value, line_number)?,
        "colors" => scenario.expected.colors = parse_string_array(value, line_number)?,
        "tapped" => scenario.expected.tapped = parse_string_array(value, line_number)?,
        "token_count" => scenario.expected.token_count = Some(parse_number(value, line_number)?),
        "mana" => scenario.expected.mana = parse_string_array(value, line_number)?,
        "mana_receipts" => {
            scenario.expected.mana_receipts = parse_string_array(value, line_number)?;
        }
        "revealed_library_tops" => {
            scenario.expected.revealed_library_tops = parse_string_array(value, line_number)?;
        }
        "stack_size" => scenario.expected.stack_size = Some(parse_number(value, line_number)?),
        "priority" => scenario.expected.priority = Some(parse_number(value, line_number)?),
        "event_markers" => {
            scenario.expected.event_markers = parse_string_array(value, line_number)?;
        }
        "event_absent" => {
            scenario.expected.event_absent = parse_string_array(value, line_number)?;
        }
        "digest" => scenario.expected.digest = parse_string(value, line_number)?,
        _ => return Err(line_error(line_number, "unknown expected field")),
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One fixture execution keeps all bindings and assertions auditable.
fn execute_scenario(specification: &ScenarioSpec) -> Result<ScenarioResult, String> {
    let mut game = if specification.triggers {
        Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
            card_definitions(),
            2,
            rav_mana_ability_bindings(),
            rav_basic_land_type_bindings(),
            rav_additional_spell_cost_bindings(),
            rav_activated_ability_bindings(),
            rav_triggered_ability_bindings(),
            rav_static_continuous_effect_bindings(),
            crate::rav_land_entry_bindings(),
        )
    } else {
        Game::new_with_all_bindings_and_static_continuous_effects(
            card_definitions(),
            2,
            rav_mana_ability_bindings(),
            rav_basic_land_type_bindings(),
            rav_additional_spell_cost_bindings(),
            rav_activated_ability_bindings(),
            rav_static_continuous_effect_bindings(),
        )
    }
    .map_err(rules_error)?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(rules_error)?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(rules_error)?;
    game.register_attachment_bindings(crate::rav_attachment_bindings())
        .map_err(rules_error)?;
    game.register_static_entry_restriction_bindings(crate::rav_static_entry_restriction_bindings())
        .map_err(rules_error)?;
    game.register_mana_ability_cost_bindings(crate::rav_mana_ability_cost_bindings())
        .map_err(rules_error)?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(rules_error)?;
    game.register_activated_ability_cost_modifier_bindings(
        crate::rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(rules_error)?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(rules_error)?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(rules_error)?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(rules_error)?;
    game.set_shuffle_seed(specification.seed);
    let mut labels = BTreeMap::new();
    for setup in &specification.cards {
        let player = checked_player(setup.owner)?;
        let definition = game
            .catalog()
            .get(setup.definition.as_str())
            .ok_or_else(|| {
                format!(
                    "{}: unknown definition `{}`",
                    specification.id, setup.definition
                )
            })?
            .id;
        let card = game
            .add_card(player, definition, parse_zone(&setup.zone)?)
            .map_err(rules_error)?;
        if setup.tapped {
            game.set_tapped_for_setup(card, true).map_err(rules_error)?;
        }
        if let Some(entered_turn) = setup.entered_turn {
            game.set_entered_turn_for_setup(card, entered_turn)
                .map_err(rules_error)?;
        }
        if labels.insert(setup.label.clone(), card).is_some() {
            return Err(format!(
                "{}: duplicate setup label `{}`",
                specification.id, setup.label
            ));
        }
    }
    for setup in &specification.mana {
        game.grant_mana(
            checked_player(setup.player)?,
            parse_color(&setup.color)?,
            setup.amount,
        )
        .map_err(rules_error)?;
    }
    game.clear_event_log();
    for (action_index, action) in specification.actions.iter().enumerate() {
        execute_action(&mut game, &labels, action)
            .map_err(|error| format!("{} action {action_index}: {error}", specification.id))?;
        game.validate_invariants()
            .map_err(|error| format!("{}: {}", specification.id, rules_error(error)))?;
    }
    assert_expected_state(specification, &game, &labels)?;
    let event_log = game.canonical_event_log();
    let digest = event_digest(&event_log);
    if digest != specification.expected.digest {
        return Err(format!(
            "{}: event digest mismatch; expected {}, got {}",
            specification.id, specification.expected.digest, digest
        ));
    }
    Ok(ScenarioResult {
        id: specification.id.clone(),
        event_log,
        digest,
        summary: specification.description.clone(),
    })
}

#[allow(clippy::too_many_lines)] // The public action grammar stays in one auditable dispatch table.
fn execute_action(
    game: &mut Game,
    labels: &BTreeMap<String, ObjectId>,
    action: &ActionSpec,
) -> Result<(), String> {
    let player = checked_player(action.player)?;
    let result = match action.kind.as_str() {
        "begin_game" => game.begin_game().map_err(rules_error),
        "cast" => {
            let targets = cast_targets(action, labels)?;
            let convoke = action
                .convoke
                .iter()
                .map(|entry| parse_convoke(entry, labels))
                .collect::<Result<Vec<_>, _>>()?;
            let payment_mana_abilities = action
                .payment_mana
                .iter()
                .map(|entry| parse_cast_payment_mana_ability(entry, game, labels))
                .collect::<Result<Vec<_>, _>>()?;
            let request = CastRequest {
                card: lookup(labels, &action.card)?,
                targets,
                convoke,
                payment_mana_abilities,
            };
            if action.color.is_empty() {
                if let Some(chosen_x) = action.chosen_x {
                    game.submit_policy_move(
                        player,
                        "rav-scenario.cast-with-payment.v1",
                        PolicyAction::CastWithPayment {
                            request,
                            chosen_x: Some(chosen_x),
                            mana_selection: parse_mana_payment_selection(&action.mana_spend)?,
                        },
                    )
                    .map_err(rules_error)
                } else if action.mana_spend.is_empty() {
                    game.cast_spell(player, request).map_err(rules_error)
                } else {
                    game.cast_spell_with_mana_spend(
                        player,
                        request,
                        parse_mana_payment_selection(&action.mana_spend)?,
                    )
                    .map_err(rules_error)
                }
            } else {
                if action.chosen_x.is_some() {
                    return Err(
                        "chosen-X scenario casts do not yet combine with a chosen color".to_owned(),
                    );
                }
                if !action.mana_spend.is_empty() {
                    return Err(
                        "chosen-color scenario casts do not yet combine with an explicit mana-spend selection"
                            .to_owned(),
                    );
                }
                game.submit_policy_move(
                    player,
                    "rav-scenario.cast-with-color-choice.v1",
                    PolicyAction::CastWithColorChoice {
                        request,
                        color: parse_color(&action.color)?,
                    },
                )
                .map_err(rules_error)
            }
        }
        "pass" => game.pass_priority(player).map_err(rules_error),
        "order_triggers" => {
            let decision = game
                .view_for_player(player)
                .map_err(rules_error)?
                .pending_decision
                .ok_or_else(|| "no simultaneous-trigger ordering decision is pending".to_owned())?;
            if decision.kind != DecisionKind::TriggeredAbilityOrder {
                return Err(
                    "pending decision is not a simultaneous-trigger ordering choice".to_owned(),
                );
            }
            let order = action
                .trigger_order
                .iter()
                .map(|entry| {
                    let (label, ability) = split_pair(entry, "trigger-order entry")?;
                    let source = lookup(labels, label)?;
                    decision
                        .trigger_candidates
                        .iter()
                        .copied()
                        .find(|candidate| {
                            candidate.source == source && candidate.ability == ability
                        })
                        .ok_or_else(|| {
                            format!("trigger-order entry `{entry}` is not a pending candidate")
                        })
                })
                .collect::<Result<Vec<_>, String>>()?;
            game.submit_policy_move(
                player,
                "rav-scenario.order-triggers.v1",
                PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::TriggerOrder(order),
                },
            )
            .map_err(rules_error)
        }
        "declare_attackers" => {
            let attackers = action
                .attackers
                .iter()
                .map(|label| lookup(labels, label))
                .collect::<Result<Vec<_>, _>>()?;
            game.declare_attackers(player, &attackers)
                .map_err(rules_error)
        }
        "declare_blockers" => {
            let assignments = action
                .attackers
                .iter()
                .map(|entry| {
                    let (attacker, blocker) = split_pair(entry, "blocker assignment")?;
                    Ok(CombatBlock {
                        attacker: lookup(labels, attacker)?,
                        blocker: lookup(labels, blocker)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            game.declare_blockers(player, &assignments)
                .map_err(rules_error)
        }
        "draw" => {
            let dredge = (!action.dredge.is_empty())
                .then(|| lookup(labels, &action.dredge))
                .transpose()?;
            game.draw_card(player, dredge).map_err(rules_error)
        }
        "transmute" => {
            let found = (!action.found.is_empty())
                .then(|| lookup(labels, &action.found))
                .transpose()?;
            game.transmute(player, lookup(labels, &action.card)?, found)
                .map_err(rules_error)
        }
        "play_land" => game
            .play_land(player, lookup(labels, &action.card)?)
            .map_err(rules_error),
        "play_land_with_entry_life_payment" => game
            .play_land_with_entry_life_payment(player, lookup(labels, &action.card)?, action.pay)
            .map_err(rules_error),
        "activate_mana_ability" => game
            .activate_mana_ability(
                player,
                lookup(labels, &action.card)?,
                parse_color(&action.color)?,
            )
            .map_err(rules_error),
        "activate_bound_mana_ability" | "activate_bound_mana" => {
            let source = lookup(labels, &action.card)?;
            let definition = game.card_definition(source).map_err(rules_error)?.id;
            let ability_id = rav_mana_ability_bindings()
                .into_iter()
                .find(|binding| {
                    binding.card_definition == definition && binding.ability.id == action.ability
                })
                .map(|binding| binding.ability.id)
                .ok_or_else(|| {
                    format!(
                        "unknown RAV mana ability `{}` for `{definition}`",
                        action.ability
                    )
                })?;
            let chosen_color = (!action.color.is_empty())
                .then(|| parse_color(&action.color))
                .transpose()?;
            game.activate_bound_mana_ability(
                player,
                ManaAbilityActivation {
                    source,
                    ability_id,
                    chosen_color,
                },
            )
            .map_err(rules_error)
        }
        "activate_ability" => {
            let source = lookup(labels, &action.card)?;
            let definition = game.card_definition(source).map_err(rules_error)?.id;
            let ability_id = rav_activated_ability_bindings()
                .into_iter()
                .find(|binding| {
                    binding.card_definition == definition && binding.ability.id == action.ability
                })
                .map(|binding| binding.ability.id)
                .or_else(|| {
                    crate::rav_attachment_bindings()
                        .into_iter()
                        .flat_map(|binding| binding.granted_activated_abilities)
                        .find(|ability| ability.id == action.ability)
                        .map(|ability| ability.id)
                })
                .or_else(|| {
                    card_definitions()
                        .into_iter()
                        .flat_map(|definition| definition.effects)
                        .find_map(|effect| match effect {
                            Effect::GrantActivatedAbilityToControllerCreaturesUntilEndOfTurn {
                                ability,
                            } if ability.id == action.ability => Some(ability.id),
                            _ => None,
                        })
                })
                .ok_or_else(|| {
                    format!(
                        "unknown RAV activated ability `{}` for `{definition}`",
                        action.ability
                    )
                })?;
            let targets = if action.targets.is_empty() {
                (!action.target.is_empty())
                    .then_some(action.target.as_str())
                    .into_iter()
                    .map(|target| parse_target(target, labels))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                action
                    .targets
                    .iter()
                    .map(|target| parse_target(target, labels))
                    .collect::<Result<Vec<_>, _>>()?
            };
            let activation = AbilityActivation {
                source,
                ability_id,
                sacrifice_sources: action
                    .sacrifice_sources
                    .iter()
                    .map(|permanent| lookup(labels, permanent))
                    .collect::<Result<Vec<_>, _>>()?,
                additional_tap_creatures: action
                    .additional_tap_creatures
                    .iter()
                    .map(|permanent| lookup(labels, permanent))
                    .collect::<Result<Vec<_>, _>>()?,
                discard_cards: vec![],
                targets,
            };
            if action.counter_sources.is_empty() {
                game.activate_ability(player, activation)
                    .map_err(rules_error)
            } else {
                game.activate_ability_with_generalized_costs(
                    player,
                    GeneralizedAbilityActivation {
                        activation,
                        cost_payment: AbilityCostPayment {
                            counter_sources: action
                                .counter_sources
                                .iter()
                                .map(|permanent| lookup(labels, permanent))
                                .collect::<Result<Vec<_>, _>>()?,
                            ..AbilityCostPayment::default()
                        },
                        mana_payment_selection: None,
                    },
                )
                .map_err(rules_error)
            }
        }
        "choose_trigger_targets" => {
            let source = lookup(labels, &action.card)?;
            let definition = game.card_definition(source).map_err(rules_error)?.id;
            let ability_id = rav_triggered_ability_bindings()
                .into_iter()
                .find(|binding| {
                    binding.card_definition == definition && binding.ability.id == action.ability
                })
                .map(|binding| binding.ability.id)
                .ok_or_else(|| {
                    format!(
                        "unknown RAV triggered ability `{}` for `{definition}`",
                        action.ability
                    )
                })?;
            let targets = if action.targets.is_empty() {
                (!action.target.is_empty())
                    .then_some(action.target.as_str())
                    .into_iter()
                    .map(|target| parse_target(target, labels))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                action
                    .targets
                    .iter()
                    .map(|target| parse_target(target, labels))
                    .collect::<Result<Vec<_>, _>>()?
            };
            game.submit_policy_move(
                player,
                "rav-scenario.choose-trigger-targets.v1",
                cardbench_magic_engine::PolicyAction::ChooseTriggeredAbilityTargets {
                    source,
                    ability: ability_id,
                    targets,
                },
            )
            .map_err(rules_error)
        }
        "choose_trigger_effect_object" => {
            let source = lookup(labels, &action.card)?;
            let definition = game.card_definition(source).map_err(rules_error)?.id;
            let ability_id = rav_triggered_ability_bindings()
                .into_iter()
                .find(|binding| {
                    binding.card_definition == definition && binding.ability.id == action.ability
                })
                .map(|binding| binding.ability.id)
                .ok_or_else(|| {
                    format!(
                        "unknown RAV triggered ability `{}` for `{definition}`",
                        action.ability
                    )
                })?;
            let selected = (!action.found.is_empty())
                .then(|| lookup(labels, &action.found))
                .transpose()?;
            game.submit_policy_move(
                player,
                "rav-scenario.choose-trigger-effect-object.v1",
                cardbench_magic_engine::PolicyAction::ChooseTriggeredAbilityEffectObject {
                    source,
                    ability: ability_id,
                    selected,
                },
            )
            .map_err(rules_error)
        }
        "choose_library_search" => {
            let decision = game
                .view_for_player(player)
                .map_err(rules_error)?
                .pending_decision
                .ok_or_else(|| "no private library-search decision is pending".to_owned())?;
            let selected = (!action.found.is_empty())
                .then(|| lookup(labels, &action.found))
                .transpose()?
                .into_iter()
                .collect();
            game.submit_policy_move(
                player,
                "rav-scenario.choose-library-search.v1",
                PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::Objects(selected),
                },
            )
            .map_err(rules_error)
        }
        "resolve_optional_trigger" => {
            let source = lookup(labels, &action.card)?;
            let definition = game.card_definition(source).map_err(rules_error)?.id;
            let ability_id = rav_triggered_ability_bindings()
                .into_iter()
                .find(|binding| {
                    binding.card_definition == definition && binding.ability.id == action.ability
                })
                .map(|binding| binding.ability.id)
                .ok_or_else(|| {
                    format!(
                        "unknown RAV optional triggered ability `{}` for `{definition}`",
                        action.ability
                    )
                })?;
            let target = (!action.target.is_empty())
                .then_some(action.target.as_str())
                .map(|target| parse_target(target, labels))
                .transpose()?;
            game.submit_policy_move(
                player,
                "rav-scenario.resolve-optional-trigger.v1",
                cardbench_magic_engine::PolicyAction::ResolveOptionalTriggeredAbility {
                    source,
                    ability: ability_id,
                    pay: action.pay,
                    target,
                },
            )
            .map_err(rules_error)
        }
        _ => Err(format!("unknown action kind `{}`", action.kind)),
    };
    match (result, action.expected_error.is_empty()) {
        (Ok(()), true) => Ok(()),
        (Err(error), true) => Err(error),
        (Ok(()), false) => Err(format!(
            "action `{}` unexpectedly succeeded; expected error containing `{}`",
            action.kind, action.expected_error
        )),
        (Err(error), false) if error.contains(&action.expected_error) => Ok(()),
        (Err(error), false) => Err(format!(
            "action `{}` error `{error}` did not contain expected `{}`",
            action.kind, action.expected_error
        )),
    }
}

fn cast_targets(
    action: &ActionSpec,
    labels: &BTreeMap<String, ObjectId>,
) -> Result<Vec<Target>, String> {
    if !action.target.is_empty() && !action.targets.is_empty() {
        return Err("cast action may specify `target` or `targets`, not both".to_owned());
    }
    let target_specs = if action.targets.is_empty() {
        (!action.target.is_empty())
            .then_some(action.target.as_str())
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        action.targets.iter().map(String::as_str).collect()
    };
    target_specs
        .into_iter()
        .map(|target| parse_target(target, labels))
        .collect()
}

/// Resolves the deliberately narrow public fixture notation for a mana
/// ability used while one cast pays its mana cost.
fn parse_cast_payment_mana_ability(
    entry: &str,
    game: &Game,
    labels: &BTreeMap<String, ObjectId>,
) -> Result<CastPaymentManaAbility, String> {
    let mut fields = entry.split(':');
    let label = fields
        .next()
        .ok_or_else(|| format!("cast payment mana ability `{entry}` lacks a source"))?;
    let requested_ability = fields
        .next()
        .ok_or_else(|| format!("cast payment mana ability `{entry}` lacks an ability identity"))?;
    let chosen_color = fields.next().map(parse_color).transpose()?;
    if fields.next().is_some() {
        return Err(format!(
            "cast payment mana ability `{entry}` has too many colon-separated fields"
        ));
    }
    let source = lookup(labels, label)?;
    if requested_ability == "basic-land" {
        if chosen_color.is_some() {
            return Err(format!(
                "typed basic land cast payment `{entry}` must not supply a color choice"
            ));
        }
        let color = game
            .basic_land_type(source)
            .map_err(rules_error)?
            .ok_or_else(|| format!("`{label}` is not a typed RAV basic land"))?
            .intrinsic_mana_color();
        return Ok(CastPaymentManaAbility::BasicLand(
            BasicLandManaAbilityActivation {
                land: source,
                color,
            },
        ));
    }
    let definition = game.card_definition(source).map_err(rules_error)?.id;
    let ability_id = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition && binding.ability.id == requested_ability
        })
        .map(|binding| binding.ability.id)
        .ok_or_else(|| {
            format!(
                "unknown RAV cast-payment mana ability `{requested_ability}` for `{definition}`"
            )
        })?;
    Ok(CastPaymentManaAbility::Bound(ManaAbilityActivation {
        source,
        ability_id,
        chosen_color,
    }))
}

fn parse_mana_payment_selection(entries: &[String]) -> Result<ManaPaymentSelection, String> {
    let mut selection = ManaPaymentSelection::default();
    for entry in entries {
        let (symbol_kind, color) = split_pair(entry, "mana-spend selection")?;
        match symbol_kind {
            "generic" => selection.generic.push(parse_color(color)?),
            "hybrid" => selection.hybrid.push(parse_color(color)?),
            _ => {
                return Err(format!(
                    "mana-spend selection `{entry}` must name `generic` or `hybrid`"
                ));
            }
        }
    }
    Ok(selection)
}

#[allow(clippy::too_many_lines)] // Fixture assertion fields intentionally stay in one auditable parser path.
fn assert_expected_state(
    specification: &ScenarioSpec,
    game: &Game,
    labels: &BTreeMap<String, ObjectId>,
) -> Result<(), String> {
    if !specification.expected.life.is_empty() {
        let actual: Vec<_> = game.players.iter().map(|player| player.life).collect();
        if actual != specification.expected.life {
            return Err(format!(
                "{}: life expected {:?}, got {actual:?}",
                specification.id, specification.expected.life
            ));
        }
    }
    for expected in &specification.expected.zones {
        let (label, zone) = split_pair(expected, "zone assertion")?;
        if game.zone_of(lookup(labels, label)?) != Some(parse_zone(zone)?) {
            return Err(format!(
                "{}: zone assertion failed for `{label}`",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.powers {
        let (label, power) = split_pair(expected, "power assertion")?;
        let expected_power: i32 = power
            .parse()
            .map_err(|error| format!("invalid power `{power}`: {error}"))?;
        let actual = game
            .characteristics(lookup(labels, label)?)
            .map_err(rules_error)?
            .power;
        if actual != Some(expected_power) {
            return Err(format!(
                "{}: expected `{label}` to have power {expected_power}, got {actual:?}",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.toughnesses {
        let (label, toughness) = split_pair(expected, "toughness assertion")?;
        let expected_toughness: i32 = toughness
            .parse()
            .map_err(|error| format!("invalid toughness `{toughness}`: {error}"))?;
        let actual = game
            .characteristics(lookup(labels, label)?)
            .map_err(rules_error)?
            .toughness;
        if actual != Some(expected_toughness) {
            return Err(format!(
                "{}: expected `{label}` to have toughness {expected_toughness}, got {actual:?}",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.colors {
        let (label, color) = split_pair(expected, "color assertion")?;
        let actual = game
            .characteristics(lookup(labels, label)?)
            .map_err(rules_error)?
            .colors;
        let expected = BTreeSet::from([parse_color(color)?]);
        if actual != expected {
            return Err(format!(
                "{}: expected `{label}` to have exactly color {color}, got {actual:?}",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.tapped {
        let (label, state) = split_pair(expected, "tapped assertion")?;
        let expected_tapped = parse_bool(state, 0)?;
        let actual = game
            .object(lookup(labels, label)?)
            .map_err(rules_error)?
            .tapped;
        if actual != expected_tapped {
            return Err(format!(
                "{}: tapped assertion failed for `{label}`",
                specification.id
            ));
        }
    }
    if let Some(expected_count) = specification.expected.token_count {
        let actual = game
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter())
            .filter(|card| {
                game.object(**card)
                    .is_ok_and(|object| object.token.is_some())
            })
            .count();
        if actual != expected_count {
            return Err(format!(
                "{}: expected {expected_count} tokens, got {actual}",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.mana {
        let mut fields = expected.split(':');
        let player: usize = fields
            .next()
            .ok_or_else(|| format!("mana assertion `{expected}` lacks a player"))?
            .parse()
            .map_err(|error| format!("invalid mana player in `{expected}`: {error}"))?;
        let color = fields
            .next()
            .ok_or_else(|| format!("mana assertion `{expected}` lacks a color"))?;
        let amount: u8 = fields
            .next()
            .ok_or_else(|| format!("mana assertion `{expected}` lacks an amount"))?
            .parse()
            .map_err(|error| format!("invalid mana amount in `{expected}`: {error}"))?;
        if fields.next().is_some() {
            return Err(format!("mana assertion `{expected}` has too many fields"));
        }
        let player = checked_player(player)?;
        let actual = game
            .player(player)
            .map_err(rules_error)?
            .mana_pool
            .amount(parse_color(color)?);
        if actual != amount {
            return Err(format!(
                "{}: mana assertion `{expected}` expected {amount}, got {actual}",
                specification.id
            ));
        }
    }
    if !specification.expected.revealed_library_tops.is_empty() {
        let expected = specification
            .expected
            .revealed_library_tops
            .iter()
            .map(|entry| {
                let (owner, label) = split_pair(entry, "revealed library-top assertion")?;
                Ok((
                    checked_player(owner.parse::<usize>().map_err(|error| {
                        format!("invalid revealed library owner in `{entry}`: {error}")
                    })?)?,
                    lookup(labels, label)?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        for viewer in &game.players {
            let actual = game
                .view_for_player(viewer.id)
                .map_err(rules_error)?
                .revealed_library_tops
                .into_iter()
                .map(|top| (top.owner, top.card.id))
                .collect::<Vec<_>>();
            if actual != expected {
                return Err(format!(
                    "{}: viewer {} expected public library tops {expected:?}, got {actual:?}",
                    specification.id, viewer.id.0
                ));
            }
        }
    }
    if let Some(expected_stack_size) = specification.expected.stack_size
        && game.stack.len() != expected_stack_size
    {
        return Err(format!(
            "{}: stack size expected {expected_stack_size}, got {}",
            specification.id,
            game.stack.len()
        ));
    }
    if let Some(expected_priority) = specification.expected.priority
        && game.priority != checked_player(expected_priority)?
    {
        return Err(format!(
            "{}: priority expected player {expected_priority}, got {:?}",
            specification.id, game.priority
        ));
    }
    let event_log = game.canonical_event_log();
    for marker in &specification.expected.event_markers {
        if !event_log.iter().any(|event| event.contains(marker)) {
            return Err(format!(
                "{}: event log has no `{marker}` marker",
                specification.id
            ));
        }
    }
    for expected in &specification.expected.mana_receipts {
        let mut fields = expected.split(':');
        let player: usize = fields
            .next()
            .ok_or_else(|| format!("mana receipt `{expected}` lacks a player"))?
            .parse()
            .map_err(|error| format!("invalid mana-receipt player in `{expected}`: {error}"))?;
        let color = fields
            .next()
            .ok_or_else(|| format!("mana receipt `{expected}` lacks a color"))?;
        let amount: u8 = fields
            .next()
            .ok_or_else(|| format!("mana receipt `{expected}` lacks an amount"))?
            .parse()
            .map_err(|error| format!("invalid mana-receipt amount in `{expected}`: {error}"))?;
        if fields.next().is_some() {
            return Err(format!("mana receipt `{expected}` has too many fields"));
        }
        let receipt = format!(
            "ManaAdded {{ player: {:?}, color: {:?}, amount: {amount} }}",
            checked_player(player)?,
            parse_color(color)?,
        );
        if !event_log.iter().any(|event| event == &receipt) {
            return Err(format!(
                "{}: event log has no precise mana receipt `{receipt}`",
                specification.id
            ));
        }
    }
    for marker in &specification.expected.event_absent {
        if event_log.iter().any(|event| event.contains(marker)) {
            return Err(format!(
                "{}: event log unexpectedly contains `{marker}`",
                specification.id
            ));
        }
    }
    Ok(())
}

fn parse_target(value: &str, labels: &BTreeMap<String, ObjectId>) -> Result<Target, String> {
    let (kind, target) = split_pair(value, "target")?;
    match kind {
        "player" => Ok(Target::Player(checked_player(
            target
                .parse::<usize>()
                .map_err(|error| format!("invalid target player `{target}`: {error}"))?,
        )?)),
        "permanent" => Ok(Target::Permanent(lookup(labels, target)?)),
        "spell" => Ok(Target::Spell(lookup(labels, target)?)),
        "sacrifice" => Ok(Target::SacrificePermanent(lookup(labels, target)?)),
        _ => Err(format!("unknown target kind `{kind}`")),
    }
}

fn parse_convoke(
    value: &str,
    labels: &BTreeMap<String, ObjectId>,
) -> Result<ConvokePayment, String> {
    let (label, contribution) = split_pair(value, "convoke payment")?;
    let contribution = match contribution {
        "generic" => ConvokeContribution::Generic,
        color => ConvokeContribution::Color(parse_color(color)?),
    };
    Ok(ConvokePayment {
        creature: lookup(labels, label)?,
        contribution,
    })
}

fn lookup(labels: &BTreeMap<String, ObjectId>, label: &str) -> Result<ObjectId, String> {
    labels
        .get(label)
        .copied()
        .ok_or_else(|| format!("unknown scenario card label `{label}`"))
}

fn parse_zone(value: &str) -> Result<Zone, String> {
    match value {
        "library" => Ok(Zone::Library),
        "hand" => Ok(Zone::Hand),
        "battlefield" => Ok(Zone::Battlefield),
        "graveyard" => Ok(Zone::Graveyard),
        "exile" => Ok(Zone::Exile),
        _ => Err(format!("unknown zone `{value}`")),
    }
}

fn parse_color(value: &str) -> Result<Color, String> {
    match value {
        "white" => Ok(Color::White),
        "blue" => Ok(Color::Blue),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "colorless" => Ok(Color::Colorless),
        _ => Err(format!("unknown color `{value}`")),
    }
}

fn checked_player(index: usize) -> Result<PlayerId, String> {
    if index > 1 {
        return Err(format!(
            "scenario player {index} is outside the two-player fixture"
        ));
    }
    Ok(PlayerId(index))
}

fn parse_string(value: &str, line_number: usize) -> Result<String, String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| line_error(line_number, "expected quoted string"))
}

fn parse_string_array(value: &str, line_number: usize) -> Result<Vec<String>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| line_error(line_number, "expected string array"))?;
    if inner.trim().is_empty() {
        return Ok(vec![]);
    }
    inner
        .split(',')
        .map(|item| parse_string(item.trim(), line_number))
        .collect()
}

fn parse_number<T>(value: &str, line_number: usize) -> Result<T, String>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| line_error(line_number, &format!("invalid number `{value}`: {error}")))
}

fn parse_number_array(value: &str, line_number: usize) -> Result<Vec<i64>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| line_error(line_number, "expected number array"))?;
    if inner.trim().is_empty() {
        return Ok(vec![]);
    }
    inner
        .split(',')
        .map(|item| parse_number(item.trim(), line_number))
        .collect()
}

fn parse_bool(value: &str, line_number: usize) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(line_error(line_number, "expected boolean")),
    }
}

fn split_pair<'a>(value: &'a str, label: &str) -> Result<(&'a str, &'a str), String> {
    value
        .split_once(':')
        .ok_or_else(|| format!("{label} `{value}` must contain a colon"))
}

fn line_error(line_number: usize, message: &str) -> String {
    format!("line {}: {message}", line_number + 1)
}

#[allow(clippy::needless_pass_by_value)] // `Result::map_err` supplies an owned rules error.
fn rules_error(error: RulesError) -> String {
    error.to_string()
}
