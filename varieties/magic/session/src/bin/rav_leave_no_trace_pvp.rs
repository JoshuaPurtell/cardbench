//! Full-game diagnostic authority for RAV Leave No Trace.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
const CARD_ID: &str = "RAV-LEAVE-NO-TRACE";
const TARGET_ID: &str = "TEST-WHITE-ENCHANTMENT-TARGET";
const MATCHING_ID: &str = "TEST-WHITE-ENCHANTMENT-MATCH";
const UNMATCHED_ID: &str = "TEST-BLUE-ENCHANTMENT";
const SIGNET_ID: &str = "RAV-BOROS-SIGNET";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const MOUNTAIN_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/leave_no_trace.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/leave_no_trace.rs"]
mod candidate_leave_no_trace;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Leave No Trace result: {error}");
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
            eprintln!("Leave No Trace PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoRadiance,
    TargetOnly,
    WrongCost,
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
    matching: Option<ObjectId>,
    matching_zone: Option<Zone>,
    unmatched: Option<ObjectId>,
    unmatched_zone: Option<Zone>,
    destroyed: Vec<ObjectId>,
    spell_zone: Option<Zone>,
    mana_paid: Vec<Color>,
    resolved: bool,
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
        return Err("Leave No Trace full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "radiance_instruction_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoRadiance)?,
        "shared_color_destruction_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::TargetOnly)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "radiance_instruction_removed_mutation_detected_in_game",
        "shared_color_destruction_removed_mutation_detected_in_game",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
        return Err(format!("Leave No Trace gameplay mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 23, "printings": 1, "definition_id": CARD_ID, "name": "Leave No Trace"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: instant spell casting and target selection",
                "608.2: spell and ability resolution",
                "701.7: destroying a permanent",
                "700.2 and 702.39: radiance chooses a permanent that shares a color",
                "508.1 and 510.1: declaring attackers and combat damage"
            ],
            "source_contract": "RAV Leave No Trace #23 is a white {1}{W} Instant that destroys target enchantment and each other enchantment that shares a color with it.",
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
    let mut definition = candidate_leave_no_trace::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoRadiance => definition
            .effects
            .retain(|effect| !matches!(effect, Effect::RadianceDestroyEnchantments)),
        Mutation::TargetOnly => {
            definition.effects = vec![Effect::DestroyTargetArtifactOrEnchantment];
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::White]),
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_leave_no_trace::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Leave No Trace candidate or registry differs from the exact RAV #23 identity and Radiance instruction")?;
    if registered != candidate
        || candidate.id != CARD_ID
        || candidate.name != "Leave No Trace"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(1, [Color::White])
        || candidate.colors != BTreeSet::from([Color::White])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "radiance-enchantment-destruction",
            ]
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || candidate.effects != [Effect::RadianceDestroyEnchantments]
    {
        return Err("Leave No Trace candidate or registry differs from the exact RAV #23 identity and Radiance instruction".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Leave No Trace source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Leave No Trace source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err(
            "Leave No Trace module must contain only private imports and its direct-literal definition"
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
                    return Err("Leave No Trace source contains an unsupported import".to_owned());
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
                        "Leave No Trace must expose one crate-private, direct-literal CardDefinition"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Leave No Trace source contains an executable or unsupported item".to_owned(),
                );
            }
        }
    }
    if imports != 2 || definitions != 1 {
        return Err(
            "Leave No Trace imports or definition count differs from the source contract".to_owned(),
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
                || path_is(&segments, &["Effect", "RadianceDestroyEnchantments"])
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
                || path_is(&segments, &["BTreeSet", "new"]);
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
                && literal.fields.iter().all(|field| safe_expression(&field.expr))
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
        .ok_or("Leave No Trace definition missing during game construction")? =
        candidate_value(mutation);
    definitions.extend([
        test_enchantment(TARGET_ID, "Radiance target", Color::White),
        test_enchantment(MATCHING_ID, "Shared-color enchantment", Color::White),
        test_enchantment(UNMATCHED_ID, "Unmatched-color enchantment", Color::Blue),
    ]);
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

fn test_enchantment(id: &'static str, name: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: "TEST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["radiance-test-fixture"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_LEAVE_NO_TRACE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_LEAVE_NO_TRACE_SEED must be an unsigned integer".to_owned())?;
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
        "no Leave No Trace full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        (PLAINS_ID, 26),
        (FOREST_ID, 26),
    ]);
    let opponent = deck(&[(SIGNET_ID, 4), (MOUNTAIN_ID, 56)]);
    for (label, list) in [
        ("Leave No Trace pilot", &pilot),
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
    let target = game
        .put_on_battlefield(PlayerId(1), TARGET_ID)
        .map_err(|error| error.to_string())?;
    let matching = game
        .put_on_battlefield(PlayerId(0), MATCHING_ID)
        .map_err(|error| error.to_string())?;
    let unmatched = game
        .put_on_battlefield(PlayerId(1), UNMATCHED_ID)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(LeaveNoTracePolicy::new(PlayerId(0), mutation)),
        Box::new(SignetPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        spell: None,
        target: Some(target),
        target_definition: None,
        target_zone: Some(Zone::Battlefield),
        matching: Some(matching),
        matching_zone: Some(Zone::Battlefield),
        unmatched: Some(unmatched),
        unmatched_zone: Some(Zone::Battlefield),
        destroyed: vec![],
        spell_zone: None,
        mana_paid: vec![],
        resolved: false,
        combat_damage: vec![],
        opponent_life: None,
    };
    while !game.is_game_over() {
        if game.turn > 42 || captured.accepted >= 6_000 {
            return Err(format!(
                "Leave No Trace PvP exceeded bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Leave No Trace policy seat missing")?;
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
            let selected = request.targets.iter().find_map(|target| match target {
                Target::Permanent(object) => Some(*object),
                _ => None,
            });
            if selected != captured.target {
                return Err("Leave No Trace did not target the seeded white enchantment".to_owned());
            }
        }
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Leave No Trace policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game.event_log[before..], &mut captured)?;
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

fn observe(events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::CardDestroyed { source, card } if Some(*source) == captured.spell => {
                captured.destroyed.push(*card);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == captured.spell => captured.mana_paid = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == captured.spell => {
                captured.resolved = true;
            }
            GameEvent::CardMoved { card, to } if Some(*card) == captured.target => captured.target_zone = Some(*to),
            GameEvent::CardMoved { card, to } if Some(*card) == captured.matching => captured.matching_zone = Some(*to),
            GameEvent::CardMoved { card, to } if Some(*card) == captured.unmatched => captured.unmatched_zone = Some(*to),
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

struct LeaveNoTracePolicy {
    player: PlayerId,
    mutation: Mutation,
    spell_cast: bool,
    attacker_cast: bool,
}

impl LeaveNoTracePolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            spell_cast: false,
            attacker_cast: false,
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
        let have_green = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Green)
        });
        let preferred = if have_white { FOREST_ID } else { PLAINS_ID };
        let secondary = if have_green { PLAINS_ID } else { FOREST_ID };
        view.hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(secondary))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(PLAINS_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(FOREST_ID))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for LeaveNoTracePolicy {
    fn id(&self) -> &'static str {
        "rav.leave-no-trace-radiance-and-watchwolf.v1"
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
                    .find(|card| card.definition == Some(TARGET_ID))
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
                && !self.attacker_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = self.cast_or_make_mana(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.attacker_cast = true;
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
        return Err("Leave No Trace duel did not end in a clean P0 terminal win".to_owned());
    }
    let spell = captured.spell.ok_or("Leave No Trace was not cast")?;
    let target = captured.target.ok_or("Leave No Trace did not select its target")?;
    let matching = captured.matching.ok_or("shared-color fixture missing")?;
    let unmatched = captured.unmatched.ok_or("off-color fixture missing")?;
    if captured.target_definition != Some(TARGET_ID)
        || captured.target_zone != Some(Zone::Graveyard)
        || captured.matching_zone != Some(Zone::Graveyard)
        || captured.unmatched_zone != Some(Zone::Battlefield)
        || captured.spell_zone != Some(Zone::Graveyard)
        || !captured.resolved
    {
        return Err(format!(
            "Leave No Trace failed its Radiance zone contract (target={:?}/{:?}, shared={:?}/{:?}, off_color={:?}/{:?}, spell={:?}, resolved={})",
            captured.target_definition,
            captured.target_zone,
            captured.matching,
            captured.matching_zone,
            captured.unmatched,
            captured.unmatched_zone,
            captured.spell_zone,
            captured.resolved
        ));
    }
    if candidate_leave_no_trace::definition() != expected_definition() {
        return Err("Leave No Trace candidate differs from its RAV #23 identity, {1}{W} cost, or Radiance instruction".to_owned());
    }
    if captured.destroyed.iter().copied().collect::<BTreeSet<_>>()
        != BTreeSet::from([target, matching])
        || captured.destroyed.len() != 2
    {
        return Err(format!(
            "Radiance must destroy exactly the target and the other white enchantment: {:?}",
            captured.destroyed
        ));
    }
    if captured.mana_paid.len() != 2
        || captured.mana_paid.iter().filter(|color| **color == Color::White).count() != 1
    {
        return Err(format!(
            "Leave No Trace must pay exactly {{1}}{{W}}, including one white: {:?}",
            captured.mana_paid
        ));
    }
    if !captured.opponent_life.is_some_and(|life| life <= 0)
        || captured.combat_damage.iter().map(|(_, amount)| *amount).sum::<i32>() < 20
    {
        return Err("the full duel did not continue to a terminal P0 win".to_owned());
    }

    let events = &captured.events;
    let payment = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors.len() == 2 && colors.iter().filter(|color| **color == Color::White).count() == 1)).ok_or("exact white {1}{W} payment receipt missing")?;
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Leave No Trace SpellCast receipt missing")?;
    let target_destroyed = unique_index(events, |event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target)).ok_or("Leave No Trace did not destroy its selected target")?;
    let matching_destroyed = unique_index(events, |event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == matching)).ok_or("Radiance did not destroy the shared-color enchantment")?;
    let target_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("target did not move to its owner's graveyard")?;
    let matching_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == matching)).ok_or("shared-color enchantment did not move to its owner's graveyard")?;
    let resolved = unique_index(events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell)).ok_or("Leave No Trace SpellResolved receipt missing")?;
    let spell_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("resolved Leave No Trace did not move to graveyard")?;
    let terminal = unique_index(events, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal P0 GameEnded receipt missing")?;
    if !(payment < cast
        && cast < target_destroyed
        && cast < matching_destroyed
        && target_destroyed < target_grave
        && matching_destroyed < matching_grave
        && target_grave < resolved
        && matching_grave < resolved
        && resolved < spell_grave
        && spell_grave < terminal)
    {
        return Err(format!(
            "Leave No Trace cast/Radiance/resolve ordering invalid: payment={payment}, cast={cast}, destroys=({target_destroyed},{matching_destroyed}), moves=({target_grave},{matching_grave}), resolved={resolved}, spell_grave={spell_grave}, terminal={terminal}"
        ));
    }
    let projected = project_events(events);
    let indices: BTreeSet<_> = events.iter().enumerate().filter_map(|(index, event)| {
        let relevant = matches!(event, GameEvent::SpellManaPaid { card, .. } if *card == spell)
            || matches!(event, GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } if *card == spell)
            || matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && (*card == target || *card == matching))
            || matches!(event, GameEvent::CardMoved { card, .. } if *card == target || *card == matching || *card == spell)
            || matches!(event, GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } if *amount > 0)
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        relevant.then_some(index)
    }).collect();
    let trace_events: Vec<_> = indices.iter().map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string())).collect::<Result<_, _>>()?;
    let mut ordered_event_sequence = vec![
        payment,
        cast,
        target_destroyed,
        target_grave,
        matching_destroyed,
        matching_grave,
        resolved,
        spell_grave,
        terminal,
    ];
    ordered_event_sequence.sort_unstable();
    Ok(json!({
        "id": "rav-leave-no-trace-destroys-target-and-shared-color-enchantment",
        "seed": seed,
        "decks": ["rav-leave-no-trace-watchwolf-lands-60", "rav-boros-signet-mountain-60"],
        "policies": ["rav.leave-no-trace-radiance-and-watchwolf.v1", "rav.boros-signet-opponent-casts-and-passes.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "spell_object": spell.0,
        "target_object": target.0,
        "target_definition": captured.target_definition,
        "target_controller_seat": 1,
        "target_zone": format!("{:?}", captured.target_zone),
        "matching_object": matching.0,
        "matching_definition": MATCHING_ID,
        "matching_controller_seat": 0,
        "matching_zone": format!("{:?}", captured.matching_zone),
        "unmatched_object": unmatched.0,
        "unmatched_definition": UNMATCHED_ID,
        "unmatched_controller_seat": 1,
        "unmatched_zone": format!("{:?}", captured.unmatched_zone),
        "destroyed_objects": captured.destroyed.iter().map(|object| object.0).collect::<Vec<_>>(),
        "spell_zone": format!("{:?}", captured.spell_zone),
        "mana_paid": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "opponent_life_after_combat": captured.opponent_life,
        "ordered_event_sequence": ordered_event_sequence,
        "events": trace_events,
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Leave No Trace",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "radiance-enchantment-destruction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceDestroyEnchantments],
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
