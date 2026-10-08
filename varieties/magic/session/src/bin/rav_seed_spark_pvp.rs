//! Full-game diagnostic authority for RAV Seed Spark.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CreatureSubtype, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TokenSpec, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SEED-SPARK";
const SIGNET_ID: &str = "RAV-BOROS-SIGNET";
const SLASHER_ID: &str = "RAV-VIASHINO-SLASHER";
const PLAINS_ID: &str = "RAV-PLAINS";
const MOUNTAIN_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/seed_spark.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/seed_spark.rs"]
mod candidate_seed_spark;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Seed Spark result: {error}");
                return ExitCode::FAILURE;
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
            eprintln!("Seed Spark PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoDestroy,
    NoTokens,
    OneToken,
    WrongTokenBody,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenWitness {
    id: ObjectId,
    spec: Option<TokenSpec>,
    controller: Option<PlayerId>,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    creature_subtypes: BTreeSet<CreatureSubtype>,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<Keyword>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    target_definition: Option<&'static str>,
    target_zone: Option<Zone>,
    spell_zone: Option<Zone>,
    mana_paid: Vec<Color>,
    resolved: bool,
    tokens: Vec<TokenWitness>,
    attacks_with_tokens: Vec<ObjectId>,
    combat_damage: Vec<(ObjectId, i32)>,
    opponent_life: Option<i64>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }

    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    if !same_match(&reference, &run_match(seed, Mutation::None)?) {
        return Err("Seed Spark full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "destruction_instruction_removed_mutation_detected": mutation_detected(seed, Mutation::NoDestroy)?,
        "token_instruction_removed_mutation_detected": mutation_detected(seed, Mutation::NoTokens)?,
        "token_count_reduced_mutation_detected": mutation_detected(seed, Mutation::OneToken)?,
        "saproling_body_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTokenBody)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "destruction_instruction_removed_mutation_detected",
        "token_instruction_removed_mutation_detected",
        "token_count_reduced_mutation_detected",
        "saproling_body_changed_mutation_detected",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Seed Spark gameplay mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 30, "printings": 1, "definition_id": CARD_ID, "name": "Seed Spark"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: instant spell casting and target selection",
                "608.2: spell resolution and printed instruction order",
                "701.7: destroying a permanent",
                "111.1 and 111.3: token creation and characteristics",
                "508.1 and 510.1: declaring attackers and combat damage"
            ],
            "source_contract": "RAV Seed Spark #30 is a white {3}{W} Instant that destroys target artifact or enchantment, then creates two green 1/1 Saproling creature tokens.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_seed_spark::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoDestroy => definition
            .effects
            .retain(|effect| !matches!(effect, Effect::DestroyTargetArtifactOrEnchantment)),
        Mutation::NoTokens => definition
            .effects
            .retain(|effect| !matches!(effect, Effect::CreateToken { .. })),
        Mutation::OneToken => {
            if let Some(Effect::CreateToken { count, .. }) = definition
                .effects
                .iter_mut()
                .find(|effect| matches!(effect, Effect::CreateToken { .. }))
            {
                *count = 1;
            }
        }
        Mutation::WrongTokenBody => {
            if let Some(Effect::CreateToken { token, .. }) = definition
                .effects
                .iter_mut()
                .find(|effect| matches!(effect, Effect::CreateToken { .. }))
            {
                token.power = 2;
            }
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::White]),
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_seed_spark::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Seed Spark is missing from the RAV definition registry")?;
    if registered != candidate
        || candidate.id != CARD_ID
        || candidate.name != "Seed Spark"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(3, [Color::White])
        || candidate.colors != BTreeSet::from([Color::White])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "typed-artifact-or-enchantment-target",
                "destroy",
                "saproling-token-creation",
            ]
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || candidate.effects
            != [
                Effect::DestroyTargetArtifactOrEnchantment,
                Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 2,
                },
            ]
    {
        return Err("Seed Spark candidate or registry differs from the exact RAV #30 identity and ordered instructions".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Seed Spark source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Seed Spark source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err(
            "Seed Spark module must contain only private imports and its direct-literal definition"
                .to_owned(),
        );
    }
    let mut imports = 0;
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
                if !safe_use_tree(&item.tree) {
                    return Err("Seed Spark source contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                definitions += 1;
                let attrs_ok = function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                });
                if !attrs_ok
                    || function.sig.ident != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.path.segments.last().is_some_and(|segment| segment.ident == "CardDefinition") && literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(
                        "Seed Spark must expose one crate-private, direct-literal CardDefinition"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Seed Spark source contains an executable or unsupported item".to_owned(),
                );
            }
        }
    }
    if imports != 2 || definitions != 1 {
        return Err(
            "Seed Spark imports or definition count differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet"
                | "CardDefinition"
                | "CardType"
                | "Color"
                | "Effect"
                | "ManaCost"
                | "TokenSpec"
        ),
        UseTree::Group(group) => group.items.iter().all(|item| safe_use_tree(item)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["None"])
                || path_is(&segments, &["Color", "White"])
                || path_is(&segments, &["CardType", "Instant"])
                || path_is(&segments, &["Effect", "DestroyTargetArtifactOrEnchantment"])
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            let valid = path_is(&segments, &["ManaCost", "with_colors"])
                || path_is(&segments, &["BTreeSet", "from"])
                || path_is(&segments, &["BTreeSet", "new"])
                || path_is(&segments, &["TokenSpec", "saproling"]);
            valid && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            let segments: Vec<_> = literal
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["Effect", "CreateToken"])
                && literal.rest.is_none()
                && literal
                    .fields
                    .iter()
                    .all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Seed Spark definition missing during game construction")? =
        candidate_value(mutation);
    Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .and_then(|mut game| {
        game.register_attachment_bindings(rav_attachment_bindings())?;
        Ok(game)
    })
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SEED_SPARK_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SEED_SPARK_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Seed Spark full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = deck(&[
        (CARD_ID, 4),
        (SLASHER_ID, 4),
        (PLAINS_ID, 26),
        (MOUNTAIN_ID, 26),
    ]);
    let opponent = deck(&[(SIGNET_ID, 4), (MOUNTAIN_ID, 56)]);
    for (label, list) in [
        ("Seed Spark pilot", &pilot),
        ("Boros Signet opponent", &opponent),
    ] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("{label} fixture must contain exactly 60 cards"));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SeedSparkPolicy::new(PlayerId(0), mutation)),
        Box::new(SignetPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        spell: None,
        target: None,
        target_definition: None,
        target_zone: None,
        spell_zone: None,
        mana_paid: vec![],
        resolved: false,
        tokens: vec![],
        attacks_with_tokens: vec![],
        combat_damage: vec![],
        opponent_life: None,
    };
    while !game.is_game_over() {
        if game.turn > 42 || captured.accepted >= 6_000 {
            return Err(format!(
                "Seed Spark PvP exceeded bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Seed Spark policy seat missing")?;
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
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            captured.spell = Some(request.card);
            captured.target = request.targets.iter().find_map(|target| match target {
                Target::Permanent(object) => Some(*object),
                _ => None,
            });
        }
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Seed Spark policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.target_definition = captured.target.and_then(|target| {
        game.card_definition(target)
            .ok()
            .map(|definition| definition.id)
    });
    captured.spell_zone = captured.spell.and_then(|spell| game.zone_of(spell));
    captured.opponent_life = game.player(PlayerId(1)).ok().map(|player| player.life);
    Ok(captured)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == captured.spell => captured.mana_paid = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == captured.spell => {
                captured.resolved = true;
            }
            GameEvent::CardMoved { card, to } if Some(*card) == captured.target => {
                captured.target_zone = Some(*to);
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => {
                if !captured.tokens.iter().any(|witness| witness.id == *token) {
                    let stats = game
                        .characteristics(*token)
                        .map_err(|error| error.to_string())?;
                    let object = game.object(*token).map_err(|error| error.to_string())?;
                    captured.tokens.push(TokenWitness {
                        id: *token,
                        spec: object.token.clone(),
                        controller: game.controller_of(*token).ok(),
                        colors: stats.colors,
                        card_types: stats.card_types,
                        creature_subtypes: stats.creature_subtypes,
                        power: stats.power,
                        toughness: stats.toughness,
                        keywords: stats.keywords,
                    });
                }
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => {
                let token_ids: HashSet<_> = captured.tokens.iter().map(|token| token.id).collect();
                captured.attacks_with_tokens.extend(
                    attackers
                        .iter()
                        .copied()
                        .filter(|id| token_ids.contains(id)),
                );
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *amount > 0 => captured.combat_damage.push((*source, *amount)),
            _ => {}
        }
    }
    Ok(())
}

struct SeedSparkPolicy {
    player: PlayerId,
    mutation: Mutation,
    spell_cast: bool,
    slasher_cast: bool,
}

impl SeedSparkPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            spell_cast: false,
            slasher_cast: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(candidate, _)| candidate == color)?;
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

    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let source = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && preferred.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = preferred
            .filter(|color| source.mana_colors.contains(color))
            .or_else(|| source.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: source.id,
            color,
        })
    }

    fn cast_or_make_mana(
        &self,
        view: &GameView,
        card_id: &'static str,
        cost: &ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(card_id))?;
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        Self::mana_source(view, missing)
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let have_white = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::White)
        });
        let preferred = if have_white { MOUNTAIN_ID } else { PLAINS_ID };
        view.hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(PLAINS_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(MOUNTAIN_ID))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for SeedSparkPolicy {
    fn id(&self) -> &'static str {
        "rav.seed-spark-destroy-and-create-two-saprolings.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0) {
            if view.active_player == self.player
                && view.step.is_main()
                && let Some(action) = self.play_land(view)
            {
                return action;
            }
            if !self.spell_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                && let Some(target) = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(SIGNET_ID))
            {
                let cost = candidate_value(self.mutation).mana_cost;
                if let Some(action) =
                    self.cast_or_make_mana(view, CARD_ID, &cost, vec![Target::Permanent(target.id)])
                {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.spell_cast = true;
                    }
                    return action;
                }
            }
            if view.active_player == self.player
                && view.step.is_main()
                && !self.slasher_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(SLASHER_ID))
            {
                if let Some(action) = self.cast_or_make_mana(
                    view,
                    SLASHER_ID,
                    &ManaCost::with_colors(1, [Color::Red]),
                    vec![],
                ) {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.slasher_cast = true;
                    }
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

struct SignetPolicy {
    player: PlayerId,
    cast: bool,
}

impl SignetPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast: false,
        }
    }
}

impl CodePolicy for SignetPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-signet-opponent-casts-and-passes.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && view.step.is_main()
            && view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(MOUNTAIN_ID))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        let signet_live = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(SIGNET_ID));
        if view.active_player == self.player
            && view.step.is_main()
            && !self.cast
            && !signet_live
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(SIGNET_ID))
        {
            if view.mana_pool.total() >= 2 {
                let signet = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(SIGNET_ID))
                    .unwrap();
                self.cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: signet.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Red; 2],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(MOUNTAIN_ID) && !card.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 || captured.accepted == 0 {
        return Err("Seed Spark duel did not end in a clean P0 terminal win".to_owned());
    }
    let spell = captured.spell.ok_or("Seed Spark was not cast")?;
    let target = captured
        .target
        .ok_or("Seed Spark did not select a permanent target")?;
    if captured.target_definition != Some(SIGNET_ID)
        || captured.target_zone != Some(Zone::Graveyard)
        || captured.spell_zone != Some(Zone::Graveyard)
        || !captured.resolved
    {
        return Err(format!(
            "Seed Spark did not destroy the exact opposing Boros Signet and resolve cleanly (target_definition={:?}, target_zone={:?}, spell_zone={:?}, resolved={})",
            captured.target_definition,
            captured.target_zone,
            captured.spell_zone,
            captured.resolved
        ));
    }
    let definition = candidate_seed_spark::definition();
    if definition != expected_definition() {
        return Err(
            "Seed Spark exact identity, white Instant type, {3}{W} cost, or ordered effects differ"
                .to_owned(),
        );
    }
    if captured.mana_paid.len() != 4
        || captured
            .mana_paid
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            < 1
    {
        return Err(format!(
            "Seed Spark exact four mana including white was not paid: {:?}",
            captured.mana_paid
        ));
    }
    if captured.tokens.len() != 2
        || captured
            .tokens
            .iter()
            .map(|token| token.id)
            .collect::<BTreeSet<_>>()
            .len()
            != 2
    {
        return Err(format!(
            "Seed Spark must create exactly two distinct Saproling tokens; got {}",
            captured.tokens.len()
        ));
    }
    let expected_token = TokenSpec::saproling();
    if captured.tokens.iter().any(|token| {
        token.spec.as_ref() != Some(&expected_token)
            || token.controller != Some(PlayerId(0))
            || token.colors != BTreeSet::from([Color::Green])
            || token.card_types != BTreeSet::from([CardType::Creature])
            || token.creature_subtypes != BTreeSet::from([CreatureSubtype::Saproling])
            || token.power != Some(1)
            || token.toughness != Some(1)
            || !token.keywords.is_empty()
    }) {
        return Err(
            "Seed Spark live token characteristics differ from two green 1/1 Saprolings".to_owned(),
        );
    }
    let token_ids: BTreeSet<_> = captured.tokens.iter().map(|token| token.id).collect();
    if captured
        .attacks_with_tokens
        .iter()
        .all(|token| !token_ids.contains(token))
        || captured
            .combat_damage
            .iter()
            .all(|(source, _)| !token_ids.contains(source))
        || captured
            .combat_damage
            .iter()
            .map(|(_, amount)| *amount)
            .sum::<i32>()
            < 20
        || !captured.opponent_life.is_some_and(|life| life <= 0)
    {
        return Err(format!(
            "the full duel did not continue through Saproling combat to the terminal opponent loss (tokens={:?}, token_attacks={:?}, damage={:?}, life={:?})",
            token_ids, captured.attacks_with_tokens, captured.combat_damage, captured.opponent_life
        ));
    }

    let events = &captured.events;
    let payment = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors.len() == 4 && colors.contains(&Color::White))).ok_or("Seed Spark exact white four-mana payment receipt missing")?;
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Seed Spark SpellCast receipt missing")?;
    let destroyed = unique_index(events, |event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target)).ok_or("Seed Spark did not record itself as source of the exact target destruction")?;
    let target_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("destroyed target did not move to its owner's graveyard")?;
    let token_events: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if token_ids.contains(token)).then_some(index))
        .collect();
    let resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Seed Spark SpellResolved receipt missing")?;
    let spell_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("resolved Seed Spark did not move to graveyard")?;
    let token_attack = first_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|attacker| token_ids.contains(attacker)))).ok_or("created Saprolings did not attack in combat")?;
    let token_damage = first_index(events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if token_ids.contains(source) && *amount == 1)).ok_or("Saproling combat damage receipt missing")?;
    let terminal = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    if !(payment < cast
        && cast < destroyed
        && destroyed < target_grave
        && token_events.len() == 2
        && target_grave < token_events[0]
        && token_events[0] < token_events[1]
        && token_events[1] < resolved
        && resolved < spell_grave
        && spell_grave < token_attack
        && token_attack < token_damage
        && token_damage < terminal)
    {
        return Err(format!(
            "Seed Spark printed resolution/combat ordering differs: payment={payment}, cast={cast}, destroyed={destroyed}, target_grave={target_grave}, tokens={token_events:?}, resolved={resolved}, spell_grave={spell_grave}, token_attack={token_attack}, token_damage={token_damage}, terminal={terminal}"
        ));
    }
    let projected = project_events(events);
    let indices: BTreeSet<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            let relevant = matches!(event, GameEvent::SpellManaPaid { card, .. } if *card == spell)
                || matches!(event, GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } if *card == spell)
                || matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target)
                || matches!(event, GameEvent::CardMoved { card, .. } if *card == target || *card == spell)
                || matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if token_ids.contains(token))
                || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|attacker| token_ids.contains(attacker)))
                || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if token_ids.contains(source))
                || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
            relevant.then_some(index)
        })
        .collect();
    let trace_events: Vec<_> = indices
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "id": "rav-seed-spark-destroys-target-then-creates-two-saprolings",
        "seed": seed,
        "decks": ["rav-seed-spark-slasher-lands-60", "rav-boros-signet-mountain-60"],
        "policies": ["rav.seed-spark-destroy-and-create-two-saprolings.v1", "rav.boros-signet-opponent-casts-and-passes.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "spell_object": spell.0,
        "target_object": target.0,
        "target_definition": captured.target_definition,
        "target_zone": format!("{:?}", captured.target_zone),
        "spell_zone": format!("{:?}", captured.spell_zone),
        "mana_paid": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "tokens": captured.tokens.iter().map(|token| json!({
            "object": token.id.0,
            "owner": token.controller.map(|player| player.0),
            "name": token.spec.as_ref().map(|spec| spec.name),
            "colors": token.colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "card_types": token.card_types.iter().map(|kind| format!("{kind:?}")).collect::<Vec<_>>(),
            "subtypes": token.creature_subtypes.iter().map(|subtype| format!("{subtype:?}")).collect::<Vec<_>>(),
            "power": token.power,
            "toughness": token.toughness,
            "keywords": token.keywords.iter().map(|keyword| format!("{keyword:?}")).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "token_damage": captured.combat_damage.iter().filter(|(source, _)| token_ids.contains(source)).map(|(source, amount)| json!({"source": source.0, "amount": amount})).collect::<Vec<_>>(),
        "opponent_life_after_combat": captured.opponent_life,
        "ordered_event_sequence": [payment, cast, destroyed, target_grave, token_events[0], token_events[1], resolved, spell_grave, token_attack, token_damage, terminal],
        "events": trace_events,
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Seed Spark",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "typed-artifact-or-enchantment-target",
            "destroy",
            "saproling-token-creation",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DestroyTargetArtifactOrEnchantment,
            Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            },
        ],
    }
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        .is_err())
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn first_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only",
        },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }
    })
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
