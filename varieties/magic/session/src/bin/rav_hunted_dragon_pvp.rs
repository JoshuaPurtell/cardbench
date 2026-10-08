//! Full-game diagnostic authority for RAV Hunted Dragon.

use cardbench_magic_engine::{
    CardType, Color, CreatureSubtype, DecisionKind, DecisionSelection, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TargetRequirement, TokenSpec, TriggerCondition, Zone,
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
const CARD_ID: &str = "RAV-HUNTED-DRAGON";
const CARD_NAME: &str = "Hunted Dragon";
const TRIGGER_ID: &str = "etb-opponent-knights";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_dragon.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const SEED: u64 = 0;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Hunted Dragon definition and targeted Knight ETB.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed red 6/6 Dragon with Flying and Haste.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-DRAGON",
        name: "Hunted Dragon",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "haste",
            "etb-targeted-opponent-knight-tokens",
        ],
        power: Some(6),
        toughness: Some(6),
        keywords: vec![Keyword::Flying, Keyword::Haste],
        effects: vec![],
    }
}

/// The targeted opponent creates three white 2/2 Knight tokens with First Strike.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-DRAGON",
        ability: TriggeredAbility {
            id: "etb-opponent-knights",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::knight(),
                count: 3,
            }],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/hunted_dragon.rs"]
mod candidate_hunted_dragon;

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
            eprintln!("Hunted Dragon authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // Validate the complete source before the first candidate function call.
    validate_candidate_source()?;
    let side_effect_rejected = malicious_candidate_source_rejected()?;
    if !side_effect_rejected {
        return Err("compile-valid candidate side effect was not rejected before execution".into());
    }
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let reference = run_match(Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Hunted Dragon full-game replay was not deterministic".into());
    }
    let mutations = json!({
        "body_changed_detected_in_game": mutation_detected(Mutation::WrongBody),
        "flying_removed_detected_in_game": mutation_detected(Mutation::NoFlying),
        "haste_removed_detected_in_game": mutation_detected(Mutation::NoHaste),
        "mana_cost_changed_detected_in_game": mutation_detected(Mutation::WrongManaCost),
        "etb_trigger_condition_changed_detected_in_game": mutation_detected(Mutation::WrongCondition),
        "opponent_target_requirement_changed_detected_in_game": mutation_detected(Mutation::WrongTarget),
        "knight_characteristics_changed_detected_in_game": mutation_detected(Mutation::WrongToken),
        "knight_count_changed_detected_in_game": mutation_detected(Mutation::WrongCount),
        "compile_valid_candidate_side_effect_rejected_before_execution": side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "body_changed_detected_in_game",
        "flying_removed_detected_in_game",
        "haste_removed_detected_in_game",
        "mana_cost_changed_detected_in_game",
        "etb_trigger_condition_changed_detected_in_game",
        "opponent_target_requirement_changed_detected_in_game",
        "knight_characteristics_changed_detected_in_game",
        "knight_count_changed_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Hunted Dragon semantic mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":131,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["110 and 111 Permanents and tokens","117 Timing and priority","202.1-202.2 Colors and mana cost","302.1 Creature characteristics","601.2 Casting spells","603.2 and 603.3 ETB trigger and target selection","608.2 Resolving triggered abilities","702.9 Flying","702.10 Haste","702.7 First Strike","701.6 Creating tokens"],"source_contract":"CardBench-authored semantic summary: Hunted Dragon is a red {3}{R}{R} 6/6 creature with flying and haste; when it enters the battlefield, target opponent creates three white 2/2 Knight creature tokens with first strike.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,"mutation_sensitivity":mutations,"traces":[trace_json(&reference)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hunted Dragon source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Hunted Dragon source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Hunted Dragon source AST differs from task contract".into());
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
                    _ => return Err("Hunted Dragon source AST differs from task contract".into()),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err("Hunted Dragon source AST differs from task contract".into());
                };
                if !return_type_is(&item.sig.output, ty)
                    || !path_is_type(&literal.path, ty)
                    || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err("Hunted Dragon source AST differs from task contract".into());
                }
            }
            _ => return Err("Hunted Dragon source AST differs from task contract".into()),
        }
    }
    let expected =
        syn::parse_file(EXPECTED_CANDIDATE_SOURCE).expect("trusted source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || source != EXPECTED_CANDIDATE_SOURCE
        || expected.items.len() != 4
    {
        return Err("Hunted Dragon source AST differs from task contract".into());
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
        .map_err(|error| format!("could not read Hunted Dragon source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-hunted-dragon-candidate-executed\", b\"executed\");";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Hunted Dragon side-effect mutation could not be constructed".into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_hunted_dragon::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Hunted Dragon definition is missing from the RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Red, Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(6)
        || definition.toughness != Some(6)
        || definition.keywords != vec![Keyword::Flying, Keyword::Haste]
        || definition.is_basic_land
        || !definition.effects.is_empty()
    {
        return Err("Hunted Dragon literal is outside the task source contract".into());
    }
    let binding = candidate_hunted_dragon::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID)
        .ok_or("Hunted Dragon opponent-targeted ETB binding is missing from the RAV registry")?;
    if binding != registered
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.optional
        || binding.ability.targets != vec![TargetRequirement::Opponent]
        || binding.ability.effects
            != vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::knight(),
                count: 3,
            }]
    {
        return Err("Hunted Dragon trigger binding is outside the task source contract".into());
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
        return Err("RAV catalog denominator differs from source contract".into());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongBody,
    NoFlying,
    NoHaste,
    WrongManaCost,
    WrongCondition,
    WrongTarget,
    WrongToken,
    WrongCount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenFact {
    object: ObjectId,
    controller: PlayerId,
    white: bool,
    creature: bool,
    knight: bool,
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
    dragon: Option<ObjectId>,
    paid_colors: Vec<Color>,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
    haste: bool,
    target: Option<PlayerId>,
    legal_target_seats: Vec<usize>,
    trigger_stacked: bool,
    trigger_resolved: bool,
    source_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    tokens: Vec<TokenFact>,
    both_alive_at_creation: bool,
    tokens_at_resolution: usize,
    both_alive_at_resolution: bool,
    entered_turn: Option<u32>,
    attack_turn: Option<u32>,
    attacked: bool,
    combat_damage: i32,
}

fn run_match(mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Hunted Dragon definition missing")?;
    *definition = candidate_hunted_dragon::definition();
    match mutation {
        Mutation::WrongBody => {
            definition.power = Some(5);
            definition.toughness = Some(5);
        }
        Mutation::NoFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::NoHaste => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Haste),
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Red, Color::Red])
        }
        _ => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    let trigger = triggers
        .iter()
        .position(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID);
    match mutation {
        Mutation::WrongCondition
        | Mutation::WrongTarget
        | Mutation::WrongToken
        | Mutation::WrongCount => {
            let index = trigger.ok_or("Hunted Dragon trigger missing for mutation")?;
            triggers[index] = candidate_hunted_dragon::triggered_ability();
            match mutation {
                Mutation::WrongCondition => {
                    triggers[index].ability.condition = TriggerCondition::BeginningOfUpkeep
                }
                Mutation::WrongTarget => {
                    triggers[index].ability.targets = vec![TargetRequirement::Player];
                    triggers[index].ability.effects = vec![Effect::CreateTokenForTargetPlayer {
                        token: TokenSpec::knight(),
                        count: 3,
                    }];
                }
                Mutation::WrongToken => {
                    let mut token = TokenSpec::knight();
                    token.keywords.clear();
                    triggers[index].ability.effects =
                        vec![Effect::CreateTokenForTargetOpponent { token, count: 3 }];
                }
                Mutation::WrongCount => {
                    triggers[index].ability.effects = vec![Effect::CreateTokenForTargetOpponent {
                        token: TokenSpec::knight(),
                        count: 2,
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
                card: "RAV-MOUNTAIN".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".into(),
            count: 60,
        }],
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
        return Err("Hunted Dragon task decks must contain 60 cards".into());
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
        Box::new(DragonPolicy::new(PlayerId(0), mutation)),
        Box::new(DragonPolicy::new(PlayerId(1), mutation)),
    ];
    let mut c = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        dragon: None,
        paid_colors: vec![],
        power: None,
        toughness: None,
        flying: false,
        haste: false,
        target: None,
        trigger_stacked: false,
        trigger_resolved: false,
        source_incarnation: None,
        resolved_incarnation: None,
        legal_target_seats: vec![],
        tokens: vec![],
        both_alive_at_creation: false,
        tokens_at_resolution: 0,
        both_alive_at_resolution: false,
        entered_turn: None,
        attack_turn: None,
        attacked: false,
        combat_damage: 0,
    };
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || c.accepted >= MAX_MOVES {
            return Err(format!(
                "Hunted Dragon game exceeded bounds at turn {} step {:?}, life {:?}",
                game.turn,
                game.step,
                game.players.iter().map(|p| p.life).collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        if let Some(choice) = view
            .triggered_ability_target_choice
            .as_ref()
            .filter(|choice| choice.ability == TRIGGER_ID)
        {
            c.legal_target_seats = choice
                .target_options
                .first()
                .map(|targets| {
                    targets
                        .iter()
                        .filter_map(|target| match target {
                            Target::Player(player) => Some(player.0),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
                    "Hunted Dragon policy rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        c.accepted += 1;
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
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    c.dragon = Some(*card);
                    c.paid_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    c.dragon = Some(*card)
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    c.dragon = Some(*card);
                    c.entered_turn = Some(game.turn);
                }
                GameEvent::SpellResolved { card } if Some(*card) == c.dragon => {
                    let characteristics = game.characteristics(*card).map_err(|e| e.to_string())?;
                    c.power = characteristics.power;
                    c.toughness = characteristics.toughness;
                    c.flying = characteristics.keywords.contains(&Keyword::Flying);
                    c.haste = characteristics.keywords.contains(&Keyword::Haste);
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                    ..
                } if Some(*source) == c.dragon => {
                    c.trigger_stacked = true;
                    c.source_incarnation = Some(*source_incarnation);
                    c.target = game
                        .stack
                        .last()
                        .and_then(|stack| match stack.targets.first() {
                            Some(Target::Player(player)) => Some(*player),
                            _ => None,
                        });
                }
                GameEvent::TokenCreated { player, token } => {
                    let characteristics =
                        game.characteristics(*token).map_err(|e| e.to_string())?;
                    c.tokens.push(TokenFact {
                        object: *token,
                        controller: *player,
                        white: characteristics.colors.contains(&Color::White),
                        creature: characteristics.card_types.contains(&CardType::Creature),
                        knight: characteristics
                            .creature_subtypes
                            .contains(&CreatureSubtype::Knight),
                        power: characteristics.power,
                        toughness: characteristics.toughness,
                        keywords: characteristics
                            .keywords
                            .iter()
                            .map(|keyword| format!("{keyword:?}"))
                            .collect(),
                    });
                    c.both_alive_at_creation =
                        !game.is_game_over() && game.players.iter().all(|player| player.life > 0);
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                } if Some(*source) == c.dragon => {
                    c.trigger_resolved = true;
                    c.resolved_incarnation = Some(*source_incarnation);
                    c.tokens_at_resolution = c.tokens.len();
                    c.both_alive_at_resolution =
                        !game.is_game_over() && game.players.iter().all(|player| player.life > 0);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if c.dragon.is_some_and(|dragon| attackers.contains(&dragon)) => {
                    c.attacked = true;
                    c.attack_turn = Some(game.turn);
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if Some(*source) == c.dragon => c.combat_damage += *amount,
                _ => {}
            }
        }
    }
    c.events = game.event_log.clone();
    c.winner = game.winner().map(|player| player.0);
    c.turns = game.turn;
    c.rejected = attempted.saturating_sub(c.accepted);
    let projected = project_events(&c.events);
    c.entered_turn = c
        .dragon
        .and_then(|dragon| event_index(&c.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == dragon)))
        .and_then(|index| projected.get(index).map(|event| event.turn));
    c.attack_turn = c
        .dragon
        .and_then(|dragon| event_index(&c.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&dragon))))
        .and_then(|index| projected.get(index).map(|event| event.turn));
    Ok(c)
}

struct DragonPolicy {
    player: PlayerId,
    mutation: Mutation,
    cast_once: bool,
}
impl DragonPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            cast_once: false,
        }
    }
    fn target(&self, view: &GameView) -> Option<PolicyAction> {
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
    fn cost(&self) -> ManaCost {
        if self.mutation == Mutation::WrongManaCost {
            ManaCost::with_colors(2, [Color::Red, Color::Red])
        } else {
            ManaCost::with_colors(3, [Color::Red, Color::Red])
        }
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let red_required = cost
            .colored
            .iter()
            .filter(|color| **color == Color::Red)
            .count() as u8;
        if view.mana_pool.amount(Color::Red) < red_required
            || view.mana_pool.total() < cost.generic as u8 + red_required
        {
            return None;
        }
        Some(ManaPaymentSelection {
            generic: vec![Color::Red; cost.generic as usize],
            hybrid: vec![],
        })
    }
}
impl CodePolicy for DragonPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.hunted-dragon-controller.v1"
        } else {
            "rav.hunted-dragon-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if let Some(action) = self.target(view) {
            return action;
        }
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
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0)
            && !self.cast_once
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            let cost = self.cost();
            if let Some(mana_selection) = Self::payment(view, &cost) {
                let card = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .expect("checked");
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some("RAV-MOUNTAIN") && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        if let Some(action) = self.target(view) {
            return Some(action);
        }
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::CleanupDiscard {
            return None;
        }
        let cards = decision
            .candidates
            .iter()
            .take(usize::from(decision.min_selections))
            .map(|card| card.id)
            .collect();
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(cards),
        })
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    let dragon = c.dragon.ok_or("Hunted Dragon was not cast")?;
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Hunted Dragon duel did not reach a clean terminal P0 win".into());
    }
    if c.paid_colors != vec![Color::Red; 5]
        || c.power != Some(6)
        || c.toughness != Some(6)
        || !c.flying
        || !c.haste
    {
        return Err("Hunted Dragon body, keywords, or exact {3}{R}{R} payment differs".into());
    }
    if c.target != Some(PlayerId(1))
        || c.legal_target_seats != vec![1]
        || !c.trigger_stacked
        || !c.trigger_resolved
        || c.source_incarnation.is_none()
        || c.source_incarnation != c.resolved_incarnation
    {
        return Err("Hunted Dragon ETB did not target its opponent and resolve for the same source incarnation".into());
    }
    if c.tokens.len() != 3
        || c.tokens_at_resolution != 3
        || c.tokens.iter().any(|token| {
            token.controller != PlayerId(1)
                || !token.white
                || !token.creature
                || !token.knight
                || token.power != Some(2)
                || token.toughness != Some(2)
                || token.keywords != vec!["FirstStrike"]
        })
    {
        return Err(format!(
            "Hunted Dragon did not create exactly three white 2/2 First Strike Knight tokens: {:?}",
            c.tokens
        ));
    }
    if !c.both_alive_at_creation || !c.both_alive_at_resolution {
        return Err(
            "Hunted Dragon tokens were not captured while both players remained in the game".into(),
        );
    }
    if !c.attacked
        || c.entered_turn.is_none()
        || c.attack_turn != c.entered_turn
        || c.combat_damage < 20
    {
        let combat_debug: Vec<_> = c
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(
                    event,
                    GameEvent::CardMoved {
                        to: Zone::Battlefield,
                        ..
                    } | GameEvent::SpellResolved { .. }
                        | GameEvent::AttackersDeclared { .. }
                )
            })
            .collect();
        return Err(format!(
            "Hunted Dragon did not demonstrate same-turn Haste combat through a terminal win: entry={:?}, attack={:?}, damage={}, events={:?}",
            c.entered_turn, c.attack_turn, c.combat_damage, combat_debug
        ));
    }
    let paid = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == dragon && colors == &vec![Color::Red; 5])).ok_or("exact {3}{R}{R} payment receipt is missing")?;
    let cast = event_index(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == dragon)).ok_or("Hunted Dragon cast receipt is missing")?;
    let resolved = event_index(
        &c.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == dragon),
    )
    .ok_or("Hunted Dragon spell resolution receipt is missing")?;
    let entered = event_index_after(&c.events, resolved, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == dragon)
    })
    .ok_or("Hunted Dragon battlefield entry receipt is missing")?;
    let stacked = event_index(&c.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == dragon)).ok_or("Hunted Dragon ETB stack receipt is missing")?;
    let first_token = event_index(&c.events, |event| {
        matches!(
            event,
            GameEvent::TokenCreated {
                player: PlayerId(1),
                ..
            }
        )
    })
    .ok_or("opponent Knight TokenCreated receipt is missing")?;
    let last_token = event_index_after(&c.events, first_token, |event| {
        matches!(
            event,
            GameEvent::TokenCreated {
                player: PlayerId(1),
                ..
            }
        )
    })
    .and_then(|first| {
        event_index_after(&c.events, first, |event| {
            matches!(
                event,
                GameEvent::TokenCreated {
                    player: PlayerId(1),
                    ..
                }
            )
        })
    })
    .ok_or("all three Knight TokenCreated receipts are required")?;
    let ability = event_index_after(&c.events, last_token, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == dragon)).ok_or("Hunted Dragon ETB resolution receipt is missing")?;
    let attack = event_index_after(&c.events, ability, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&dragon))).ok_or("same-turn Hunted Dragon attack event is missing")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == dragon && *amount > 0)).ok_or("Hunted Dragon combat damage receipt is missing")?;
    let end = event_index(&c.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt is missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < stacked
        && stacked < first_token
        && last_token < ability
        && ability < attack
        && attack < damage
        && damage < end)
    {
        return Err("Hunted Dragon spell, ETB, token, and combat event order differs".into());
    }
    let projected = project_events(&c.events);
    if projected.get(entered).map(|event| event.turn)
        != projected.get(attack).map(|event| event.turn)
    {
        return Err("Hunted Dragon attack did not occur on the battlefield-entry turn".into());
    }
    Ok(())
}

fn mutation_detected(mutation: Mutation) -> bool {
    run_match(mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}
fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
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
    let source = c.dragon;
    let projected = project_events(&c.events);
    let events: Vec<_> = c.events.iter().enumerate().filter_map(|(index, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => source == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Battlefield } => source == Some(*card),
            GameEvent::TriggeredAbilityStacked { source: card, ability: TRIGGER_ID, .. } | GameEvent::AbilityResolved { source: card, ability: TRIGGER_ID, .. } => source == Some(*card),
            GameEvent::TokenCreated { player: PlayerId(1), .. } => true,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => source.is_some_and(|id| attackers.contains(&id)),
            GameEvent::DamageDealtToPlayer { source: card, player: PlayerId(1), amount } => source == Some(*card) && *amount > 0,
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical}))
    }).collect();
    let token_json: Vec<_> = c.tokens.iter().map(|token| json!({"object":token.object.0,"controller_seat":token.controller.0,"white":token.white,"creature":token.creature,"knight_subtype":token.knight,"power":token.power,"toughness":token.toughness,"keywords":token.keywords})).collect();
    json!({"id":"rav-hunted-dragon-opponent-knights-etb-haste","passed":true,"terminal":c.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"dragon_object":source.map(|id|id.0),"etb":{"source_incarnation":c.source_incarnation,"resolved_incarnation":c.resolved_incarnation,"target_seat":c.target.map(|p|p.0),"legal_target_seats":c.legal_target_seats,"tokens":token_json,"tokens_at_resolution":c.tokens_at_resolution,"both_players_alive_at_creation":c.both_alive_at_creation,"both_players_alive_at_resolution":c.both_alive_at_resolution},"dragon":{"mana_cost":"{3}{R}{R}","paid_colors":c.paid_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":c.power,"toughness":c.toughness,"flying":c.flying,"haste":c.haste,"entered_turn":c.entered_turn,"attack_turn":c.attack_turn,"same_turn_attack":c.entered_turn.is_some() && c.entered_turn == c.attack_turn,"attacked":c.attacked,"combat_damage":c.combat_damage},"events":events})
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
    let passed = card.collector_number == 131;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
