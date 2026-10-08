//! Full-game RAV Transluminant task authority.
//!
//! Two legal constructed matches cast Transluminant and destroy it with
//! Putrefy. The controller accepts the dies trigger in one match and declines
//! it in the other; typed events and live token characteristics distinguish
//! those outcomes.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CreatureSubtype, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TokenSpec, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, load_constructed_decks,
    rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/transluminant.rs";
const DECK_P0: &str = "rav_selesnya_midrange";
const DECK_P1: &str = "rav_golgari_midrange";
const MAX_TRACE_SEED: u64 = 64;
const CANDIDATE_ID: &str = "RAV-TRANSLUMINANT";
const PUTREFY_ID: &str = "RAV-PUTREFY";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/transluminant.rs"]
mod candidate_transluminant;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Transluminant result: {error}");
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
            eprintln!("Transluminant authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    let (seed, accepted, declined) = find_traces()?;
    let accepted_trace = validate_trace(&accepted, seed, true)?;
    let declined_trace = validate_trace(&declined, seed, false)?;
    for (capture, accept_trigger) in [(&accepted, true), (&declined, false)] {
        let replay = run_match(
            seed,
            candidate_transluminant::definition(),
            false,
            false,
            accept_trigger,
        )?;
        if capture.events != replay.events
            || capture.terminal != replay.terminal
            || capture.winner != replay.winner
            || capture.transluminant != replay.transluminant
            || capture.token != replay.token
            || capture.token_characteristics != replay.token_characteristics
            || project_events(&capture.events) != project_events(&replay.events)
        {
            return Err("Transluminant full-game replay was not deterministic".to_owned());
        }
    }
    let mutant_results = json!({
        "power_mutation_detected_in_game": mutation_detected(Mutation::Power, seed),
        "dies_trigger_removal_detected_in_game": mutation_detected(Mutation::RemoveDiesTrigger, seed),
        "saproling_token_mutation_detected_in_game": mutation_detected(Mutation::WrongToken, seed),
        "optional_choice_decline_verified": true,
        "sealed_mutation_authority": false
    });
    if mutant_results["power_mutation_detected_in_game"] != true
        || mutant_results["dies_trigger_removal_detected_in_game"] != true
        || mutant_results["saproling_token_mutation_detected_in_game"] != true
    {
        return Err("a Transluminant semantic mutation escaped the trace oracle".to_owned());
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 186, "definition_id": CANDIDATE_ID, "name": "Transluminant"},
        "candidate": {"kind": candidate_kind(), "card_definition": CANDIDATE_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "110 and 111: permanents, tokens, and token characteristics",
                "117: timing and priority",
                "301-302 and 601.2: creature spell casting and resolution",
                "603.2 and 603.6c: dies-trigger event and source incarnation",
                "603.5: optional triggered ability resolution",
                "608.2: resolving spells and abilities",
                "704.5f: lethal damage state-based action",
            ],
            "source_contract": "RAV printed identity, green 2/2 body, and optional dies trigger creating one white 1/1 flying Spirit",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": mutant_results,
        "traces": [accepted_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Transluminant source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Transluminant source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Transluminant source has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TokenSpec"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = std::collections::BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Transluminant source contains an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(item) => {
                if functions.insert(item.sig.ident.to_string(), item).is_some() {
                    return Err("Transluminant source contains a duplicate function".to_owned());
                }
            }
            _ => return Err("Transluminant source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected_imports || functions.len() != 2 || file.items.len() != 4 {
        return Err("Transluminant imports or module structure differ from contract".to_owned());
    }
    let definition = functions
        .get("definition")
        .ok_or("Transluminant definition() is missing")?;
    check_plain_function(definition, "definition", "CardDefinition")?;
    let Stmt::Expr(Expr::Struct(card), None) = &definition.block.stmts[0] else {
        return Err("Transluminant definition must return one direct card literal".to_owned());
    };
    let fields = checked_struct_fields(card, &["CardDefinition"], 13)?;
    check_string(fields["id"], CANDIDATE_ID)?;
    check_string(fields["name"], "Transluminant")?;
    check_string(fields["set_code"], "RAV")?;
    check_cost_source(fields["mana_cost"])?;
    check_enum_set(fields["colors"], "Color", &["Green"])?;
    check_empty_set(fields["mana_colors"])?;
    check_enum_set(fields["card_types"], "CardType", &["Creature"])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_i16(fields["power"], &[1, 2])?;
    check_some_i16(fields["toughness"], &[1, 2])?;
    check_empty_vec(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    let trigger = functions
        .get("triggered_ability")
        .ok_or("Transluminant triggered_ability() is missing")?;
    validate_trigger_source(trigger)?;
    Ok(())
}

fn check_plain_function(function: &syn::ItemFn, name: &str, output: &str) -> Result<(), String> {
    let safe_attrs = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if !safe_attrs
        || !matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
        || function.sig.ident != name
        || !function.sig.inputs.is_empty()
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &[output]))
        || function.block.stmts.len() != 1
    {
        return Err(format!("Transluminant source must define plain {name}()"));
    }
    Ok(())
}

fn validate_trigger_source(function: &syn::ItemFn) -> Result<(), String> {
    check_plain_function(function, "triggered_ability", "TriggeredAbilityBinding")?;
    let Stmt::Expr(Expr::Struct(binding), None) = &function.block.stmts[0] else {
        return Err("Transluminant trigger must be a direct binding literal".to_owned());
    };
    let binding_fields = checked_struct_fields(binding, &["TriggeredAbilityBinding"], 2)?;
    check_string(binding_fields["card_definition"], CANDIDATE_ID)?;
    let Expr::Struct(ability) = binding_fields["ability"] else {
        return Err("Transluminant ability must be a direct literal".to_owned());
    };
    let fields = checked_struct_fields(ability, &["TriggeredAbility"], 6)?;
    check_string(fields["id"], "dies-create-flying-spirit")?;
    if !matches!(fields["condition"], Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["TriggerCondition", "Dies"]))
        || !matches!(fields["optional"], Expr::Lit(value) if matches!(&value.lit, Lit::Bool(lit) if lit.value))
        || !matches!(fields["targets"], Expr::Macro(mac) if mac.attrs.is_empty() && mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty())
    {
        return Err("Transluminant trigger must be an optional untargeted dies ability".to_owned());
    }
    let cost = call_args(fields["mana_cost"], &["ManaCost", "new"])
        .ok_or("Transluminant trigger cost must be zero")?;
    if cost.len() != 1 || !integer_is(cost[0], 0) {
        return Err("Transluminant trigger cost must be zero".to_owned());
    }
    let Expr::Macro(effects) = fields["effects"] else {
        return Err("Transluminant trigger effect must be a literal vector".to_owned());
    };
    if !effects.attrs.is_empty() || !effects.mac.path.is_ident("vec") {
        return Err("Transluminant trigger effect differs".to_owned());
    }
    let effect: Expr = syn::parse2(effects.mac.tokens.clone())
        .map_err(|_| "Transluminant trigger effect is malformed".to_owned())?;
    let Expr::Struct(effect) = effect else {
        return Err("Transluminant trigger must create one Spirit token".to_owned());
    };
    let effect_fields = checked_struct_fields(&effect, &["Effect", "CreateToken"], 2)?;
    if !integer_is(effect_fields["count"], 1)
        || !matches!(effect_fields["token"], Expr::Call(call) if path_is_expr(&call.func, &["TokenSpec", "white_spirit"]) && call.args.is_empty())
    {
        return Err("Transluminant trigger token must be one white 1/1 flying Spirit".to_owned());
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let runtime = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CANDIDATE_ID)
        .ok_or("Transluminant dies binding is missing from the RAV registry")?;
    if candidate_transluminant::triggered_ability() != runtime {
        return Err(
            "Transluminant candidate binding differs from the executable RAV binding".to_owned(),
        );
    }
    Ok(())
}

fn find_traces() -> Result<(u64, CapturedMatch, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_TRANSLUMINANT_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_TRANSLUMINANT_SEED must be unsigned".to_owned())?;
        let accepted = run_match(
            seed,
            candidate_transluminant::definition(),
            false,
            false,
            true,
        )?;
        validate_trace(&accepted, seed, true)?;
        let declined = run_match(
            seed,
            candidate_transluminant::definition(),
            false,
            false,
            false,
        )?;
        validate_trace(&declined, seed, false)?;
        return Ok((seed, accepted, declined));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(accepted) = run_match(
            seed,
            candidate_transluminant::definition(),
            false,
            false,
            true,
        ) && validate_trace(&accepted, seed, true).is_ok()
            && let Ok(declined) = run_match(
                seed,
                candidate_transluminant::definition(),
                false,
                false,
                false,
            )
            && validate_trace(&declined, seed, false).is_ok()
        {
            return Ok((seed, accepted, declined));
        }
    }
    Err(format!(
        "no Transluminant full-game traces found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone, Copy)]
enum Mutation {
    Power,
    RemoveDiesTrigger,
    WrongToken,
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    let mut definition = candidate_transluminant::definition();
    let remove_trigger = matches!(mutation, Mutation::RemoveDiesTrigger);
    let wrong_token = matches!(mutation, Mutation::WrongToken);
    if matches!(mutation, Mutation::Power) {
        definition.power = Some(1);
    }
    let expected = match mutation {
        Mutation::Power => "resolved Transluminant is not a 2/2",
        Mutation::RemoveDiesTrigger => "Transluminant optional dies choice is missing or incorrect",
        Mutation::WrongToken => "Transluminant trigger token is not a white 1/1 flying Spirit",
    };
    matches!(run_match(seed, definition, remove_trigger, wrong_token, true).and_then(|capture| validate_trace(&capture, seed, true).map(|_| ())), Err(error) if error.contains(expected))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    transluminant: Option<ObjectId>,
    putrefy: Option<ObjectId>,
    mana_paid_transluminant: Vec<Color>,
    mana_paid_putrefy: Vec<Color>,
    resolved_power_toughness: Option<(i32, i32)>,
    resolved_cost: Option<ManaCost>,
    source_in_graveyard_before_choice: Option<bool>,
    optional_choice_seen: bool,
    optional_can_pay: Option<bool>,
    optional_accepted: bool,
    token: Option<ObjectId>,
    token_characteristics: Option<(BTreeSet<Color>, BTreeSet<CreatureSubtype>, i32, i32, bool)>,
}

fn task_deck(base: &DeckList) -> Result<DeckList, String> {
    let mut deck = base.clone();
    let entry = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-LOXODON-HIERARCH")
        .ok_or("Selesnya deck no longer has its four Loxodon Hierarch slots")?;
    if entry.count != 4 {
        return Err("Selesnya task deck replacement must preserve a four-of slot".to_owned());
    }
    entry.card = CANDIDATE_ID.to_owned();
    entry.count = 1;
    let forests = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-FOREST")
        .ok_or("Selesnya deck no longer has its basic Forest slot")?;
    forests.count = forests
        .count
        .checked_add(3)
        .ok_or("Transluminant task deck Forest count overflow")?;
    let size: usize = deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if size != 60 {
        return Err(format!(
            "Transluminant task deck has {size} cards, expected 60"
        ));
    }
    Ok(deck)
}

fn run_match(
    seed: u64,
    definition: CardDefinition,
    remove_trigger: bool,
    wrong_token: bool,
    accept_trigger: bool,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let source0 = decks
        .iter()
        .find(|deck| deck.id == DECK_P0)
        .ok_or("Selesnya deck missing")?;
    let source1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Golgari deck missing")?;
    let deck0 = task_deck(&source0.deck)?;
    let deck1 = source1.deck.clone();
    if deck1
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
    {
        return Err(format!("{} is not a 60-card deck", DECK_P1));
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_rav_game(definition, remove_trigger, wrong_token)?;
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
        Box::new(TransluminantPolicy::new(PlayerId(0), accept_trigger)),
        Box::new(PutrefyTaskPolicy::new(PlayerId(1))),
    ];
    let mut attempted = 0_u32;
    let mut accepted_moves = 0_u32;
    let mut transluminant = None;
    let mut putrefy = None;
    let mut mana_paid_transluminant = Vec::new();
    let mut mana_paid_putrefy = Vec::new();
    let mut resolved_power_toughness = None;
    let mut resolved_cost = None;
    let mut source_in_graveyard_before_choice = None;
    let mut optional_choice_seen = false;
    let mut optional_can_pay = None;
    let mut optional_accepted = false;
    let mut token = None;
    let mut token_characteristics = None;
    let mut trigger_position = None;
    let mut ability_resolved_position = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Transluminant game hit a run bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let visible_candidate = view
            .own_battlefield
            .iter()
            .chain(view.hand.iter())
            .chain(view.opponent_battlefield.iter())
            .find(|card| card.definition == Some(CANDIDATE_ID))
            .map(|card| card.id);
        if let Some(choice) = &view.optional_triggered_ability_choice
            && choice.ability == "dies-create-flying-spirit"
            && Some(choice.source) == transluminant.or(visible_candidate)
        {
            optional_choice_seen = true;
            optional_can_pay = Some(choice.can_pay);
            source_in_graveyard_before_choice =
                Some(game.zone_of(choice.source) == Some(Zone::Graveyard));
        }
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
        } else {
            policy.propose_move(&view)
        };
        if let PolicyAction::ResolveOptionalTriggeredAbility {
            source,
            ability: "dies-create-flying-spirit",
            pay,
            ..
        } = &action
            && Some(*source) == transluminant
        {
            optional_accepted = *pay;
        }
        let old_event_count = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("Transluminant policy move rejected: {error}"))?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for (offset, event) in game.event_log[old_event_count..].iter().enumerate() {
            let position = old_event_count + offset;
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    transluminant = Some(*card);
                    mana_paid_transluminant = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    transluminant = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == PUTREFY_ID) =>
                {
                    putrefy = Some(*card);
                    mana_paid_putrefy = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == PUTREFY_ID) =>
                {
                    putrefy = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == transluminant => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    resolved_power_toughness = Some((
                        characteristics
                            .power
                            .ok_or("Transluminant has no live power")?,
                        characteristics
                            .toughness
                            .ok_or("Transluminant has no live toughness")?,
                    ));
                    resolved_cost = Some(
                        game.card_definition(*card)
                            .map_err(|error| error.to_string())?
                            .mana_cost
                            .clone(),
                    );
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: "dies-create-flying-spirit",
                    ..
                } if Some(*source) == transluminant => trigger_position = Some(position),
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token: created,
                } if trigger_position.is_some() && ability_resolved_position.is_none() => {
                    if token.replace(*created).is_some() {
                        return Err("Transluminant trigger created more than one token".to_owned());
                    }
                    let characteristics = game
                        .characteristics(*created)
                        .map_err(|error| error.to_string())?;
                    let object = game.object(*created).map_err(|error| error.to_string())?;
                    let token_spec = object
                        .token
                        .as_ref()
                        .ok_or("created object is not a token")?;
                    token_characteristics = Some((
                        characteristics.colors,
                        characteristics.creature_subtypes,
                        characteristics.power.ok_or("Spirit token has no power")?,
                        characteristics
                            .toughness
                            .ok_or("Spirit token has no toughness")?,
                        characteristics.keywords.contains(&Keyword::Flying)
                            && token_spec.name == "Spirit",
                    ));
                }
                GameEvent::AbilityResolved {
                    source,
                    ability: "dies-create-flying-spirit",
                    ..
                } if Some(*source) == transluminant => ability_resolved_position = Some(position),
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted.saturating_sub(accepted_moves),
        transluminant,
        putrefy,
        mana_paid_transluminant,
        mana_paid_putrefy,
        resolved_power_toughness,
        resolved_cost,
        source_in_graveyard_before_choice,
        optional_choice_seen,
        optional_can_pay,
        optional_accepted,
        token,
        token_characteristics,
    })
}

fn new_rav_game(
    definition: CardDefinition,
    remove_trigger: bool,
    wrong_token: bool,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|card| card.id == CANDIDATE_ID)
        .ok_or("Transluminant definition missing from the RAV registry")?;
    *candidate = definition;
    let mut triggers = rav_triggered_ability_bindings();
    if remove_trigger {
        triggers.retain(|binding| {
            !(binding.card_definition == CANDIDATE_ID
                && binding.ability.id == "dies-create-flying-spirit")
        });
    }
    if wrong_token {
        for binding in &mut triggers {
            if binding.card_definition == CANDIDATE_ID
                && binding.ability.id == "dies-create-flying-spirit"
            {
                binding.ability.effects = vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }];
            }
        }
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    Ok(game)
}

struct TransluminantPolicy {
    player: PlayerId,
    accept_trigger: bool,
    candidate_seen: bool,
    inner: RavCatalogPolicy,
}
impl TransluminantPolicy {
    fn new(player: PlayerId, accept_trigger: bool) -> Self {
        Self {
            player,
            accept_trigger,
            candidate_seen: false,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}
impl CodePolicy for TransluminantPolicy {
    fn id(&self) -> &'static str {
        "rav.transluminant-optional-spirit-choice.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let candidate_present = view
            .hand
            .iter()
            .chain(view.own_battlefield.iter())
            .any(|card| card.definition == Some(CANDIDATE_ID));
        self.candidate_seen |= candidate_present;
        if view.step == Step::DeclareAttackers
            && view.player == self.player
            && view.active_player == self.player
            && !view.attackers_declared
            && (!self.candidate_seen || candidate_present)
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player == self.player
            && view.decision_player == self.player
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0 {
                let action = self.inner.propose_move(view);
                if matches!(
                    action,
                    PolicyAction::PlayLand { .. }
                        | PolicyAction::PlayLandWithEntryLifePayment { .. }
                ) {
                    return action;
                }
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CANDIDATE_ID))
            {
                let green = view.mana_pool.amount(Color::Green);
                if green > 0 && view.mana_pool.total() >= 2 {
                    let generic = [
                        Color::White,
                        Color::Green,
                        Color::Blue,
                        Color::Black,
                        Color::Red,
                        Color::Colorless,
                    ]
                    .into_iter()
                    .find(|color| {
                        view.mana_pool.amount(*color)
                            > u8::from(*color == Color::Green && green < 2)
                    });
                    if let Some(generic) = generic {
                        return PolicyAction::CastWithPayment {
                            request: cardbench_magic_engine::CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: ManaPaymentSelection {
                                generic: vec![generic],
                                hybrid: vec![],
                            },
                        };
                    }
                }
                let needed_color = if green == 0 {
                    Some(Color::Green)
                } else if view.mana_pool.total() < 2 {
                    [
                        Color::White,
                        Color::Green,
                        Color::Blue,
                        Color::Black,
                        Color::Red,
                    ]
                    .into_iter()
                    .find(|color| {
                        view.own_battlefield.iter().any(|land| {
                            !land.tapped
                                && land.card_types.contains(&CardType::Land)
                                && land.mana_colors.contains(color)
                        })
                    })
                } else {
                    None
                };
                if let Some(color) = needed_color
                    && let Some(land) = view.own_battlefield.iter().find(|land| {
                        !land.tapped
                            && land.card_types.contains(&CardType::Land)
                            && land.mana_colors.contains(&color)
                    })
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color,
                    };
                }
            }
        }
        self.inner.propose_move(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("optional choice visible");
        if choice.ability == "dies-create-flying-spirit" {
            PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: self.accept_trigger,
                target: None,
            }
        } else {
            self.inner.propose_optional_triggered_ability(view)
        }
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
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

struct PutrefyTaskPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
}
impl PutrefyTaskPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
    fn putrefy_action(&self, view: &GameView) -> Option<PolicyAction> {
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return None;
        }
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(CANDIDATE_ID))?;
        let putrefy = view
            .hand
            .iter()
            .find(|card| card.definition == Some(PUTREFY_ID))?;
        let black = view.mana_pool.amount(Color::Black);
        let green = view.mana_pool.amount(Color::Green);
        if black >= 1 && green >= 1 && u16::from(black) + u16::from(green) >= 3 {
            let generic_color = if green >= 2 {
                Color::Green
            } else {
                Color::Black
            };
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: putrefy.id,
                    targets: vec![Target::Permanent(target.id)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![generic_color],
                    hybrid: vec![],
                },
            });
        }
        let needed_color = if black == 0 {
            Some(Color::Black)
        } else if green == 0 {
            Some(Color::Green)
        } else if u16::from(black) + u16::from(green) < 3 {
            Some(Color::Green)
        } else {
            None
        }?;
        let land = view.own_battlefield.iter().find(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && land.mana_colors.contains(&needed_color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: needed_color,
        })
    }
}
impl CodePolicy for PutrefyTaskPolicy {
    fn id(&self) -> &'static str {
        "rav.transluminant-golgari-putrefy.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.player == self.player
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player == self.player && view.active_player == self.player && view.lands_played == 0
        {
            let action = self.inner.propose_move(view);
            if matches!(
                action,
                PolicyAction::PlayLand { .. } | PolicyAction::PlayLandWithEntryLifePayment { .. }
            ) {
                return action;
            }
        }
        if let Some(action) = self.putrefy_action(view) {
            return action;
        }
        self.inner.propose_move(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
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

fn validate_trace(
    capture: &CapturedMatch,
    seed: u64,
    accept_trigger: bool,
) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Transluminant game did not reach a clean seat-zero win".to_owned());
    }
    let source = capture.transluminant.ok_or("Transluminant was not cast")?;
    let putrefy = capture
        .putrefy
        .ok_or("Putrefy was not cast at Transluminant")?;
    if capture.resolved_power_toughness != Some((2, 2)) {
        return Err("resolved Transluminant is not a 2/2".to_owned());
    }
    if capture.resolved_cost.as_ref() != Some(&ManaCost::with_colors(1, [Color::Green]))
        || capture.mana_paid_transluminant.len() != 2
        || !capture.mana_paid_transluminant.contains(&Color::Green)
    {
        return Err(format!(
            "Transluminant did not pay its green creature cost: {:?} with cost {:?}",
            capture.mana_paid_transluminant, capture.resolved_cost
        ));
    }
    if capture.mana_paid_putrefy.len() != 3
        || !capture.mana_paid_putrefy.contains(&Color::Black)
        || !capture.mana_paid_putrefy.contains(&Color::Green)
    {
        return Err("Putrefy did not pay its black, green, and generic cost".to_owned());
    }
    if !capture.optional_choice_seen
        || capture.optional_can_pay != Some(true)
        || capture.optional_accepted != accept_trigger
        || capture.source_in_graveyard_before_choice != Some(true)
    {
        return Err("Transluminant optional dies choice is missing or incorrect".to_owned());
    }
    let positions = event_positions(&capture.events, source, putrefy, accept_trigger)?;
    if !(positions.transluminant_payment < positions.transluminant_cast
        && positions.transluminant_cast < positions.transluminant_resolved
        && positions.transluminant_resolved < positions.entry
        && positions.entry < positions.putrefy_payment
        && positions.putrefy_payment < positions.putrefy_cast
        && positions.putrefy_cast < positions.destroyed
        && positions.destroyed < positions.source_graveyard
        && positions.source_graveyard < positions.putrefy_resolved
        && positions.putrefy_resolved < positions.trigger
        && positions.trigger < positions.ability_resolved
        && positions.ability_resolved < positions.choice_receipt
        && positions.choice_receipt < positions.game_end)
    {
        return Err(
            "Transluminant cast, Putrefy, dies trigger, token, or choice event order differs"
                .to_owned(),
        );
    }
    let token = if accept_trigger {
        let token = capture
            .token
            .ok_or("accepted Transluminant trigger created no token")?;
        if capture.token_characteristics.as_ref().is_none_or(
            |(colors, subtypes, power, toughness, flying)| {
                colors != &BTreeSet::from([Color::White])
                    || subtypes != &BTreeSet::from([CreatureSubtype::Spirit])
                    || *power != 1
                    || *toughness != 1
                    || !*flying
            },
        ) {
            return Err("Transluminant trigger token is not a white 1/1 flying Spirit".to_owned());
        }
        Some(token)
    } else {
        if capture.token.is_some() {
            return Err("declined Transluminant trigger unexpectedly created a token".to_owned());
        }
        None
    };
    let projection = project_events(&capture.events);
    let mut selected = vec![
        positions.transluminant_payment,
        positions.transluminant_cast,
        positions.transluminant_resolved,
        positions.entry,
        positions.putrefy_payment,
        positions.putrefy_cast,
        positions.destroyed,
        positions.source_graveyard,
        positions.putrefy_resolved,
        positions.trigger,
    ];
    if let Some(token_created) = positions.token_created {
        selected.push(token_created);
    }
    selected.extend([
        positions.ability_resolved,
        positions.choice_receipt,
        positions.game_end,
    ]);
    let projected: Vec<_> = selected
        .iter()
        .map(|index| projection[*index].clone())
        .collect();
    let mut expected = vec![
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "CardDestroyed",
        "CardMoved",
        "SpellResolved",
        "TriggeredAbilityStacked",
    ];
    if accept_trigger {
        expected.push("TokenCreated");
    }
    expected.extend(["AbilityResolved", "PolicyMoveSubmitted", "GameEnded"]);
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    if kinds != expected {
        return Err(format!(
            "Transluminant typed event trace differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": if accept_trigger { "rav-transluminant-optional-spirit-accepted" } else { "rav-transluminant-optional-spirit-declined" },
        "seed": seed,
        "decks": [format!("{DECK_P0}+1-transluminant"), DECK_P1],
        "policies": ["rav.transluminant-optional-spirit-choice.v1", "rav.transluminant-golgari-putrefy.v1"],
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "digest": event_digest(&capture.events),
        "transluminant_object": source.0,
        "putrefy_object": putrefy.0,
        "mana_paid": {"transluminant": capture.mana_paid_transluminant.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>(), "putrefy": capture.mana_paid_putrefy.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>()},
        "resolved_characteristics": {"power": 2, "toughness": 2},
        "optional_trigger": {"source_object": source.0, "choice_seen": capture.optional_choice_seen, "can_pay": capture.optional_can_pay, "choice": if accept_trigger { "accepted" } else { "declined" }, "source_in_graveyard_before_choice": capture.source_in_graveyard_before_choice, "token_object": token.map(|token| token.0), "token_characteristics": if accept_trigger { json!({"name":"Spirit","colors":["White"],"subtypes":["Spirit"],"power":1,"toughness":1,"flying":true}) } else { Value::Null }},
        "events": projected,
        "passed": true
    }))
}

struct EventPositions {
    transluminant_payment: usize,
    transluminant_cast: usize,
    transluminant_resolved: usize,
    entry: usize,
    putrefy_payment: usize,
    putrefy_cast: usize,
    putrefy_resolved: usize,
    destroyed: usize,
    source_graveyard: usize,
    trigger: usize,
    token_created: Option<usize>,
    ability_resolved: usize,
    choice_receipt: usize,
    game_end: usize,
}
fn event_positions(
    events: &[GameEvent],
    source: ObjectId,
    putrefy: ObjectId,
    accept: bool,
) -> Result<EventPositions, String> {
    let pos = |predicate: &dyn Fn(&GameEvent) -> bool, start: usize, label: &str| {
        events
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(i, e)| predicate(e).then_some(i))
            .ok_or_else(|| format!("{label} event missing"))
    };
    let transluminant_payment = pos(
        &|e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 2 && colors.contains(&Color::Green)),
        0,
        "Transluminant payment",
    )?;
    let transluminant_cast = pos(
        &|e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source),
        transluminant_payment + 1,
        "Transluminant cast",
    )?;
    let transluminant_resolved = pos(
        &|e| matches!(e, GameEvent::SpellResolved { card } if *card == source),
        transluminant_cast + 1,
        "Transluminant resolution",
    )?;
    let entry = pos(
        &|e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source),
        transluminant_resolved + 1,
        "Transluminant entry",
    )?;
    let putrefy_payment = pos(
        &|e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == putrefy && colors.len() == 3 && colors.contains(&Color::Black) && colors.contains(&Color::Green)),
        entry + 1,
        "Putrefy payment",
    )?;
    let putrefy_cast = pos(
        &|e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if *card == putrefy),
        putrefy_payment + 1,
        "Putrefy cast",
    )?;
    let destroyed = pos(
        &|e| matches!(e, GameEvent::CardDestroyed { source: spell, card } if *spell == putrefy && *card == source),
        putrefy_cast + 1,
        "Putrefy destruction",
    )?;
    let source_graveyard = pos(
        &|e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source),
        destroyed + 1,
        "Transluminant graveyard move",
    )?;
    let putrefy_resolved = pos(
        &|e| matches!(e, GameEvent::SpellResolved { card } if *card == putrefy),
        source_graveyard + 1,
        "Putrefy resolution",
    )?;
    let trigger = pos(
        &|e| matches!(e, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: trigger_source, ability: "dies-create-flying-spirit", .. } if *trigger_source == source),
        putrefy_resolved + 1,
        "Transluminant dies trigger",
    )?;
    let ability_resolved = pos(
        &|e| matches!(e, GameEvent::AbilityResolved { source: trigger_source, ability: "dies-create-flying-spirit", .. } if *trigger_source == source),
        trigger + 1,
        "Transluminant trigger resolution",
    )?;
    let token_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .skip(trigger + 1)
        .take(ability_resolved.saturating_sub(trigger + 1))
        .filter_map(|(i, e)| {
            matches!(
                e,
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    ..
                }
            )
            .then_some(i)
        })
        .collect();
    let token_created = token_positions.first().copied();
    if token_positions.len() != usize::from(accept) {
        return Err(
            "Transluminant optional trigger created an unexpected number of tokens".to_owned(),
        );
    }
    let choice_receipt = pos(
        &|e| matches!(e, GameEvent::PolicyMoveSubmitted { player: PlayerId(0), policy, kind } if policy == "rav.transluminant-optional-spirit-choice.v1" && *kind == cardbench_magic_engine::PolicyMoveKind::ResolveOptionalTriggeredAbility),
        ability_resolved + 1,
        "optional token-choice receipt",
    )?;
    let game_end = pos(
        &|e| {
            matches!(
                e,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        },
        choice_receipt + 1,
        "terminal win",
    )?;
    Ok(EventPositions {
        transluminant_payment,
        transluminant_cast,
        transluminant_resolved,
        entry,
        putrefy_payment,
        putrefy_cast,
        putrefy_resolved,
        destroyed,
        source_graveyard,
        trigger,
        token_created,
        ability_resolved,
        choice_receipt,
        game_end,
    })
}

fn checked_struct_fields<'a>(
    structure: &'a syn::ExprStruct,
    path: &[&str],
    count: usize,
) -> Result<std::collections::BTreeMap<String, &'a Expr>, String> {
    if !path_is(&structure.path, path)
        || structure.fields.len() != count
        || structure.rest.is_some()
    {
        return Err("Transluminant struct literal differs from its source contract".to_owned());
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &structure.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Transluminant fields must be named".to_owned());
        };
        if field.colon_token.is_none()
            || !field.attrs.is_empty()
            || fields.insert(name.to_string(), &field.expr).is_some()
        {
            return Err("Transluminant struct fields differ".to_owned());
        }
    }
    Ok(fields)
}
fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path,expected))
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(part, name)| {
            part.ident == *name && matches!(&part.arguments, PathArguments::None)
        })
}
fn path_is_expr(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}
fn call_args<'a>(expr: &'a Expr, expected: &[&str]) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else {
        return None;
    };
    (path_is_expr(call.func.as_ref(), expected)).then(|| call.args.iter().collect())
}
fn integer_is(expr: &Expr, expected: u8) -> bool {
    matches!(expr,Expr::Lit(value) if matches!(&value.lit,Lit::Int(number) if number.base10_parse::<u8>().is_ok_and(|number| number==expected)))
}
fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr,Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit,Lit::Str(lit) if lit.value()==expected))
    {
        Ok(())
    } else {
        Err(format!("Transluminant string field must be {expected:?}"))
    }
}
fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr,Expr::Lit(value) if matches!(&value.lit,Lit::Bool(lit) if lit.value==expected))
    {
        Ok(())
    } else {
        Err("Transluminant boolean field differs".to_owned())
    }
}
fn check_enum_set(expr: &Expr, kind: &str, values: &[&str]) -> Result<(), String> {
    let Some(args) = call_args(expr, &["BTreeSet", "from"]) else {
        return Err("Transluminant set must be literal".to_owned());
    };
    if args.len() != 1 {
        return Err("Transluminant set differs".to_owned());
    }
    let Expr::Array(array) = args[0] else {
        return Err("Transluminant set differs".to_owned());
    };
    let actual: Option<Vec<String>> = array
        .elems
        .iter()
        .map(|expr| {
            let Expr::Path(path) = expr else {
                return None;
            };
            let variant = path.path.segments.last()?.ident.to_string();
            (path.qself.is_none()
                && path.path.segments.len() == 2
                && path.path.segments.first()?.ident == kind
                && values.contains(&variant.as_str()))
            .then_some(variant)
        })
        .collect();
    if actual
        .as_deref()
        .is_some_and(|actual| actual.iter().map(String::as_str).eq(values.iter().copied()))
    {
        Ok(())
    } else {
        Err("Transluminant set differs".to_owned())
    }
}
fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_args(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Transluminant empty set differs".to_owned())
    }
}
fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    if matches!(expr,Expr::Macro(mac) if mac.attrs.is_empty()&&mac.mac.path.is_ident("vec")&&mac.mac.tokens.is_empty())
    {
        Ok(())
    } else {
        Err("Transluminant vector must be empty".to_owned())
    }
}
fn check_some_i16(expr: &Expr, values: &[i16]) -> Result<(), String> {
    let Some(args) = call_args(expr, &["Some"]) else {
        return Err("Transluminant statistic must be Some(integer)".to_owned());
    };
    if args.len() == 1
        && matches!(args[0],Expr::Lit(value) if matches!(&value.lit,Lit::Int(number) if number.base10_parse::<i16>().is_ok_and(|value|values.contains(&value))))
    {
        Ok(())
    } else {
        Err("Transluminant statistic differs".to_owned())
    }
}
fn check_cost_source(expr: &Expr) -> Result<(), String> {
    let args = call_args(expr, &["ManaCost", "with_colors"])
        .ok_or("Transluminant cost must use ManaCost::with_colors")?;
    if args.len() != 2 || !integer_is(args[0], 1) {
        return Err("Transluminant green mana cost differs".to_owned());
    }
    let Expr::Array(colors) = args[1] else {
        return Err("Transluminant colored cost differs".to_owned());
    };
    if colors.elems.len() == 1
        && matches!(colors.elems.first(),Some(Expr::Path(path)) if path_is(&path.path,&["Color","Green"]))
    {
        Ok(())
    } else {
        Err("Transluminant green mana cost differs".to_owned())
    }
}
fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Transluminant rules must be a static slice".to_owned());
    };
    let Expr::Array(array) = reference.expr.as_ref() else {
        return Err("Transluminant rules differ".to_owned());
    };
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "dies-create-flying-spirit",
    ];
    if array.elems.len() == expected.len()
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(item, expected)| check_string(item, expected).is_ok())
    {
        Ok(())
    } else {
        Err("Transluminant rules differ".to_owned())
    }
}
fn event_digest(events: &[GameEvent]) -> String {
    let mut text = String::new();
    for event in events {
        use std::fmt::Write as _;
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
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = id == Some(CANDIDATE_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":match card.semantic_status{CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn qualification_status() -> &'static str {
    if candidate_kind() == "candidate" {
        "candidate_pilot_pass; independent reviewer approval remains"
    } else {
        "reference_pilot_pass; independent review and candidate trial remain"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
