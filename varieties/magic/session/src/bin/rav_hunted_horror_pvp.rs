//! Full-game diagnostic authority for RAV Hunted Horror.

use cardbench_magic_engine::{
    CardType, Color, CreatureSubtype, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, PolicyMoveKind,
    Step, Target, TargetRequirement, TokenSpec, TriggerCondition, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-HUNTED-HORROR";
const CARD_NAME: &str = "Hunted Horror";
const TRIGGER_ID: &str = "etb-opponent-centaurs";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_horror.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const SEED: u64 = 0;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Hunted Horror definition and Centaur ETB trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed black 7/7 creature with Trample.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-HORROR",
        name: "Hunted Horror",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "trample",
            "enter-battlefield-targeted-opponent-token-creation",
            "centaur-protection-from-black",
        ],
        power: Some(7),
        toughness: Some(7),
        keywords: vec![Keyword::Trample],
        effects: vec![],
    }
}

/// Gives exactly two green 3/3 Centaurs with Protection from Black to a target opponent.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-HORROR",
        ability: TriggeredAbility {
            id: "etb-opponent-centaurs",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::hunted_centaur(),
                count: 2,
            }],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/hunted_horror.rs"]
mod candidate_hunted_horror;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Hunted Horror authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let malicious_source_rejected = malicious_candidate_source_rejected()?;
    if !malicious_source_rejected {
        return Err(
            "compile-valid candidate side-effect mutation was not rejected before candidate calls"
                .into(),
        );
    }
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let reference = run_match(Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Hunted Horror full-game replay was not deterministic".into());
    }
    let mutations = json!({
        "body_changed_detected_in_game": mutation_detected(Mutation::WrongBody),
        "trample_removed_detected_in_game": mutation_detected(Mutation::NoTrample),
        "etb_trigger_removed_detected_in_game": mutation_detected(Mutation::RemoveTrigger),
        "opponent_target_requirement_removed_detected_in_game": mutation_detected(Mutation::WrongTarget),
        "centaur_protection_removed_detected_in_game": mutation_detected(Mutation::WrongToken),
        "centaur_count_changed_detected_in_game": mutation_detected(Mutation::WrongCount),
        "mana_cost_changed_detected_in_game": mutation_detected(Mutation::WrongManaCost),
        "compile_valid_candidate_side_effect_rejected_before_execution": malicious_source_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "body_changed_detected_in_game",
        "trample_removed_detected_in_game",
        "etb_trigger_removed_detected_in_game",
        "opponent_target_requirement_removed_detected_in_game",
        "centaur_protection_removed_detected_in_game",
        "centaur_count_changed_detected_in_game",
        "mana_cost_changed_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Hunted Horror semantic mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    if cards_passed != 1 {
        return Err("Hunted Horror trace must bind exactly one RAV catalog identity".into());
    }
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":90,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["110 and 111 Permanents and tokens","117 Timing and priority","202.1-202.2 Colors and mana cost","302.1 Creature characteristics","601.2 Casting spells","603.2 and 603.3 ETB trigger and target selection","608.2 Resolving triggered abilities","702.19 Trample","702.16 Protection","701.6 Creating tokens"],"source_contract":"CardBench-authored semantic summary: Hunted Horror is a black {B}{B} 7/7 creature with trample; when it enters the battlefield, target opponent creates two green 3/3 Centaur creature tokens with protection from black.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":cards_passed,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,
        "candidate_source_validation":{
            "candidate_ast_contract_matched":true,
            "compile_valid_injected_side_effect_candidate_rejected":malicious_source_rejected,
            "guard_ran_before_candidate_function_calls":true
        },
        "mutation_sensitivity":mutations,"traces":[trace_json(&reference)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_hunted_horror::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Hunted Horror definition is missing from the RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Black, Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(7)
        || definition.toughness != Some(7)
        || definition.keywords != vec![Keyword::Trample]
        || definition.is_basic_land
        || !definition.effects.is_empty()
    {
        return Err("Hunted Horror literal is outside the task source contract".into());
    }
    let binding = candidate_hunted_horror::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID)
        .ok_or("Hunted Horror opponent-targeted ETB binding is missing from the RAV registry")?;
    if binding != registered
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.optional
        || binding.ability.targets != vec![TargetRequirement::Opponent]
        || binding.ability.effects
            != vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::hunted_centaur(),
                count: 2,
            }]
    {
        return Err("Hunted Horror trigger binding is outside the task source contract".into());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hunted Horror source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Hunted Horror source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Hunted Horror source AST differs from task contract".into());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(&item.vis, Visibility::Restricted(_))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none() =>
            {
                let (expected_name, expected_type) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                    _ => return Err("Hunted Horror source AST differs from task contract".into()),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err("Hunted Horror source AST differs from task contract".into());
                };
                if !return_type_is(&item.sig.output, expected_type)
                    || !path_is_type(&literal.path, expected_type)
                    || literal.rest.is_some()
                    || !functions.insert(expected_name.to_owned())
                {
                    return Err("Hunted Horror source AST differs from task contract".into());
                }
            }
            _ => return Err("Hunted Horror source AST differs from task contract".into()),
        }
    }
    let expected = syn::parse_file(EXPECTED_CANDIDATE_SOURCE)
        .expect("trusted Hunted Horror source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err("Hunted Horror source AST differs from task contract".into());
    }
    Ok(())
}

fn return_type_is(output: &ReturnType, expected: &str) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn malicious_candidate_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hunted Horror source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    std::process::exit(0);";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Hunted Horror side-effect mutation could not be constructed".into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || cards
            .iter()
            .filter(|card| card.collector_number == 90 && card.name == CARD_NAME)
            .count()
            != 1
    {
        return Err("RAV catalog denominator differs from source contract".into());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongBody,
    NoTrample,
    RemoveTrigger,
    WrongTarget,
    WrongToken,
    WrongCount,
    WrongManaCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenFact {
    object: ObjectId,
    controller: PlayerId,
    green: bool,
    creature: bool,
    centaur: bool,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    horror: Option<ObjectId>,
    paid_colors: Vec<Color>,
    power: Option<i32>,
    toughness: Option<i32>,
    trample: bool,
    target: Option<PlayerId>,
    trigger_stacked: bool,
    trigger_resolved: bool,
    trigger_controller: Option<PlayerId>,
    target_decision_actor: Option<PlayerId>,
    target_decision_submissions: u32,
    source_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    tokens: Vec<TokenFact>,
    protection_probe_rejected: bool,
    protection_probe_no_state_change: bool,
    watchwolf: Option<ObjectId>,
    watchwolf_blocked: bool,
    damage_to_watchwolf: i32,
    damage_to_player: i32,
    centaurs_present_after_creation: Vec<bool>,
}

fn run_match(mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Hunted Horror definition missing")?;
    *definition = candidate_hunted_horror::definition();
    match mutation {
        Mutation::WrongBody => {
            definition.power = Some(6);
        }
        Mutation::NoTrample => definition.keywords.clear(),
        Mutation::WrongManaCost => {
            definition.mana_cost =
                ManaCost::with_colors(0, [Color::Black, Color::Black, Color::Black])
        }
        _ => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    let trigger = triggers
        .iter()
        .position(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID);
    match mutation {
        Mutation::RemoveTrigger => {
            if let Some(index) = trigger {
                triggers.remove(index);
            }
        }
        Mutation::WrongTarget | Mutation::WrongToken | Mutation::WrongCount => {
            let index = trigger.ok_or("Hunted Horror trigger missing for mutation")?;
            triggers[index] = candidate_hunted_horror::triggered_ability();
            match mutation {
                Mutation::WrongTarget => {
                    triggers[index].ability.targets = vec![TargetRequirement::Player];
                    triggers[index].ability.effects = vec![Effect::CreateTokenForTargetPlayer {
                        token: TokenSpec::hunted_centaur(),
                        count: 2,
                    }];
                }
                Mutation::WrongToken => {
                    let mut token = TokenSpec::hunted_centaur();
                    token.keywords.clear();
                    triggers[index].ability.effects =
                        vec![Effect::CreateTokenForTargetOpponent { token, count: 2 }];
                }
                Mutation::WrongCount => {
                    triggers[index].ability.effects = vec![Effect::CreateTokenForTargetOpponent {
                        token: TokenSpec::hunted_centaur(),
                        count: 1,
                    }]
                }
                _ => unreachable!(),
            }
        }
        _ => {}
    }
    let mut all_triggers = triggers;
    all_triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        all_triggers,
        vec![],
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-WATCHWOLF".into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [
        deck0
            .mainboard
            .iter()
            .map(|e| usize::from(e.count))
            .sum::<usize>(),
        deck1
            .mainboard
            .iter()
            .map(|e| usize::from(e.count))
            .sum::<usize>(),
    ] != [60, 60]
    {
        return Err("Hunted Horror task decks must contain 60 cards".into());
    }
    let config = DeckMatchConfig {
        shuffle_seed: SEED,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(SEED).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(HorrorPolicy::new(PlayerId(0), mutation)),
        Box::new(HorrorPolicy::new(PlayerId(1), mutation)),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        horror: None,
        paid_colors: vec![],
        power: None,
        toughness: None,
        trample: false,
        target: None,
        trigger_stacked: false,
        trigger_resolved: false,
        trigger_controller: None,
        target_decision_actor: None,
        target_decision_submissions: 0,
        source_incarnation: None,
        resolved_incarnation: None,
        tokens: vec![],
        protection_probe_rejected: false,
        protection_probe_no_state_change: false,
        watchwolf: None,
        watchwolf_blocked: false,
        damage_to_watchwolf: 0,
        damage_to_player: 0,
        centaurs_present_after_creation: vec![],
    };
    let mut attempted = 0u32;
    let mut probe_done = false;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted >= MAX_MOVES {
            return Err(format!(
                "Hunted Horror game exceeded bounds at turn {} step {:?}, life {:?}",
                game.turn,
                game.step,
                game.players.iter().map(|p| p.life).collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        if !probe_done && player == PlayerId(1) && view.step == Step::DeclareBlockers {
            if let (Some(attacker), Some(token)) = (
                view.combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .map(|card| card.id),
                captured.tokens.first().map(|token| token.object),
            ) {
                let before_events = game.event_log.len();
                let before_view = view.clone();
                let result = game.submit_policy_move(
                    player,
                    policy.id(),
                    PolicyAction::DeclareBlockers {
                        assignments: vec![cardbench_magic_engine::CombatBlock {
                            attacker,
                            blocker: token,
                        }],
                    },
                );
                attempted += 1;
                if result.is_ok() {
                    captured.protection_probe_rejected = false;
                    captured.protection_probe_no_state_change = false;
                    captured.accepted += 1;
                    for event in &game.event_log[before_events..] {
                        capture_event(&mut captured, &game, event)?;
                    }
                    probe_done = true;
                    continue;
                }
                captured.rejected += 1;
                captured.protection_probe_rejected = true;
                let after_view = game.view_for_player(player).map_err(|e| e.to_string())?;
                captured.protection_probe_no_state_change = game.event_log.len() == before_events
                    && after_view == before_view
                    && game.validate_invariants().is_ok();
                if !captured.protection_probe_no_state_change {
                    return Err("Protection from black block rejection changed game state".into());
                }
                probe_done = true;
            }
        }
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        let previous = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Hunted Horror policy rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[previous..] {
            capture_event(&mut captured, &game, event)?;
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|p| p.0);
    captured.turns = game.turn;
    captured.rejected += attempted.saturating_sub(captured.accepted + captured.rejected);
    Ok(captured)
}

fn capture_event(c: &mut Captured, game: &Game, event: &GameEvent) -> Result<(), String> {
    match event {
        GameEvent::SpellManaPaid {
            player: PlayerId(0),
            card,
            colors,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
            c.horror = Some(*card);
            c.paid_colors = colors.clone();
        }
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => c.horror = Some(*card),
        GameEvent::SpellResolved { card } if Some(*card) == c.horror => {
            let characteristics = game.characteristics(*card).map_err(|e| e.to_string())?;
            c.power = characteristics.power;
            c.toughness = characteristics.toughness;
            c.trample = characteristics.keywords.contains(&Keyword::Trample);
        }
        GameEvent::TriggeredAbilityStacked {
            controller,
            source,
            source_incarnation,
            ability: TRIGGER_ID,
            ..
        } if Some(*source) == c.horror => {
            c.trigger_stacked = true;
            c.trigger_controller = Some(*controller);
            c.source_incarnation = Some(*source_incarnation);
            if let Some(stack) = game.stack.last() {
                c.target = match stack.targets.first() {
                    Some(Target::Player(player)) => Some(*player),
                    _ => None,
                };
            }
        }
        GameEvent::PolicyMoveSubmitted {
            player,
            policy,
            kind: PolicyMoveKind::ChooseTriggeredAbilityTargets,
        } if policy == "rav.hunted-horror-pilot.v1" => {
            c.target_decision_actor = Some(*player);
            c.target_decision_submissions += 1;
        }
        GameEvent::AbilityResolved {
            source,
            source_incarnation,
            ability: TRIGGER_ID,
        } if Some(*source) == c.horror => {
            c.trigger_resolved = true;
            c.resolved_incarnation = Some(*source_incarnation);
            c.centaurs_present_after_creation = c
                .tokens
                .iter()
                .map(|token| game.zone_of(token.object) == Some(Zone::Battlefield))
                .collect();
        }
        GameEvent::TokenCreated { player, token } => {
            let characteristics = game.characteristics(*token).map_err(|e| e.to_string())?;
            c.tokens.push(TokenFact {
                object: *token,
                controller: *player,
                green: characteristics.colors == BTreeSet::from([Color::Green]),
                creature: characteristics.card_types.contains(&CardType::Creature),
                centaur: characteristics
                    .creature_subtypes
                    .contains(&CreatureSubtype::Centaur),
                power: characteristics.power,
                toughness: characteristics.toughness,
                keywords: characteristics
                    .keywords
                    .iter()
                    .map(|keyword| format!("{keyword:?}"))
                    .collect(),
            });
        }
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|d| d.id == "RAV-WATCHWOLF") =>
        {
            c.watchwolf = Some(*card)
        }
        GameEvent::SpellResolved { card }
            if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == "RAV-WATCHWOLF") =>
        {
            c.watchwolf = Some(*card)
        }
        GameEvent::BlockersDeclared {
            player: PlayerId(1),
            assignments,
        } if assignments.iter().any(|(attacker, blocker)| {
            Some(*attacker) == c.horror && Some(*blocker) == c.watchwolf
        }) =>
        {
            c.watchwolf_blocked = true
        }
        GameEvent::DamageDealtToPermanent {
            source,
            permanent,
            amount,
        } if Some(*source) == c.horror && Some(*permanent) == c.watchwolf => {
            c.damage_to_watchwolf += *amount
        }
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount,
        } if Some(*source) == c.horror => c.damage_to_player += *amount,
        _ => {}
    }
    Ok(())
}

struct HorrorPolicy {
    player: PlayerId,
    mutation: Mutation,
    horror_cast: bool,
    watchwolf_cast: bool,
}
impl HorrorPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            horror_cast: false,
            watchwolf_cast: false,
        }
    }
}
impl CodePolicy for HorrorPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.hunted-horror-pilot.v1"
        } else {
            "rav.hunted-horror-centaur-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player == PlayerId(1) {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-WATCHWOLF") && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![cardbench_magic_engine::CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let wanted = if self.player == PlayerId(0) {
                "RAV-SWAMP"
            } else {
                if view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.card_types.contains(&CardType::Land))
                    .any(|card| card.definition == Some("RAV-FOREST"))
                {
                    "RAV-PLAINS"
                } else {
                    "RAV-FOREST"
                }
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(wanted))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if self.player == PlayerId(1) {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| matches!(card.definition, Some("RAV-FOREST" | "RAV-PLAINS")))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
        }
        if self.player == PlayerId(0)
            && !self.horror_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = if self.mutation == Mutation::WrongManaCost {
                ManaCost::with_colors(0, [Color::Black, Color::Black, Color::Black])
            } else {
                ManaCost::with_colors(0, [Color::Black, Color::Black])
            };
            if view.mana_pool.amount(Color::Black)
                >= cost
                    .colored
                    .iter()
                    .filter(|color| **color == Color::Black)
                    .count() as u8
            {
                self.horror_cast = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Black; cost.generic as usize],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some("RAV-SWAMP") && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Black,
                };
            }
        }
        if self.player == PlayerId(1)
            && !self.watchwolf_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-WATCHWOLF"))
        {
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                self.watchwolf_cast = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
            let missing = if view.mana_pool.amount(Color::Green) == 0 {
                Color::Green
            } else {
                Color::White
            };
            let wanted = if missing == Color::Green {
                "RAV-FOREST"
            } else {
                "RAV-PLAINS"
            };
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(wanted) && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: missing,
                };
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != TRIGGER_ID {
            return None;
        }
        let player = if self.mutation == Mutation::WrongTarget {
            PlayerId(0)
        } else {
            PlayerId(1)
        };
        if !choice
            .target_options
            .first()?
            .contains(&Target::Player(player))
        {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![Target::Player(player)],
        })
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    let horror = c.horror.ok_or("Hunted Horror was not cast")?;
    if c.winner != Some(0) || c.rejected != 1 {
        return Err("Hunted Horror duel did not reach a terminal P0 win with only the expected protection probe rejected".into());
    }
    if c.paid_colors != vec![Color::Black, Color::Black]
        || c.power != Some(7)
        || c.toughness != Some(7)
        || !c.trample
    {
        return Err("Hunted Horror body or exact {B}{B} payment differs".into());
    }
    if c.target != Some(PlayerId(1))
        || !c.trigger_stacked
        || !c.trigger_resolved
        || c.trigger_controller != Some(PlayerId(0))
        || c.target_decision_actor != Some(PlayerId(0))
        || c.target_decision_submissions != 1
        || c.source_incarnation.is_none()
        || c.source_incarnation != c.resolved_incarnation
    {
        return Err(
            "Hunted Horror ETB target decision was not submitted once by its trigger controller or did not resolve for the opponent and same source incarnation".into(),
        );
    }
    if c.tokens.len() != 2
        || c.tokens.iter().any(|t| {
            t.controller != PlayerId(1)
                || !t.green
                || !t.creature
                || !t.centaur
                || t.power != Some(3)
                || t.toughness != Some(3)
                || t.keywords != vec!["Protection(Black)"]
        })
        || c.centaurs_present_after_creation != vec![true, true]
    {
        return Err(format!(
            "Hunted Horror token mismatch: tokens={:?} present_after_creation={:?}",
            c.tokens, c.centaurs_present_after_creation
        ));
    }
    if !c.protection_probe_rejected || !c.protection_probe_no_state_change {
        return Err("a Centaur with protection from black was permitted to block Hunted Horror or changed state on rejection".into());
    }
    if !c.watchwolf_blocked || c.damage_to_watchwolf != 3 || c.damage_to_player < 4 {
        return Err("Hunted Horror did not demonstrate 3 lethal trample damage to a Watchwolf and at least 4 excess damage to its controller".into());
    }
    let paid = find_event(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == horror && colors == &vec![Color::Black, Color::Black])).ok_or("exact {B}{B} payment receipt is missing")?;
    let cast = find_event(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == horror)).ok_or("Hunted Horror spell cast receipt is missing")?;
    let resolved = find_event_after(
        &c.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == horror),
    )
    .ok_or("Hunted Horror spell resolution receipt is missing")?;
    let entered = find_event_after(&c.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == horror)).ok_or("Hunted Horror battlefield entry receipt is missing")?;
    let stacked = find_event_after(&c.events, entered, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == horror)).ok_or("Hunted Horror ETB trigger stack receipt is missing")?;
    let target_choice = find_event_after(&c.events, stacked, |event| matches!(event, GameEvent::PolicyMoveSubmitted { player: PlayerId(0), policy, kind: PolicyMoveKind::ChooseTriggeredAbilityTargets } if policy == "rav.hunted-horror-pilot.v1")).ok_or("Hunted Horror trigger controller target-submission receipt is missing")?;
    let first_token = find_event_after(&c.events, target_choice, |event| {
        matches!(
            event,
            GameEvent::TokenCreated {
                player: PlayerId(1),
                ..
            }
        )
    })
    .ok_or("opponent Centaur token event is missing")?;
    let ability = find_event_after(&c.events, first_token, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == horror)).ok_or("Hunted Horror ETB resolve receipt is missing")?;
    let attack = find_event_after(&c.events, ability, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&horror))).ok_or("Hunted Horror attack event is missing")?;
    let block = find_event_after(&c.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().any(|(attacker, blocker)| *attacker == horror && Some(*blocker) == c.watchwolf))).ok_or("Watchwolf did not block Hunted Horror")?;
    let damage = find_event_after(&c.events, block, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == horror && Some(*permanent) == c.watchwolf)).ok_or("Hunted Horror did not assign lethal trample damage to Watchwolf")?;
    let trample = find_event_after(&c.events, damage, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == horror)).ok_or("Hunted Horror did not deal the four excess trample damage to the defending player")?;
    let end = find_event_after(&c.events, trample, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win event missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < stacked
        && stacked < first_token
        && stacked < target_choice
        && target_choice < first_token
        && first_token < ability
        && ability < attack
        && attack < block
        && block < damage
        && damage < trample
        && trample < end)
    {
        return Err(
            "Hunted Horror cast, ETB, token, and trample combat event order differs".into(),
        );
    }
    Ok(())
}

fn mutation_detected(mutation: Mutation) -> bool {
    run_match(mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}
fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn trace_json(c: &Captured) -> Value {
    let horror = c.horror;
    let projected = project_events(&c.events);
    let events: Vec<_> = c.events.iter().enumerate().filter_map(|(index, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => horror == Some(*card) || c.watchwolf == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Battlefield } => horror == Some(*card),
            GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } | GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } => horror == Some(*source),
            GameEvent::PolicyMoveSubmitted { policy, kind: PolicyMoveKind::ChooseTriggeredAbilityTargets, .. } => policy == "rav.hunted-horror-pilot.v1",
            GameEvent::TokenCreated { token, .. } => c.tokens.iter().any(|item| item.object == *token),
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => horror.is_some_and(|id| attackers.contains(&id)),
            GameEvent::BlockersDeclared { player: PlayerId(1), assignments } => horror.is_some_and(|id| assignments.iter().any(|(attacker, _)| *attacker == id)),
            GameEvent::DamageDealtToPermanent { source, .. } => horror == Some(*source),
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } => horror == Some(*source),
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let item: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence":item.sequence,"turn":item.turn,"kind":item.kind,"facts":item.facts,"canonical":item.canonical}))
    }).collect();
    let ordered_event_sequence: Vec<_> = events
        .iter()
        .filter_map(|event| event.get("sequence").cloned())
        .collect();
    let digest = event_digest(&c.events);
    let tokens: Vec<_> = c.tokens.iter().map(|token| json!({"object":token.object.0,"controller_seat":token.controller.0,"green":token.green,"creature":token.creature,"centaur_subtype":token.centaur,"power":token.power,"toughness":token.toughness,"keywords":token.keywords,"present_after_creation":c.centaurs_present_after_creation.get(c.tokens.iter().position(|current| current == token).unwrap_or(usize::MAX)).copied().unwrap_or(false)})).collect();
    json!({"id":"rav-hunted-horror-opponent-centaurs-trample","seed":SEED,"digest":digest,"ordered_event_sequence":ordered_event_sequence,"passed":true,"terminal":c.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"horror_object":horror.map(|id|id.0),"horror":{"mana_cost":"{B}{B}","paid_colors":c.paid_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":c.power,"toughness":c.toughness,"trample":c.trample,"printed_subtype":"Horror","subtype_asserted":false},"etb":{"source_incarnation":c.source_incarnation,"resolved_incarnation":c.resolved_incarnation,"target_seat":c.target.map(|p|p.0),"trigger_controller_seat":c.trigger_controller.map(|p|p.0),"target_decision_actor_seat":c.target_decision_actor.map(|p|p.0),"target_decision_submissions":c.target_decision_submissions,"tokens":tokens},"combat":{"protection_block_probe_rejected":c.protection_probe_rejected,"probe_state_unchanged":c.protection_probe_no_state_change,"watchwolf_object":c.watchwolf.map(|id|id.0),"watchwolf_blocked":c.watchwolf_blocked,"damage_to_watchwolf":c.damage_to_watchwolf,"damage_to_player":c.damage_to_player},"events":events})
}

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    for event in events {
        let _ = writeln!(text, "{event:?}");
    }
    let hash = text
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 90 && card.name == CARD_NAME && id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
