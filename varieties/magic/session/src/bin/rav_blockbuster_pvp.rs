//! Full-game RAV Blockbuster authority.
//!
//! A legal shuffled two-player game casts Blockbuster, then sacrifices it
//! for {1}{R} to damage every tapped creature and each player.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Type, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BLOCKBUSTER";
const CARD_NAME: &str = "Blockbuster";
const ABILITY_ID: &str = "sacrifice-source-deal-three-to-tapped-creatures-and-players";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/blockbuster.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/blockbuster.rs"]
mod candidate_blockbuster;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Blockbuster result: {error}");
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
            eprintln!("Blockbuster PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events || capture.winner != replay.winner {
        return Err("Blockbuster shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "red_generic_activation_cost_mutation_detected": mutation_detected(seed, Mutation::ActivatedCost),
        "source_sacrifice_mutation_detected": mutation_detected(seed, Mutation::NoSourceSacrifice),
        "tapped_creature_and_player_damage_mutation_detected": mutation_detected(seed, Mutation::NoGlobalDamage),
        "sealed_mutation_authority": false
    });
    for key in [
        "red_generic_activation_cost_mutation_detected",
        "source_sacrifice_mutation_detected",
        "tapped_creature_and_player_damage_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Blockbuster mutation escaped the trace oracle: {key}"
            ));
        }
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
        "card": {"collector_number": 115, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "303 Enchantments",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities, targets, and costs",
                "608.2 Resolving an activated ability",
                "120.3-120.6 Damage results and marked damage",
                "701.17 Sacrifice",
                "704 State-based actions"
            ],
            "source_contract": "RAV Blockbuster #115, {3}{R}{R} red enchantment; {1}{R}, sacrifice Blockbuster: deal 3 damage to each tapped creature and each player",
            "source_audit": {
                "printed_source_url": "https://www.lastdodo.nl/nl/items/829007-blockbuster",
                "oracle_source_url": "https://mtg.wtf/card/rav/115/Blockbuster",
                "oracle_reconciliation": "Oracle changes the self-name to 'this enchantment'; it preserves mana cost, source-sacrifice cost, damage amount, tapped-creature filter, and both-player scope",
                "no_printed_oracle_rules_difference": true,
                "legacy_catalog_note": "The old ability label appears to say a tapped creature is sacrificed, but the RAV printing sacrifices Blockbuster itself. This task's candidate contract and trace use source sacrifice."
            },
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "source_guard": {
            "runs_before_candidate_functions": true,
            "syntactic_side_effect_fixture_rejected": true,
            "fixture_compiled": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Blockbuster source: {error}"))?;
    validate_source_text(&source)?;
    let injected = source.replacen(
        "id: \"RAV-BLOCKBUSTER\",",
        "id: { let _side_effect = 1; \"RAV-BLOCKBUSTER\" },",
        1,
    );
    if injected == source
        || syn::parse_file(&injected).is_err()
        || validate_source_text(&injected).is_ok()
    {
        return Err(
            "Blockbuster source guard did not reject its syntactically valid side-effect fixture"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Blockbuster source is not valid declarative Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Blockbuster source must contain two imports and two literal functions".to_owned(),
        );
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports += 1;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_return = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => "",
                };
                let safe_attributes = function
                    .attrs
                    .iter()
                    .all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"));
                let crate_visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
                let literal = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(expression, None) if expression_is_declarative(expression));
                if expected_return.is_empty()
                    || !functions.insert(name)
                    || !crate_visible
                    || !safe_attributes
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || function.sig.variadic.is_some()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_return))
                    || !literal
                {
                    return Err(
                        "Blockbuster functions must return direct declarative literals".to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Blockbuster source contains unsupported executable top-level code".to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("Blockbuster source structure differs from its contract".to_owned());
    }
    Ok(())
}

fn expression_is_declarative(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Array(array) => array.elems.iter().all(expression_is_declarative),
        Expr::Reference(reference) => expression_is_declarative(&reference.expr),
        Expr::Paren(paren) => expression_is_declarative(&paren.expr),
        Expr::Group(group) => expression_is_declarative(&group.expr),
        Expr::Tuple(tuple) => tuple.elems.iter().all(expression_is_declarative),
        Expr::Struct(structure) => {
            let path = structure
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            let known_struct = path.last().is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "CardDefinition" | "ActivatedAbilityBinding" | "ActivatedAbility"
                )
            });
            let effect_variant = path.first().is_some_and(|name| name == "Effect");
            (known_struct || effect_variant)
                && structure
                    .fields
                    .iter()
                    .all(|field| expression_is_declarative(&field.expr))
                && structure
                    .rest
                    .as_ref()
                    .is_none_or(expression_is_declarative)
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) => {
                    let segments = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>();
                    matches!(segments.as_slice(), [set, method] if set == "BTreeSet" && (method == "from" || method == "new"))
                        || matches!(segments.as_slice(), [cost, method] if cost == "ManaCost" && method == "with_colors")
                }
                _ => false,
            };
            allowed && call.args.iter().all(expression_is_declarative)
        }
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::parse2::<syn::ExprArray>(mac.mac.tokens.clone())
                .is_ok_and(|array| array.elems.iter().all(expression_is_declarative))
        }
        _ => false,
    }
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn validate_candidate_semantics() -> Result<(), String> {
    let submitted = candidate_blockbuster::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Blockbuster is missing from the RAV registry")?;
    let definition_rules = [
        "full-rules-fidelity",
        "colored-enchantment-casting",
        "sacrifice-source-deal-three-to-tapped-creatures-and-players",
    ];
    let binding = candidate_blockbuster::activated_ability();
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Blockbuster activated ability binding is missing from the RAV registry")?;
    let ability = &binding.ability;
    if submitted.id != CARD_ID
        || submitted.name != CARD_NAME
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(3, [Color::Red, Color::Red])
        || submitted.colors != BTreeSet::from([Color::Red])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Enchantment])
        || submitted.is_basic_land
        || submitted.power.is_some()
        || submitted.toughness.is_some()
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
        || submitted.supported_rules != definition_rules
        || registered.mana_cost != submitted.mana_cost
        || registered.colors != submitted.colors
        || registered.card_types != submitted.card_types
        || registered.supported_rules != submitted.supported_rules
        || registered.power != submitted.power
        || registered.toughness != submitted.toughness
        || binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects != [Effect::DealDamageToEachTappedCreatureAndPlayer { amount: 3 }]
        || registered_binding.ability.mana_cost != ability.mana_cost
        || registered_binding.ability.id != ability.id
        || registered_binding.ability.sacrifice_source != ability.sacrifice_source
        || registered_binding.ability.targets != ability.targets
        || registered_binding.ability.effects != ability.effects
    {
        return Err("Blockbuster identity, characteristics, activated cost, sacrifice, target, or effect differ from the RAV printing".to_owned());
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
        || !cards
            .iter()
            .any(|card| card.collector_number == 115 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator changed for Blockbuster".to_owned());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    ActivatedCost,
    NoSourceSacrifice,
    NoGlobalDamage,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_blockbuster::definition();
    let mut binding = candidate_blockbuster::activated_ability();
    match mutation {
        Mutation::ActivatedCost => {
            binding.ability.mana_cost = ManaCost::with_colors(2, [Color::Red])
        }
        Mutation::NoSourceSacrifice => binding.ability.sacrifice_source = false,
        Mutation::NoGlobalDamage => {
            binding.ability.effects =
                vec![Effect::DealDamageToEachTappedCreatureAndPlayer { amount: 2 }]
        }
        Mutation::None => {}
    }
    if mutation == Mutation::None {
        return (definition, binding);
    }
    // Keep this hook local and explicit so future card mutations can cover both
    // the enchantment identity and activated-ability branches.
    let _ = &mut definition;
    (definition, binding)
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    cast_payment: Vec<Color>,
    activation_cost: Option<ManaCost>,
    source_sacrificed_as_cost: bool,
    source_moved_to_graveyard: bool,
    ability_resolved: bool,
    life_before_activation: Option<(i64, i64)>,
    creatures_before_resolution: Vec<(ObjectId, bool)>,
    creature_damage: Vec<(ObjectId, i32)>,
    player_damage: Vec<(usize, i32)>,
    creature_zones_after_resolution: Vec<(ObjectId, Option<Zone>)>,
    life_after_resolution: Option<(i64, i64)>,
    final_life: (i64, i64),
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BLOCKBUSTER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BLOCKBUSTER_SEED must be unsigned".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Blockbuster trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    let Ok(capture) = run_match(seed, mutation) else {
        return true;
    };
    validate_trace(&capture, seed).is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let (definition, binding) = candidate_values(mutation);
    let spell_cost = definition.mana_cost.clone();
    let activation_cost = binding.ability.mana_cost.clone();
    let pilot_deck = deck(&[
        (CARD_ID, 4),
        ("RAV-GOBLIN-FIRE-FIEND", 4),
        ("RAV-CHAR", 4),
        ("RAV-MOUNTAIN", 48),
    ]);
    let opponent_deck = deck(&[("RAV-WATCHWOLF", 4), ("RAV-FOREST", 28), ("RAV-PLAINS", 28)]);
    for (name, list) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("{name} Blockbuster deck is not sixty cards"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition, binding.clone())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(BlockbusterPolicy::new(
            PlayerId(0),
            spell_cost,
            activation_cost,
            binding.ability.sacrifice_source,
        )),
        Box::new(RavCatalogPolicy::new(
            PlayerId(1),
            CatalogPolicyProfile::Curve,
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut cast_payment = Vec::new();
    let mut captured_activation_cost = None;
    let mut source_sacrificed_as_cost = false;
    let mut source_moved_to_graveyard = false;
    let mut ability_resolved = false;
    let mut life_before_activation = None;
    let mut creatures_before_resolution = Vec::new();
    let mut creature_damage = Vec::new();
    let mut player_damage = Vec::new();
    let mut creature_zones_after_resolution = Vec::new();
    let mut life_after_resolution = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Blockbuster game hit its development run bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Blockbuster policy seat missing")?;
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
        if let PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } = &action
            && view
                .hand
                .iter()
                .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
        {
            source.get_or_insert(request.card);
        }
        if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == ABILITY_ID
        {
            source.get_or_insert(activation.source);
            life_before_activation = Some((
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life,
            ));
            let mut observed = Vec::new();
            for card in view
                .own_battlefield
                .iter()
                .chain(&view.opponent_battlefield)
            {
                if card.card_types.contains(&CardType::Creature) {
                    observed.push((card.id, card.tapped));
                }
            }
            creatures_before_resolution = observed;
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy action rejected at turn {} / {:?} for {:?}: {error}",
                    game.turn, view.step, player
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    source.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source => cast_payment = colors.clone(),
                GameEvent::AbilityManaPaid {
                    source: ability_source,
                    ability,
                    mana_cost,
                    ..
                } if Some(*ability_source) == source && *ability == ABILITY_ID => {
                    captured_activation_cost = Some(mana_cost.clone())
                }
                GameEvent::SacrificedAsAbilityCost {
                    source: ability_source,
                    permanent,
                    ..
                } if Some(*ability_source) == source && Some(*permanent) == source => {
                    source_sacrificed_as_cost = true
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if Some(*card) == source => source_moved_to_graveyard = true,
                GameEvent::DamageDealtToPermanent {
                    source: damage_source,
                    permanent,
                    amount,
                } if Some(*damage_source) == source => creature_damage.push((*permanent, *amount)),
                GameEvent::DamageDealtToPlayer {
                    source: damage_source,
                    player: damaged,
                    amount,
                } if Some(*damage_source) == source => player_damage.push((damaged.0, *amount)),
                GameEvent::AbilityResolved {
                    source: ability_source,
                    ability,
                    ..
                } if Some(*ability_source) == source && *ability == ABILITY_ID => {
                    ability_resolved = true;
                    creature_zones_after_resolution = creatures_before_resolution
                        .iter()
                        .map(|(object, _)| (*object, game.zone_of(*object)))
                        .collect();
                    life_after_resolution = Some((
                        game.player(PlayerId(0))
                            .map_err(|error| error.to_string())?
                            .life,
                        game.player(PlayerId(1))
                            .map_err(|error| error.to_string())?
                            .life,
                    ));
                }
                _ => {}
            }
        }
    }
    let final_life = (
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    );
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        source,
        cast_payment,
        activation_cost: captured_activation_cost,
        source_sacrificed_as_cost,
        source_moved_to_graveyard,
        ability_resolved,
        life_before_activation,
        creatures_before_resolution,
        creature_damage,
        player_damage,
        creature_zones_after_resolution,
        life_after_resolution,
        final_life,
    })
}

fn new_game(definition: CardDefinition, binding: ActivatedAbilityBinding) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Blockbuster missing from card definitions")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|known| known.card_definition != CARD_ID);
    abilities.push(binding);
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        rav_triggered_ability_bindings(),
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

struct BlockbusterPolicy {
    player: PlayerId,
    spell_cost: ManaCost,
    activation_cost: ManaCost,
    source_sacrifice: bool,
    activation_submitted: bool,
    inner: RavCatalogPolicy,
}

impl BlockbusterPolicy {
    fn new(
        player: PlayerId,
        spell_cost: ManaCost,
        activation_cost: ManaCost,
        source_sacrifice: bool,
    ) -> Self {
        Self {
            player,
            spell_cost,
            activation_cost,
            source_sacrifice,
            activation_submitted: false,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }

    fn activate_red_land(&self, view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Red)
                    && !card.tapped
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Red,
            })
    }
}

impl CodePolicy for BlockbusterPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.blockbuster-sacrifice-damage.v1"
        } else {
            "rav.blockbuster-ravnica-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && view.active_player == self.player
            && view.priority == self.player
            && view.step == Step::PrecombatMain
            && view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if self.player == PlayerId(0)
            && !self.activation_submitted
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && view.step == Step::PrecombatMain
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let has_tapped_opponent_creature = view
                .opponent_battlefield
                .iter()
                .any(|card| card.card_types.contains(&CardType::Creature) && card.tapped);
            if let Some(source) = source.filter(|_| has_tapped_opponent_creature) {
                let required = self.activation_cost.mana_value();
                if view.mana_pool.amount(Color::Red) == 0 || view.mana_pool.total() < required {
                    return self
                        .activate_red_land(view)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                self.activation_submitted = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: if self.source_sacrifice {
                            vec![source.id]
                        } else {
                            vec![]
                        },
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![],
                    },
                };
            }
        }
        let mut action = self.inner.propose_move(view);
        if self.player == PlayerId(0) {
            let request = match &action {
                PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } => {
                    Some(request.clone())
                }
                _ => None,
            };
            if let Some(request) = request
                && view
                    .hand
                    .iter()
                    .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
                && let Some(mana_selection) = payment_selection(view, &self.spell_cost)
            {
                action = PolicyAction::CastWithPayment {
                    request,
                    chosen_x: None,
                    mana_selection,
                };
            }
        }
        action
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let source = capture.source.ok_or("Blockbuster was not cast")?;
    if capture.winner != Some(0)
        || capture.rejected_moves != 0
        || capture.final_life.0 <= 0
        || capture.final_life.1 > 0
    {
        return Err(format!(
            "Blockbuster game did not reach a clean seat-zero win: winner={:?}, life={:?}, rejected={}",
            capture.winner, capture.final_life, capture.rejected_moves
        ));
    }
    if capture.cast_payment.len() != 5
        || capture
            .cast_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 5
        || capture.activation_cost.as_ref() != Some(&ManaCost::with_colors(1, [Color::Red]))
        || !capture.source_sacrificed_as_cost
        || !capture.source_moved_to_graveyard
        || !capture.ability_resolved
    {
        return Err(format!(
            "Blockbuster exact spell/activation payments, source sacrifice, or resolution are missing: cast={:?}, ability={:?}, sacrificed={}, source_graveyard={}, resolved={}",
            capture.cast_payment,
            capture.activation_cost,
            capture.source_sacrificed_as_cost,
            capture.source_moved_to_graveyard,
            capture.ability_resolved
        ));
    }
    let tapped: BTreeSet<_> = capture
        .creatures_before_resolution
        .iter()
        .filter_map(|(object, was_tapped)| was_tapped.then_some(*object))
        .collect();
    let untapped: BTreeSet<_> = capture
        .creatures_before_resolution
        .iter()
        .filter_map(|(object, was_tapped)| (!was_tapped).then_some(*object))
        .collect();
    let damaged: BTreeSet<_> = capture
        .creature_damage
        .iter()
        .map(|(object, _)| *object)
        .collect();
    let damaged_players: BTreeSet<_> = capture
        .player_damage
        .iter()
        .map(|(player, _)| *player)
        .collect();
    if tapped.is_empty()
        || capture.creature_damage.len() != tapped.len()
        || damaged != tapped
        || capture
            .creature_damage
            .iter()
            .any(|(_, amount)| *amount != 3)
        || !damaged.is_disjoint(&untapped)
        || damaged_players != BTreeSet::from([0, 1])
        || capture.player_damage.len() != 2
        || capture.player_damage.iter().any(|(_, amount)| *amount != 3)
    {
        return Err(format!(
            "Blockbuster did not hit exactly each tapped creature and both players: tapped={tapped:?}, untapped={untapped:?}, creature_damage={:?}, player_damage={:?}",
            capture.creature_damage, capture.player_damage
        ));
    }
    let before_resolution = capture
        .life_before_activation
        .ok_or("life snapshot before Blockbuster activation is missing")?;
    let after_resolution = capture
        .life_after_resolution
        .ok_or("life snapshot after Blockbuster resolution is missing")?;
    if after_resolution != (before_resolution.0 - 3, before_resolution.1 - 3) {
        return Err(format!(
            "Blockbuster did not deal exactly three damage to both players: before={before_resolution:?}, after={after_resolution:?}"
        ));
    }
    let zones: BTreeMap<_, _> = capture
        .creature_zones_after_resolution
        .iter()
        .copied()
        .collect();
    if tapped
        .iter()
        .any(|object| zones.get(object) != Some(&Some(Zone::Graveyard)))
        || untapped
            .iter()
            .any(|object| zones.get(object) != Some(&Some(Zone::Battlefield)))
    {
        return Err(format!(
            "post-resolution creature zones do not match the tapped filter: {zones:?}"
        ));
    }
    let events = &capture.events;
    let spell_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 5 && colors.iter().all(|color| *color == Color::Red))).ok_or("exact {3}{R}{R} Blockbuster spell payment missing")?;
    let cast = find_event_after(events, spell_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Blockbuster cast receipt missing")?;
    let spell_resolved = find_event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Blockbuster spell resolution missing")?;
    let entered = find_event_after(events, spell_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Blockbuster battlefield entry missing")?;
    let ability_paid = find_event_after(events, entered, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: ability_source, ability: ABILITY_ID, mana_cost } if *ability_source == source && *mana_cost == ManaCost::with_colors(1, [Color::Red]))).ok_or("{1}{R} ability mana payment missing")?;
    let sacrificed = find_event_after(events, ability_paid, |event| matches!(event, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source: ability_source, permanent } if *ability_source == source && *permanent == source)).ok_or("Blockbuster was not sacrificed as the ability's source cost")?;
    let source_gone = find_event_after(events, sacrificed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source)).ok_or("sacrificed Blockbuster did not move to graveyard before stacking")?;
    let activated = find_event_after(events, source_gone, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Blockbuster activation did not reach the stack")?;
    let damage_indices: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::DamageDealtToPermanent { source: damage_source, .. } | GameEvent::DamageDealtToPlayer { source: damage_source, .. } if *damage_source == source).then_some(index)).collect();
    if damage_indices.len() != tapped.len() + 2
        || damage_indices.iter().any(|index| *index <= activated)
    {
        return Err(format!(
            "damage packets do not follow activation or are incomplete: indices={damage_indices:?}, tapped={tapped:?}"
        ));
    }
    let ability_resolved = find_event_after(events, *damage_indices.last().ok_or("Blockbuster emitted no damage packets")?, |event| matches!(event, GameEvent::AbilityResolved { source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Blockbuster ability resolution receipt missing")?;
    let tapped_deaths: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if tapped.contains(card) && index > ability_resolved => Some((index, *card)),
            _ => None,
        })
        .collect();
    if tapped_deaths.len() != tapped.len() {
        return Err(format!(
            "lethal tapped-creature damage did not produce each expected state-based death: {tapped_deaths:?}, tapped={tapped:?}"
        ));
    }
    let game_end = find_event_after(events, ability_resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero game-end event missing")?;
    let mut selected = vec![
        spell_paid,
        cast,
        spell_resolved,
        entered,
        ability_paid,
        sacrificed,
        source_gone,
        activated,
    ];
    selected.extend(damage_indices);
    selected.push(ability_resolved);
    selected.extend(tapped_deaths.iter().map(|(index, _)| *index));
    selected.push(game_end);
    selected.sort_unstable();
    let projected = project_events(events);
    let projected: Vec<ProjectedEvent> = selected
        .into_iter()
        .map(|index| projected[index].clone())
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    if kinds.first() != Some(&"SpellManaPaid")
        || !kinds.contains(&"AbilityActivated")
        || !kinds.contains(&"AbilityResolved")
        || kinds.last() != Some(&"GameEnded")
    {
        return Err(format!(
            "projected Blockbuster event sequence is incomplete: {kinds:?}"
        ));
    }
    Ok(json!({
        "trace_id": "rav-blockbuster-source-sacrifice-tapped-damage-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_blockbuster_red_60", "rav_watchwolf_forest_plains_60"],
        "policies": ["rav.blockbuster-sacrifice-damage.v1", "rav.blockbuster-ravnica-opponent.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "blockbuster": {"object": source.0, "collector_number": 115, "mana_cost": "{3}{R}{R}", "mana_paid": capture.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "zone_after_cost": "Graveyard"},
        "activation": {"ability": ABILITY_ID, "source": source.0, "mana_cost": "{1}{R}", "source_sacrificed_as_cost": capture.source_sacrificed_as_cost, "source_moved_before_stack": true, "target_count": 0},
        "effect": {"damage": 3, "tapped_creatures_before_resolution": tapped.iter().map(|object| object.0).collect::<Vec<_>>(), "untapped_creatures_before_resolution": untapped.iter().map(|object| object.0).collect::<Vec<_>>(), "damaged_creatures": capture.creature_damage.iter().map(|(object, amount)| json!({"object": object.0, "amount": amount})).collect::<Vec<_>>(), "player_damage": capture.player_damage.iter().map(|(seat, amount)| json!({"seat": seat, "amount": amount})).collect::<Vec<_>>(), "life_before": before_resolution, "life_after": after_resolution, "zones_after_resolution": capture.creature_zones_after_resolution.iter().map(|(object, zone)| json!({"object": object.0, "zone": format!("{zone:?}")})).collect::<Vec<_>>()},
        "final_life": capture.final_life,
        "events": projected,
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status {CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
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
