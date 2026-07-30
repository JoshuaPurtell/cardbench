//! Public, fixture-driven RAV scenario parser and executor.
//!
//! The format intentionally uses a constrained TOML subset so the reference runner
//! has no parser dependency and each evaluated setup/action/assertion is visible in
//! versioned data rather than hidden in a Rust test body.

use std::collections::BTreeMap;
use std::fs;

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, ManaAbilityActivation, ObjectId,
    PlayerId, RulesError, Target, Zone,
};

use crate::{ScenarioResult, card_definitions, event_digest, rav_mana_ability_bindings, set_root};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ScenarioSpec {
    id: String,
    seed: u64,
    description: String,
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
    target: String,
    convoke: Vec<String>,
    ability: String,
    color: String,
    dredge: String,
    found: String,
    expected_error: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ExpectedState {
    life: Vec<i16>,
    zones: Vec<String>,
    powers: Vec<String>,
    toughnesses: Vec<String>,
    tapped: Vec<String>,
    token_count: Option<usize>,
    mana: Vec<String>,
    mana_receipts: Vec<String>,
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
        "convoke" => action.convoke = parse_string_array(value, line_number)?,
        "ability" => action.ability = parse_string(value, line_number)?,
        "color" => action.color = parse_string(value, line_number)?,
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
        "tapped" => scenario.expected.tapped = parse_string_array(value, line_number)?,
        "token_count" => scenario.expected.token_count = Some(parse_number(value, line_number)?),
        "mana" => scenario.expected.mana = parse_string_array(value, line_number)?,
        "mana_receipts" => {
            scenario.expected.mana_receipts = parse_string_array(value, line_number)?;
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

fn execute_scenario(specification: &ScenarioSpec) -> Result<ScenarioResult, String> {
    let mut game =
        Game::new_with_mana_abilities(card_definitions(), 2, rav_mana_ability_bindings())
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

fn execute_action(
    game: &mut Game,
    labels: &BTreeMap<String, ObjectId>,
    action: &ActionSpec,
) -> Result<(), String> {
    let player = checked_player(action.player)?;
    let result = match action.kind.as_str() {
        "begin_game" => game.begin_game().map_err(rules_error),
        "cast" => {
            let targets = if action.target.is_empty() {
                vec![]
            } else {
                vec![parse_target(&action.target, labels)?]
            };
            let convoke = action
                .convoke
                .iter()
                .map(|entry| parse_convoke(entry, labels))
                .collect::<Result<Vec<_>, _>>()?;
            game.cast_spell(
                player,
                CastRequest {
                    card: lookup(labels, &action.card)?,
                    targets,
                    convoke,
                },
            )
            .map_err(rules_error)
        }
        "pass" => game.pass_priority(player).map_err(rules_error),
        "declare_attackers" => game.declare_attackers(player, &[]).map_err(rules_error),
        "draw" => {
            let dredge = (!action.dredge.is_empty())
                .then(|| lookup(labels, &action.dredge))
                .transpose()?;
            game.draw_card(player, dredge).map_err(rules_error)
        }
        "transmute" => game
            .transmute(
                player,
                lookup(labels, &action.card)?,
                lookup(labels, &action.found)?,
            )
            .map_err(rules_error),
        "play_land" => game
            .play_land(player, lookup(labels, &action.card)?)
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
        let expected_power: i16 = power
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
        let expected_toughness: i16 = toughness
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

fn parse_number_array(value: &str, line_number: usize) -> Result<Vec<i16>, String> {
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
