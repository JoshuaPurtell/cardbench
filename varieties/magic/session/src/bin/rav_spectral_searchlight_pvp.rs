//! Full-game diagnostic authority for RAV Spectral Searchlight.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color,
    DecisionKind, DecisionSelection, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SPECTRAL-SEARCHLIGHT";
const CARD_NAME: &str = "Spectral Searchlight";
const ABILITY_ID: &str = "tap-target-player-chosen-color-mana";
const COURIER_HAWK_ID: &str = "RAV-COURIER-HAWK";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/spectral_searchlight.rs";
const MAX_SEED: u64 = 64;
const MAX_TURNS: u32 = 64;
const MAX_POLICY_MOVES: u32 = 10_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/spectral_searchlight.rs"]
mod candidate_spectral_searchlight;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongSpellCost,
    NoTapCost,
    WrongTarget,
    ControllerManaInstead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActivationRequest {
    source: ObjectId,
    controller: PlayerId,
    target: PlayerId,
    source_tapped_on_activation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    activations: Vec<ActivationRequest>,
    searchlights_cast: Vec<ObjectId>,
    couriers_cast: Vec<ObjectId>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    controller_choice_rejected: bool,
    pilot_life: i64,
    opponent_life: i64,
}

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Spectral Searchlight PvP result: {error}");
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
            eprintln!("Spectral Searchlight PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_implementation()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Spectral Searchlight full-game replay differs".to_owned());
    }
    let mutations = json!({
        "wrong_spell_cost_mutation_detected": mutation_detected(seed, Mutation::WrongSpellCost)?,
        "tap_cost_removed_mutation_detected": mutation_detected(seed, Mutation::NoTapCost)?,
        "wrong_target_requirement_mutation_detected": mutation_detected(seed, Mutation::WrongTarget)?,
        "target_player_mana_effect_replaced_mutation_detected": mutation_detected(seed, Mutation::ControllerManaInstead)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_spell_cost_mutation_detected",
        "tap_cost_removed_mutation_detected",
        "wrong_target_requirement_mutation_detected",
        "target_player_mana_effect_replaced_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Spectral Searchlight semantic mutant escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":271,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2: the five colors of Magic",
                "106.1 and 106.4: mana production and payment",
                "117.3 and 117.4: priority and passing priority",
                "301.1 and 301.5: artifact permanents and characteristics",
                "602.1-602.2: activating an ability and paying its costs",
                "608.2-608.3: target legality and resolving an ability"
            ],
            "source_contract":"RAV #271 is a colorless artifact costing {3}; {T}: target player chooses a color, then that player adds one mana of that color.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"reference_pilot_pass; independent review and candidate trial remain",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
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
            .any(|card| card.collector_number == 271 && card.name == CARD_NAME)
    {
        return Err(
            "RAV catalog identity or denominator changed for Spectral Searchlight".to_owned(),
        );
    }
    Ok(cards)
}

fn validate_candidate_implementation() -> Result<(), String> {
    let definition = candidate_spectral_searchlight::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Spectral Searchlight definition missing from registry")?;
    if definition != registered_definition || definition != expected_definition() {
        return Err("Spectral Searchlight candidate identity differs from RAV #271".to_owned());
    }
    let ability = candidate_spectral_searchlight::activated_ability();
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Spectral Searchlight activated binding missing from registry")?;
    if ability != registered_ability.ability || ability != expected_ability() {
        return Err("Spectral Searchlight ability must tap, target a player, and grant that player a chosen color of mana".to_owned());
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::new(3),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "tap-target-player-chooses-one-color-mana",
            "target-player-chooses-one-color-mana",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: ABILITY_ID,
        mana_cost: ManaCost::new(0),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Player],
        effects: vec![Effect::AddOneManaOfTargetPlayersChosenColor],
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Spectral Searchlight source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Spectral Searchlight source is not valid Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Spectral Searchlight module must contain only imports and two direct constructors"
                .to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty()
                    && matches!(use_item.vis, syn::Visibility::Inherited)
                    && safe_use_tree(&use_item.tree) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"))
                {
                    return Err(format!(
                        "Spectral Searchlight {name} is not a pure literal constructor"
                    ));
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(format!(
                        "Spectral Searchlight {name} must return a direct struct literal"
                    ));
                };
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbility",
                    _ => return Err(format!("unexpected Spectral Searchlight function {name}")),
                };
                if !literal.path.is_ident(expected)
                    || literal.rest.is_some()
                    || !literal
                        .fields
                        .iter()
                        .all(|field| safe_expression(&field.expr))
                {
                    return Err(format!(
                        "Spectral Searchlight {name} contains unsupported literal behavior"
                    ));
                }
            }
            _ => {
                return Err(
                    "Spectral Searchlight source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()]) {
        return Err(
            "Spectral Searchlight candidate constructor API differs from the task contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn safe_use_tree(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        syn::UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet"
                | "ActivatedAbility"
                | "CardDefinition"
                | "CardType"
                | "Effect"
                | "ManaCost"
                | "TargetRequirement"
        ),
        syn::UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => {
            let path = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            matches!(path.as_slice(), [a, b] if (a == "CardType" && b == "Artifact") || (a == "Effect" && b == "AddOneManaOfTargetPlayersChosenColor") || (a == "TargetRequirement" && b == "Player"))
                || path.as_slice() == ["None"]
        }
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Call(call) => {
            let Expr::Path(path) = call.func.as_ref() else {
                return false;
            };
            let path = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            matches!(path.as_slice(), [a, b] if (a == "ManaCost" && b == "new") || (a == "BTreeSet" && (b == "new" || b == "from")))
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            literal.path.is_ident("ActivatedAbility")
                && literal.rest.is_none()
                && literal
                    .fields
                    .iter()
                    .all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            use syn::parse::Parser;
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(safe_expression))
        }
        Expr::Reference(reference) => {
            reference.mutability.is_none() && safe_expression(&reference.expr)
        }
        _ => false,
    }
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_spectral_searchlight::definition();
    if mutation == Mutation::WrongSpellCost {
        definition.mana_cost = ManaCost::new(2);
    }
    definition
}

fn candidate_ability(mutation: Mutation) -> ActivatedAbility {
    let mut ability = candidate_spectral_searchlight::activated_ability();
    match mutation {
        Mutation::NoTapCost => ability.tap_cost = false,
        Mutation::WrongTarget => ability.targets = vec![TargetRequirement::Creature],
        Mutation::ControllerManaInstead => {
            ability.effects = vec![Effect::AddManaController {
                color: Color::Blue,
                amount: 1,
            }];
        }
        Mutation::None | Mutation::WrongSpellCost => {}
    }
    ability
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    })
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SPECTRAL_SEARCHLIGHT_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_SPECTRAL_SEARCHLIGHT_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no complete target-player choice and terminal win found for seeds 0..={MAX_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (COURIER_HAWK_ID, 4), (PLAINS_ID, 52)]);
    let opponent = deck(&[(PLAINS_ID, 60)]);
    if [(&pilot, "pilot"), (&opponent, "opponent")]
        .iter()
        .any(|(deck, _)| {
            deck.mainboard
                .iter()
                .map(|entry| usize::from(entry.count))
                .sum::<usize>()
                != 60
        })
    {
        return Err("Spectral Searchlight duels require two legal 60-card decks".to_owned());
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Spectral Searchlight missing from definitions")? = candidate_definition(mutation);
    let mut abilities = rav_activated_ability_bindings();
    abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Spectral Searchlight ability missing from registry")?
        .ability = candidate_ability(mutation);
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, deck) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, deck)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, DeckMatchConfig::default().opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SearchlightPolicy::new(PlayerId(0))),
        Box::new(SearchlightPolicy::new(PlayerId(1))),
    ];
    let mut capture = Captured {
        events: Vec::new(),
        activations: Vec::new(),
        searchlights_cast: Vec::new(),
        couriers_cast: Vec::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        controller_choice_rejected: false,
        pilot_life: 20,
        opponent_life: 20,
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Spectral Searchlight full game exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Spectral Searchlight policy seat missing")?;
        let action = if view.pending_decision.is_some() {
            policy
                .propose_pending_decision(&view)
                .unwrap_or(PolicyAction::PassPriority)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        if let (
            Some(decision),
            PolicyAction::SubmitDecision {
                decision: submitted,
                selection: DecisionSelection::Color(color),
            },
        ) = (view.pending_decision.as_ref(), &action)
            && decision.id == *submitted
            && decision.kind == DecisionKind::TargetPlayerManaColor
            && decision.color_candidates.contains(color)
            && player == PlayerId(1)
            && !capture.controller_choice_rejected
        {
            let before = game.event_log.len();
            let opponent_blue = game
                .player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .mana_pool
                .amount(Color::Blue);
            let rejected = game.submit_policy_move(
                PlayerId(0),
                "rav.spectral-searchlight-authority-rejection-probe.v1",
                PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::Color(Color::Red),
                },
            );
            capture.controller_choice_rejected = rejected.is_err()
                && game.event_log.len() == before
                && game
                    .player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .mana_pool
                    .amount(Color::Blue)
                    == opponent_blue;
            if !capture.controller_choice_rejected {
                return Err(
                    "artifact controller was able to submit the target player's mana choice"
                        .to_owned(),
                );
            }
        }
        let activation = match &action {
            PolicyAction::ActivateAbility { activation }
                if activation.ability_id == ABILITY_ID
                    && activation.targets.len() == 1
                    && matches!(activation.targets[0], Target::Player(_)) =>
            {
                let Target::Player(target) = activation.targets[0] else {
                    unreachable!()
                };
                Some((activation.source, player, target))
            }
            _ => None,
        };
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Spectral Searchlight move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        if let Some((source, controller, target)) = activation {
            let source_tapped = game
                .object(source)
                .map_err(|error| error.to_string())?
                .tapped;
            capture.activations.push(ActivationRequest {
                source,
                controller,
                target,
                source_tapped_on_activation: source_tapped,
            });
        }
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellCast { card, .. } = event {
                match game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id
                {
                    CARD_ID => capture.searchlights_cast.push(*card),
                    COURIER_HAWK_ID => capture.couriers_cast.push(*card),
                    _ => {}
                }
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("Spectral Searchlight invariant failed: {error}"))?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|winner| winner.0);
    capture.turns = game.turn;
    capture.pilot_life = game
        .player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .life;
    capture.opponent_life = game
        .player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .life;
    Ok(capture)
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

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err(
            "Spectral Searchlight duel must reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if capture.activations.len() < 2
        || capture.activations[0].target != PlayerId(0)
        || capture.activations[1].target != PlayerId(1)
    {
        return Err(
            "Spectral Searchlight must target both the pilot and opponent in distinct activations"
                .to_owned(),
        );
    }
    if capture.activations[0].source == capture.activations[1].source
        || capture.activations.iter().any(|activation| {
            activation.controller != PlayerId(0) || !activation.source_tapped_on_activation
        })
    {
        return Err(
            "each Searchlight activation must tap a distinct source controlled by the pilot"
                .to_owned(),
        );
    }
    if !capture.controller_choice_rejected {
        return Err(
            "the artifact controller incorrectly answered the opponent's target-player choice"
                .to_owned(),
        );
    }
    if capture.searchlights_cast.len() < 2 {
        return Err("pilot must cast at least two Spectral Searchlights".to_owned());
    }
    let first_searchlight_payment = find_event(&capture.events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == capture.searchlights_cast[0] && colors.len() == 3 && colors.iter().all(|color| *color == Color::White)
    )).ok_or("first Spectral Searchlight did not receive its exact {3} spell payment")?;
    for (index, activation) in capture.activations.iter().take(2).enumerate() {
        let expected_color = if index == 0 {
            Color::White
        } else {
            Color::Blue
        };
        let activated = find_event_after(&capture.events, first_searchlight_payment.min(capture.events.len()-1), |event| matches!(event,
            GameEvent::AbilityActivated { player: PlayerId(0), source, definition, ability: ABILITY_ID, .. }
                if *source == activation.source && *definition == CARD_ID
        )).ok_or("Searchlight AbilityActivated receipt missing")?;
        let opened = find_event_after(&capture.events, activated, |event| {
            matches!(event,
                GameEvent::DecisionOpened { player, kind: DecisionKind::TargetPlayerManaColor, .. }
                    if *player == activation.target
            )
        })
        .ok_or("target-player mana-color decision did not go to the selected recipient")?;
        let completed = find_event_after(&capture.events, opened, |event| matches!(event,
            GameEvent::DecisionCompleted { player, kind: DecisionKind::TargetPlayerManaColor, .. }
                if *player == activation.target
        )).ok_or("target player's mana-color decision completion missing")?;
        let added = find_event_after(&capture.events, completed, |event| {
            matches!(event,
                GameEvent::ManaAdded { player, color, amount: 1 }
                    if *player == activation.target && *color == expected_color
            )
        })
        .ok_or("chosen mana was not added to the exact target player")?;
        let resolved = find_event_after(&capture.events, added, |event| matches!(event,
            GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == activation.source
        )).ok_or("Searchlight AbilityResolved receipt missing")?;
        if !(activated < opened && opened < completed && completed < added && added < resolved) {
            return Err("target-player choice and mana-resolution event order differs".to_owned());
        }
    }
    let courier = capture
        .couriers_cast
        .first()
        .copied()
        .ok_or("Courier Hawk was not cast")?;
    let courier_payment = find_event(&capture.events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == courier && colors.len() == 2 && colors.iter().all(|color| *color == Color::White)
    )).ok_or("Courier Hawk did not spend exactly {1}{W}")?;
    let courier_cast = find_event_after(&capture.events, courier_payment, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == courier
        )
    })
    .ok_or("Courier Hawk cast receipt missing")?;
    let courier_resolved = find_event_after(&capture.events, courier_cast, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == courier
        )
    })
    .ok_or("Courier Hawk resolution missing")?;
    let entered = find_event_after(&capture.events, courier_resolved, |event| matches!(event,
        GameEvent::CardMoved { card, to: cardbench_magic_engine::Zone::Battlefield } if *card == courier
    )).ok_or("Courier Hawk did not enter after resolution")?;
    let attack = find_event_after(&capture.events, entered, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&courier)
    )).ok_or("Courier Hawk did not attack")?;
    let damage = find_event_after(&capture.events, attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if *source == courier
    )).ok_or("Courier Hawk did not deal its expected one damage")?;
    let terminal = find_event(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Spectral Searchlight full match terminal winner event missing")?;
    if !(courier_payment < courier_cast
        && courier_cast < courier_resolved
        && courier_resolved < entered
        && entered < attack
        && attack < damage
        && damage < terminal)
    {
        return Err(
            "Searchlight mana, Courier Hawk cast, combat, and terminal ordering differs".to_owned(),
        );
    }
    let source_ids: [ObjectId; 2] = [capture.activations[0].source, capture.activations[1].source];
    let mut relevant = project_events(&capture.events)
        .into_iter()
        .filter(|event| {
            event.kind == "DecisionOpened"
                || event.kind == "DecisionCompleted"
                || event.kind == "GameEnded"
                || event.facts.objects.iter().any(|object| {
                    source_ids.iter().any(|source| source.0 == object.0) || object.0 == courier.0
                })
        })
        .collect::<Vec<_>>();
    relevant.sort_by_key(|event| event.sequence);
    Ok(json!({
        "id":"rav-spectral-searchlight-target-player-color-choice",
        "seed":seed,
        "decks":["rav_spectral_searchlight_pilot","rav_basic_plains_opponent"],
        "policies":["rav.spectral-searchlight-recipient-choice.v1","rav.spectral-searchlight-recipient-choice.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":capture.turns,
        "accepted_moves":capture.accepted_moves,"rejected_moves":capture.rejected_moves,
        "controller_choice_rejected":capture.controller_choice_rejected,
        "spell":{"name":CARD_NAME,"mana_cost":"{3}","payment":["White","White","White"],"casts":capture.searchlights_cast.iter().map(|card| card.0).collect::<Vec<_>>()},
        "activations":[
            {"source":capture.activations[0].source.0,"controller":capture.activations[0].controller.0,"target_player":capture.activations[0].target.0,"source_tapped":capture.activations[0].source_tapped_on_activation,"chosen_color":"White","mana_added_to_target":true},
            {"source":capture.activations[1].source.0,"controller":capture.activations[1].controller.0,"target_player":capture.activations[1].target.0,"source_tapped":capture.activations[1].source_tapped_on_activation,"chosen_color":"Blue","mana_added_to_target":true}
        ],
        "recipient_choice_order":["AbilityActivated","DecisionOpened","DecisionCompleted","ManaAdded","AbilityResolved"],
        "courier_hawk":{"object":courier.0,"mana_cost":"{1}{W}","payment":["White","White"],"cast_and_resolved":true,"attacked":true,"damage":1},
        "life_totals":[capture.pilot_life,capture.opponent_life],
        "terminal_sequence":terminal,"events":relevant,"passed":true
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
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status":if passed {"passed"} else {"not_qualified"},"credited":false,
        "qualification":if passed {"reference_pilot_pass; independent reviewer approval and candidate trial remain"} else {"not_evaluated"}})
}

struct SearchlightPolicy {
    player: PlayerId,
    searchlights_cast: u8,
    pilot_target_source: Option<ObjectId>,
    opponent_target_source: Option<ObjectId>,
    self_choice_turn: Option<u32>,
    courier_count: u8,
}

impl SearchlightPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            searchlights_cast: 0,
            pilot_target_source: None,
            opponent_target_source: None,
            self_choice_turn: None,
            courier_count: 0,
        }
    }

    fn untapped_plains(&self, view: &GameView) -> Option<ObjectId> {
        view.own_battlefield
            .iter()
            .find(|card| card.definition == Some(PLAINS_ID) && !card.tapped)
            .map(|card| card.id)
    }

    fn available_white(&self, view: &GameView) -> usize {
        usize::from(view.mana_pool.amount(Color::White))
            + view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(PLAINS_ID) && !card.tapped)
                .count()
    }

    fn tap_plains(&self, view: &GameView) -> Option<PolicyAction> {
        self.untapped_plains(view)
            .map(|land| PolicyAction::ActivateManaAbility {
                land,
                color: Color::White,
            })
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some(PLAINS_ID))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn activate_source(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0) {
            return None;
        }
        let source = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.definition == Some(CARD_ID)
                    && !card.tapped
                    && Some(card.id) != self.pilot_target_source
                    && Some(card.id) != self.opponent_target_source
            })?
            .id;
        let target = if self.pilot_target_source.is_none() {
            self.pilot_target_source = Some(source);
            PlayerId(0)
        } else if self.opponent_target_source.is_none() {
            self.opponent_target_source = Some(source);
            PlayerId(1)
        } else {
            return None;
        };
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(target)],
            },
        })
    }

    fn cast_searchlight(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0) || self.searchlights_cast >= 2 {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let threshold = if self.searchlights_cast == 0 { 4 } else { 3 };
        if self.available_white(view) < threshold {
            return self.tap_plains(view);
        }
        if view.mana_pool.amount(Color::White) < 3 {
            return self.tap_plains(view);
        }
        self.searchlights_cast += 1;
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::White; 3],
                hybrid: vec![],
            },
        })
    }

    fn cast_courier(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0) {
            return None;
        }
        let first_white_choice_now =
            self.self_choice_turn == Some(view.turn) && self.courier_count == 0;
        let later_courier = self.opponent_target_source.is_some() && self.courier_count < 4;
        if !first_white_choice_now && !later_courier {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(COURIER_HAWK_ID))?;
        if self.available_white(view) < 2 || view.mana_pool.amount(Color::White) < 2 {
            return self.tap_plains(view);
        }
        self.courier_count += 1;
        if first_white_choice_now {
            self.self_choice_turn = None;
        }
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::White],
                hybrid: vec![],
            },
        })
    }
}

impl CodePolicy for SearchlightPolicy {
    fn id(&self) -> &'static str {
        "rav.spectral-searchlight-recipient-choice.v1"
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::TargetPlayerManaColor {
            return None;
        }
        let color = if self.player == PlayerId(0) {
            Color::White
        } else {
            Color::Blue
        };
        if !decision.color_candidates.contains(&color) {
            return None;
        }
        if self.player == PlayerId(0) {
            self.self_choice_turn = Some(view.turn);
        }
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Color(color),
        })
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
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
        if view.player != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if let Some(action) = self.activate_source(view) {
            return action;
        }
        if let Some(action) = self.cast_courier(view) {
            return action;
        }
        if let Some(action) = self.cast_searchlight(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--output-root")
        .map(|args| args[1].clone().into())
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
