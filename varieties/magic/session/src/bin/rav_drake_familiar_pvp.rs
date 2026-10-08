//! Full-game diagnostic authority for RAV Drake Familiar.

use cardbench_magic_engine::{
    CardType, Color, DecisionKind, DecisionSelection, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TargetRequirement, TriggerCondition, Zone,
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
const CARD_ID: &str = "RAV-DRAKE-FAMILIAR";
const CARD_NAME: &str = "Drake Familiar";
const ENCHANTMENT_ID: &str = "RAV-DARK-HEART-OF-THE-WOOD";
const ABILITY_ID: &str = "etb-return-any-enchantment-or-sacrifice-source";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/drake_familiar.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const MAX_TRACE_SEED: u64 = 128;

// This is intentionally a byte-for-byte source contract. The candidate module
// is validated before either candidate-owned function is called.
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned definition and target-free ETB choice for Drake Familiar.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DRAKE-FAMILIAR",
        name: "Drake Familiar",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "etb-return-any-enchantment-or-sacrifice-source",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-DRAKE-FAMILIAR",
        ability: TriggeredAbility {
            id: "etb-return-any-enchantment-or-sacrifice-source",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::ReturnAnyEnchantmentOrSacrificeSource],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/drake_familiar.rs"]
mod candidate_drake_familiar;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Drake Familiar result: {error}");
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
            eprintln!("Drake Familiar PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn run_authority() -> Result<Value, String> {
    // Keep this order: no candidate function is called until the whole module
    // has passed exact-source and structural AST validation.
    validate_candidate_source()?;
    let side_effect_rejected = syntactically_parsed_side_effect_source_rejected()?;
    if !side_effect_rejected {
        return Err(
            "syntactically parsed candidate side effect was not rejected before execution".into(),
        );
    }
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;

    let (seed, reference) = (0..MAX_TRACE_SEED)
        .find_map(|seed| {
            let trace = run_match(Mutation::None, seed).ok()?;
            validate_trace(&trace).ok().map(|()| (seed, trace))
        })
        .ok_or("no bounded shuffled 60-card Drake Familiar duel passed the reference contract")?;
    let replay = run_match(Mutation::None, seed)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Drake Familiar full-game replay was not deterministic".into());
    }

    let mutations = json!({
        "mana_cost_changed_detected_in_game": mutation_detected(Mutation::WrongManaCost, seed),
        "body_changed_detected_in_game": mutation_detected(Mutation::WrongBody, seed),
        "flying_removed_detected_in_game": mutation_detected(Mutation::NoFlying, seed),
        "trigger_removed_detected_in_game": mutation_detected(Mutation::RemoveTrigger, seed),
        "trigger_condition_changed_detected_in_game": mutation_detected(Mutation::WrongCondition, seed),
        "targeted_instead_of_target_free_detected_in_game": mutation_detected(Mutation::Targeted, seed),
        "return_effect_changed_detected_in_game": mutation_detected(Mutation::WrongEffect, seed),
        "syntactically_parsed_candidate_side_effect_rejected_before_execution": side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "mana_cost_changed_detected_in_game",
        "body_changed_detected_in_game",
        "flying_removed_detected_in_game",
        "trigger_removed_detected_in_game",
        "trigger_condition_changed_detected_in_game",
        "targeted_instead_of_target_free_detected_in_game",
        "return_effect_changed_detected_in_game",
        "syntactically_parsed_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!("Drake Familiar gameplay mutation escaped: {key}"));
        }
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":44,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["117 Priority","302.1 Creature characteristics","603.2 and 603.3 ETB trigger and target selection","608.2 Resolving spells and abilities","701.17 Sacrifice","702.9 Flying"],
            "source_contract":"RAV #44 is a blue {1}{U} 2/1 Drake with flying. The RAV FAQ and its 2005 ruling establish a target-free resolution choice to return any battlefield enchantment, including an opponent's and an untargetable enchantment, to its owner's hand; declining or having none requires sacrificing Drake Familiar.",
            "oracle_reconciliation":"Current Oracle modernizes the old entry timing language and preserves the RAV FAQ's any-enchantment, non-targeting semantics.",
            "source_references":["https://www.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224968-rav-official-faq","https://mtg.wtf/card/rav/44/Drake-Familiar","https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87960"],
            "authority_cutoff":"2026-09-24"
        },
        "passed":true,
        "pilot_card_passed":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,
        "mutation_sensitivity":mutations,
        "traces":[trace_json(&reference, seed)],
        "per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Drake Familiar source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Drake Familiar source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
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
                let (name, ty) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                    _ => {
                        return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
                    }
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
                };
                if !return_type_is(&item.sig.output, ty)
                    || !path_is_type(&literal.path, ty)
                    || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
                }
            }
            _ => {
                return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
            }
        }
    }
    let expected =
        syn::parse_file(EXPECTED_CANDIDATE_SOURCE).expect("trusted source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || source != EXPECTED_CANDIDATE_SOURCE
        || expected.items.len() != 4
    {
        return Err("Drake Familiar candidate definition or trigger is outside its literal source and AST contract".into());
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

fn syntactically_parsed_side_effect_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Drake Familiar source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-drake-familiar-candidate-executed\", b\"executed\");";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "syntactically parsed Drake Familiar side-effect source could not be constructed"
                .into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_drake_familiar::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Drake Familiar definition is missing from the RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(2)
        || definition.toughness != Some(1)
        || definition.keywords != vec![Keyword::Flying]
        || definition.is_basic_land
        || !definition.effects.is_empty()
    {
        return Err("Drake Familiar definition is outside the printed-RAV facts".into());
    }
    let binding = candidate_drake_familiar::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == ABILITY_ID)
        .ok_or("Drake Familiar target-free ETB binding is missing from the RAV registry")?;
    if binding != registered
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.optional
        || !binding.ability.targets.is_empty()
        || binding.ability.effects != vec![Effect::ReturnAnyEnchantmentOrSacrificeSource]
    {
        return Err(
            "Drake Familiar ETB is outside the printed target-free return-or-sacrifice contract"
                .into(),
        );
    }
    Ok(())
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
    {
        return Err("RAV catalog denominator differs from the source contract".into());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    WrongBody,
    NoFlying,
    RemoveTrigger,
    WrongCondition,
    Targeted,
    WrongEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    seed: u64,
    drake: Option<ObjectId>,
    enchantment: Option<ObjectId>,
    paid_colors: Vec<Color>,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
    trigger_stacked: bool,
    trigger_resolved: bool,
    source_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    target_count: Option<usize>,
    choice_opened: bool,
    choice_min: Option<u8>,
    choice_max: Option<u8>,
    selected: Option<ObjectId>,
    choice_actor: Option<PlayerId>,
    choice_candidate_controllers: Vec<PlayerId>,
    returned: bool,
    enchantment_owner: Option<PlayerId>,
    enchantment_controller: Option<PlayerId>,
    response_priority_passed: bool,
    attacked: bool,
    combat_damage: i32,
}

fn run_match(mutation: Mutation, seed: u64) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|entry| entry.id == CARD_ID)
        .ok_or("Drake Familiar definition missing")?;
    *definition = candidate_drake_familiar::definition();
    match mutation {
        Mutation::WrongManaCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Blue]),
        Mutation::WrongBody => {
            definition.power = Some(3);
            definition.toughness = Some(1);
        }
        Mutation::NoFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        _ => {}
    }

    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|entry| entry.card_definition == CARD_ID && entry.ability.id == ABILITY_ID);
    match mutation {
        Mutation::RemoveTrigger => {
            if let Some(index) = trigger_index {
                triggers.remove(index);
            }
        }
        Mutation::WrongCondition | Mutation::Targeted | Mutation::WrongEffect => {
            let index = trigger_index.ok_or("Drake Familiar trigger missing for mutation")?;
            triggers[index] = candidate_drake_familiar::triggered_ability();
            match mutation {
                Mutation::WrongCondition => {
                    triggers[index].ability.condition = TriggerCondition::BeginningOfUpkeep;
                }
                Mutation::Targeted => {
                    triggers[index].ability.targets = vec![TargetRequirement::Enchantment];
                    triggers[index].ability.effects =
                        vec![Effect::ReturnTargetEnchantmentToOwnersHand];
                }
                Mutation::WrongEffect => {
                    triggers[index].ability.effects = vec![Effect::SacrificeControllerCreature];
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
                card: "RAV-ISLAND".into(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 18,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ENCHANTMENT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [
        deck0
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>(),
        deck1
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>(),
    ] != [60, 60]
    {
        return Err("Drake Familiar task decks must contain exactly 60 cards".into());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
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
        Box::new(DrakePolicy::new(PlayerId(0))),
        Box::new(DrakePolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        seed,
        drake: None,
        enchantment: None,
        paid_colors: vec![],
        power: None,
        toughness: None,
        flying: false,
        trigger_stacked: false,
        trigger_resolved: false,
        source_incarnation: None,
        resolved_incarnation: None,
        target_count: None,
        choice_opened: false,
        choice_min: None,
        choice_max: None,
        selected: None,
        choice_actor: None,
        choice_candidate_controllers: vec![],
        returned: false,
        enchantment_owner: None,
        enchantment_controller: None,
        response_priority_passed: false,
        attacked: false,
        combat_damage: 0,
    };
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted >= MAX_MOVES {
            return Err(format!(
                "Drake Familiar game exceeded bounds at turn {} step {:?}",
                game.turn, game.step
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if let Some(choice) = &view.triggered_ability_effect_object_choice {
            if choice.ability == ABILITY_ID {
                captured.choice_candidate_controllers = choice
                    .candidates
                    .iter()
                    .map(|candidate| candidate.controller)
                    .collect();
            }
        }
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.triggered_ability_target_choice.is_some() {
            policy.propose_move(&view)
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        if let PolicyAction::ChooseTriggeredAbilityEffectObject { selected, .. } = &action {
            captured.choice_actor = Some(player);
            captured.selected = *selected;
        }
        let previous = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Drake Familiar policy rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[previous..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|entry| entry.id == CARD_ID) =>
                {
                    captured.drake = Some(*card);
                    captured.paid_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|entry| entry.id == CARD_ID) =>
                {
                    captured.drake = Some(*card);
                }
                GameEvent::SpellResolved { card } if Some(*card) == captured.drake => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    captured.power = characteristics.power;
                    captured.toughness = characteristics.toughness;
                    captured.flying = characteristics.keywords.contains(&Keyword::Flying);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|entry| entry.id == CARD_ID) =>
                {
                    captured.drake = Some(*card);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|entry| entry.id == ENCHANTMENT_ID)
                    && captured.enchantment.is_none() =>
                {
                    captured.enchantment = Some(*card);
                    captured.enchantment_owner = game.object(*card).ok().map(|object| object.owner);
                    captured.enchantment_controller = game.controller_of(*card).ok();
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == captured.enchantment => {
                    captured.returned = captured.enchantment_owner.is_some_and(|owner| {
                        game.zone_of(*card) == Some(Zone::Hand)
                            && game
                                .player(owner)
                                .is_ok_and(|player| player.hand.contains(card))
                    });
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: ABILITY_ID,
                    ..
                } if Some(*source) == captured.drake => {
                    captured.trigger_stacked = true;
                    captured.source_incarnation = Some(*source_incarnation);
                    captured.target_count = game.stack.last().map(|stack| stack.targets.len());
                }
                GameEvent::DecisionOpened {
                    player: PlayerId(0),
                    kind: DecisionKind::TriggeredEffectObject,
                    min_selections,
                    max_selections,
                    ..
                } if captured.trigger_stacked => {
                    captured.choice_opened = true;
                    captured.choice_min = Some(*min_selections);
                    captured.choice_max = Some(*max_selections);
                }
                GameEvent::PriorityPassed {
                    player: PlayerId(1),
                } if captured.trigger_stacked && !captured.choice_opened => {
                    captured.response_priority_passed = true;
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability: ABILITY_ID,
                } if Some(*source) == captured.drake => {
                    captured.trigger_resolved = true;
                    captured.resolved_incarnation = Some(*source_incarnation);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if captured
                    .drake
                    .is_some_and(|source| attackers.contains(&source)) =>
                {
                    captured.attacked = true;
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if Some(*source) == captured.drake => captured.combat_damage += *amount,
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.rejected = attempted.saturating_sub(captured.accepted);
    Ok(captured)
}

struct DrakePolicy {
    player: PlayerId,
    dark_heart_cast: bool,
    drake_cast: bool,
}

impl DrakePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            dark_heart_cast: false,
            drake_cast: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let colors = [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ];
        let mut remaining = colors.map(|color| view.mana_pool.amount(color));
        for required in &cost.colored {
            let index = colors.iter().position(|color| color == required)?;
            remaining[index] = remaining[index].checked_sub(1)?;
        }
        let mut generic = Vec::new();
        for _ in 0..usize::from(cost.generic) {
            let index = remaining.iter().position(|amount| *amount > 0)?;
            remaining[index] -= 1;
            generic.push(colors[index]);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn activate_land_for(view: &GameView, requested: Option<Color>) -> Option<PolicyAction> {
        let land = match requested {
            Some(color) => view.own_battlefield.iter().find(|card| {
                card.basic_land_type.is_some() && !card.tapped && card.mana_colors.contains(&color)
            })?,
            None => view
                .own_battlefield
                .iter()
                .find(|card| card.basic_land_type.is_some() && !card.tapped)?,
        };
        let color = requested
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn land_to_play(&self, view: &GameView) -> Option<ObjectId> {
        let has = |definition: &str| {
            view.own_battlefield
                .iter()
                .any(|card| card.definition == Some(definition))
        };
        let preference = if !has("RAV-SWAMP") {
            ["RAV-SWAMP"]
        } else if !has("RAV-FOREST") {
            ["RAV-FOREST"]
        } else {
            ["RAV-ISLAND"]
        };
        preference
            .iter()
            .find_map(|definition| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(*definition))
                    .map(|card| card.id)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.basic_land_type.is_some())
                    .map(|card| card.id)
            })
    }
}

impl CodePolicy for DrakePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.drake-familiar-controller.v1"
        } else {
            "rav.drake-familiar-opponent.v1"
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
                    .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
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
            if let Some(land) = self.land_to_play(view) {
                return PolicyAction::PlayLand { card: land };
            }
        }
        let has_any_enchantment = view
            .own_battlefield
            .iter()
            .chain(view.opponent_battlefield.iter())
            .any(|card| card.card_types.contains(&CardType::Enchantment));
        if self.player == PlayerId(1) && !self.dark_heart_cast {
            let cost = ManaCost::with_colors(0, [Color::Black, Color::Green]);
            if let Some(payment) = Self::payment(view, &cost) {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(ENCHANTMENT_ID))
                {
                    self.dark_heart_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
            } else {
                let request = if view.mana_pool.amount(Color::Black) == 0 {
                    Some(Color::Black)
                } else if view.mana_pool.amount(Color::Green) == 0 {
                    Some(Color::Green)
                } else {
                    None
                };
                if let Some(action) = Self::activate_land_for(view, request) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(0) && has_any_enchantment && !self.drake_cast {
            let cost = ManaCost::with_colors(1, [Color::Blue]);
            if let Some(payment) = Self::payment(view, &cost) {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    self.drake_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
            } else {
                let requested = if view.mana_pool.amount(Color::Blue) == 0 {
                    Some(Color::Blue)
                } else {
                    None
                };
                if let Some(action) = Self::activate_land_for(view, requested) {
                    return action;
                }
            }
        }
        if let Some(land) = self.land_to_play(view).filter(|_| view.lands_played == 0) {
            return PolicyAction::PlayLand { card: land };
        }
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        if let Some(choice) = &view.triggered_ability_effect_object_choice {
            let selected = choice
                .candidates
                .iter()
                .find(|card| card.card_types.contains(&CardType::Enchantment))
                .map(|card| card.id);
            return Some(PolicyAction::ChooseTriggeredAbilityEffectObject {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                selected,
            });
        }
        let decision = view.pending_decision.as_ref()?;
        if decision.kind == DecisionKind::CleanupDiscard {
            let cards = decision
                .candidates
                .iter()
                .take(usize::from(decision.min_selections))
                .map(|card| card.id)
                .collect();
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(cards),
            });
        }
        None
    }
}

fn validate_trace(captured: &Captured) -> Result<(), String> {
    let drake = captured.drake.ok_or("Drake Familiar was not cast")?;
    let enchantment = captured
        .enchantment
        .ok_or("Dark Heart of the Wood did not enter")?;
    if captured.winner != Some(0) || captured.rejected != 0 {
        return Err("Drake Familiar did not reach a clean seat-zero terminal win".into());
    }
    if captured.paid_colors.len() != 2
        || !captured.paid_colors.contains(&Color::Blue)
        || captured.power != Some(2)
        || captured.toughness != Some(1)
        || !captured.flying
    {
        return Err("Drake Familiar cost, body, or Flying differs from the RAV printing".into());
    }
    if !captured.trigger_stacked
        || !captured.trigger_resolved
        || captured.source_incarnation.is_none()
        || captured.source_incarnation != captured.resolved_incarnation
        || captured.target_count != Some(0)
    {
        return Err(
            "Drake Familiar ETB was not a target-free trigger of its exact resolving incarnation"
                .into(),
        );
    }
    if !captured.choice_opened
        || captured.choice_min != Some(0)
        || captured.choice_max != Some(1)
        || captured.selected != Some(enchantment)
        || captured.choice_actor != Some(PlayerId(0))
        || !captured.choice_candidate_controllers.contains(&PlayerId(1))
        || captured.enchantment_owner != Some(PlayerId(1))
        || captured.enchantment_controller != Some(PlayerId(1))
        || !captured.returned
        || !captured.response_priority_passed
    {
        return Err(
            "Drake Familiar resolution choice, response window, or owner-hand return is missing"
                .into(),
        );
    }
    if !captured.attacked || captured.combat_damage < 20 {
        return Err("Drake Familiar did not attack through a full-game seat-zero win".into());
    }
    if captured.events.iter().any(|event| matches!(event, GameEvent::SacrificedByEffect { source, permanent, .. } if *source == drake && *permanent == drake)) {
        return Err("Drake Familiar was sacrificed after a successful enchantment return".into());
    }
    let paid = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellManaPaid { card, .. } if *card == drake),
    )
    .ok_or("Drake payment receipt missing")?;
    let cast = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellCast { card, .. } if *card == drake),
    )
    .ok_or("Drake cast receipt missing")?;
    let resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == drake),
    )
    .ok_or("Drake spell resolution receipt missing")?;
    let entered = event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == drake)).ok_or("Drake battlefield entry receipt missing")?;
    let stacked = event_index(&captured.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == drake)).ok_or("Drake trigger stack receipt missing")?;
    let response = captured
        .events
        .iter()
        .enumerate()
        .skip(stacked + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::PriorityPassed {
                    player: PlayerId(1)
                }
            )
            .then_some(index)
        })
        .ok_or("opponent response-priority receipt missing")?;
    let opened = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("Drake choice-open receipt missing")?;
    let completed = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("Drake choice-completed receipt missing")?;
    let returned = event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == enchantment)).ok_or("enchantment owner-hand return receipt missing")?;
    let resolved_ability = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == drake)).ok_or("Drake ability-resolution receipt missing")?;
    let attack = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&drake))).ok_or("Drake attack receipt missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == drake && *amount > 0)).ok_or("Drake combat-damage receipt missing")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < stacked
        && stacked < response
        && response < opened
        && opened < completed
        && completed < returned
        && returned < resolved_ability
        && resolved_ability < attack
        && attack < damage
        && damage < ended)
    {
        return Err("Drake Familiar cast, response, trigger-choice, return, combat, and terminal order differs".into());
    }
    Ok(())
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    match run_match(mutation, seed) {
        Ok(captured) => validate_trace(&captured).is_err(),
        Err(_) => true,
    }
}

fn trace_json(captured: &Captured, seed: u64) -> Value {
    let projected = project_events(&captured.events);
    let events: Vec<_> = projected.iter().map(|event: &ProjectedEvent| json!({
        "sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical
    })).collect();
    json!({
        "id":"rav-drake-familiar-return-or-sacrifice-etb-pvp",
        "passed":true,
        "terminal":captured.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted,
        "rejected_moves":captured.rejected,
        "shuffle_seed":seed,
        "drake_object":captured.drake.map(|id| id.0),
        "enchantment_object":captured.enchantment.map(|id| id.0),
        "drake":{"mana_cost":"{1}{U}","paid_colors":captured.paid_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"power":captured.power,"toughness":captured.toughness,"flying":captured.flying,"attacked":captured.attacked,"combat_damage":captured.combat_damage},
        "etb":{"ability":ABILITY_ID,"source_incarnation":captured.source_incarnation,"resolved_incarnation":captured.resolved_incarnation,"target_count":captured.target_count,"choice_opened":captured.choice_opened,"choice_min":captured.choice_min,"choice_max":captured.choice_max,"selected_object":captured.selected.map(|id| id.0),"choice_actor_seat":captured.choice_actor.map(|player| player.0),"choice_candidate_controller_seats":captured.choice_candidate_controllers.iter().map(|player| player.0).collect::<Vec<_>>(),"returned_to_owner_hand":captured.returned,"enchantment_owner_seat":captured.enchantment_owner.map(|player| player.0),"enchantment_controller_seat":captured.enchantment_controller.map(|player| player.0),"response_priority_passed":captured.response_priority_passed},
        "events":events
    })
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 44;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},
        "trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
