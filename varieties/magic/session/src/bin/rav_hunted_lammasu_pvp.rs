//! Full-game diagnostic authority for RAV Hunted Lammasu.

use cardbench_magic_engine::{
    CardType, Color, CreatureSubtype, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TokenSpec, TriggerCondition,
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
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-HUNTED-LAMMASU";
const CARD_NAME: &str = "Hunted Lammasu";
const TRIGGER_ID: &str = "etb-opponent-horror";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_lammasu.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/hunted_lammasu.rs"]
mod candidate_hunted_lammasu;

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
            eprintln!("Hunted Lammasu authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let reference = run_match(Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Hunted Lammasu full-game replay was not deterministic".into());
    }
    let mutations = json!({
        "flying_removed_detected_in_game": mutation_detected(Mutation::NoFlying),
        "etb_trigger_removed_detected_in_game": mutation_detected(Mutation::RemoveTrigger),
        "opponent_target_requirement_removed_detected_in_game": mutation_detected(Mutation::WrongTarget),
        "horror_token_changed_detected_in_game": mutation_detected(Mutation::WrongToken),
        "mana_cost_changed_detected_in_game": mutation_detected(Mutation::WrongManaCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_removed_detected_in_game",
        "etb_trigger_removed_detected_in_game",
        "opponent_target_requirement_removed_detected_in_game",
        "horror_token_changed_detected_in_game",
        "mana_cost_changed_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Hunted Lammasu semantic mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":22,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["110 and 111 Permanents and tokens","117 Timing and priority","202.1-202.2 Colors and mana cost","302.1 Creature characteristics","601.2 Casting spells","603.2 and 603.3 ETB trigger and target selection","608.2 Resolving triggered abilities","701.6 Creating tokens"],"source_contract":"CardBench-authored semantic summary: Hunted Lammasu is a white 2WW 5/5 creature with flying; when it enters the battlefield, target opponent creates a black 4/4 Horror creature token.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,"mutation_sensitivity":mutations,"traces":[trace_json(&reference)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hunted Lammasu source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Hunted Lammasu source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Hunted Lammasu source has unsupported module attributes".into());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TokenSpec"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Hunted Lammasu imports differ from the source contract".into());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(&item.vis, Visibility::Restricted(_))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none() =>
            {
                let output = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => return Err("Hunted Lammasu exposes an unsupported function".into()),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err("Hunted Lammasu functions must return direct typed literals".into());
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                    || functions.contains(item.sig.ident.to_string().as_str())
                {
                    return Err("Hunted Lammasu function differs from the source contract".into());
                }
                functions.insert(item.sig.ident.to_string());
            }
            _ => return Err("Hunted Lammasu source contains an unsupported top-level item".into()),
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Hunted Lammasu source must expose only definition() and triggered_ability()".into(),
        );
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_hunted_lammasu::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Hunted Lammasu definition is missing from the RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(5)
        || definition.toughness != Some(5)
        || definition.keywords != vec![Keyword::Flying]
        || definition.is_basic_land
        || !definition.effects.is_empty()
    {
        return Err("Hunted Lammasu literal is outside the task source contract".into());
    }
    let binding = candidate_hunted_lammasu::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID)
        .ok_or("Hunted Lammasu opponent-targeted ETB binding is missing from the RAV registry")?;
    if binding != registered
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.optional
        || binding.ability.targets != vec![TargetRequirement::Opponent]
        || binding.ability.effects
            != vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::horror(),
                count: 1,
            }]
    {
        return Err("Hunted Lammasu trigger binding is outside the task source contract".into());
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
    NoFlying,
    RemoveTrigger,
    WrongTarget,
    WrongToken,
    WrongManaCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    lammasu: Option<ObjectId>,
    token: Option<ObjectId>,
    target: Option<PlayerId>,
    trigger_stacked: bool,
    trigger_resolved: bool,
    source_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    paid_four: bool,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
    token_controller: Option<PlayerId>,
    token_horror: bool,
    token_black: bool,
    token_creature: bool,
    token_power: Option<i32>,
    token_toughness: Option<i32>,
    token_keywords: Vec<String>,
    attacked: bool,
    combat_damage: i32,
}

fn run_match(mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Hunted Lammasu definition missing")?;
    *definition = candidate_hunted_lammasu::definition();
    if mutation == Mutation::NoFlying {
        definition.keywords.clear();
    }
    if mutation == Mutation::WrongManaCost {
        definition.mana_cost = ManaCost::with_colors(1, [Color::White, Color::White]);
    }
    let mut triggers = rav_triggered_ability_bindings();
    let index = triggers
        .iter()
        .position(|entry| entry.card_definition == CARD_ID && entry.ability.id == TRIGGER_ID);
    match mutation {
        Mutation::RemoveTrigger => {
            if let Some(index) = index {
                triggers.remove(index);
            }
        }
        Mutation::WrongTarget | Mutation::WrongToken => {
            let index = index.ok_or("Hunted Lammasu trigger missing for mutation")?;
            triggers[index] = candidate_hunted_lammasu::triggered_ability();
            if mutation == Mutation::WrongTarget {
                triggers[index].ability.targets = vec![TargetRequirement::Player];
                triggers[index].ability.effects = vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::horror(),
                    count: 1,
                }];
            }
            if mutation == Mutation::WrongToken {
                triggers[index].ability.effects = vec![Effect::CreateTokenForTargetOpponent {
                    token: TokenSpec::saproling(),
                    count: 1,
                }];
            }
        }
        Mutation::None | Mutation::NoFlying | Mutation::WrongManaCost => {}
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
                card: "RAV-PLAINS".into(),
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
    if deck0
        .mainboard
        .iter()
        .map(|e| usize::from(e.count))
        .sum::<usize>()
        != 60
        || deck1
            .mainboard
            .iter()
            .map(|e| usize::from(e.count))
            .sum::<usize>()
            != 60
    {
        return Err("Hunted Lammasu task decks must contain 60 cards".into());
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
        Box::new(LammasuPolicy::new(PlayerId(0), mutation)),
        Box::new(LammasuPolicy::new(PlayerId(1), mutation)),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        lammasu: None,
        token: None,
        target: None,
        trigger_stacked: false,
        trigger_resolved: false,
        source_incarnation: None,
        resolved_incarnation: None,
        paid_four: false,
        power: None,
        toughness: None,
        flying: false,
        token_controller: None,
        token_horror: false,
        token_black: false,
        token_creature: false,
        token_power: None,
        token_toughness: None,
        token_keywords: vec![],
        attacked: false,
        combat_damage: 0,
    };
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted >= MAX_MOVES {
            return Err(format!(
                "Hunted Lammasu game exceeded bounds at turn {} step {:?}, life {:?}",
                game.turn,
                game.step,
                game.players.iter().map(|p| p.life).collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(a) = policy.propose_pending_decision(&view) {
            a
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
                    "Hunted Lammasu policy rejected at turn {} {:?}: {error}",
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
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    captured.lammasu = Some(*card);
                    captured.paid_four =
                        colors.len() == 4 && colors.iter().all(|color| *color == Color::White);
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    captured.lammasu = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == captured.lammasu => {
                    let c = game.characteristics(*card).map_err(|e| e.to_string())?;
                    captured.power = c.power;
                    captured.toughness = c.toughness;
                    captured.flying = c.keywords.contains(&Keyword::Flying);
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                    ..
                } if Some(*source) == captured.lammasu => {
                    captured.trigger_stacked = true;
                    captured.source_incarnation = Some(*source_incarnation);
                    if let Some(stack) = game.stack.last() {
                        captured.target = match stack.targets.first() {
                            Some(Target::Player(player)) => Some(*player),
                            _ => None,
                        };
                    }
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                } if Some(*source) == captured.lammasu => {
                    captured.trigger_resolved = true;
                    captured.resolved_incarnation = Some(*source_incarnation);
                }
                GameEvent::TokenCreated { player, token } => {
                    captured.token = Some(*token);
                    captured.token_controller = Some(*player);
                    let object = game.object(*token).map_err(|e| e.to_string())?;
                    let c = game.characteristics(*token).map_err(|e| e.to_string())?;
                    captured.token_horror = c.creature_subtypes.contains(&CreatureSubtype::Horror);
                    captured.token_black = c.colors.contains(&Color::Black);
                    captured.token_creature = c.card_types.contains(&CardType::Creature);
                    captured.token_power = c.power;
                    captured.token_toughness = c.toughness;
                    captured.token_keywords = c
                        .keywords
                        .iter()
                        .map(|keyword| format!("{keyword:?}"))
                        .collect();
                    let _ = object;
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if captured.lammasu.is_some_and(|id| attackers.contains(&id)) => {
                    captured.attacked = true
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if Some(*source) == captured.lammasu => captured.combat_damage += *amount,
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|p| p.0);
    captured.turns = game.turn;
    captured.rejected = attempted.saturating_sub(captured.accepted);
    Ok(captured)
}

struct LammasuPolicy {
    player: PlayerId,
    mutation: Mutation,
    cast_once: bool,
}
impl LammasuPolicy {
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
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let colored_white = cost
            .colored
            .iter()
            .filter(|color| **color == Color::White)
            .count();
        let white = view.mana_pool.amount(Color::White);
        if white < colored_white as u8
            || view.mana_pool.total() < cost.generic as u8 + colored_white as u8
        {
            return None;
        }
        Some(ManaPaymentSelection {
            generic: vec![Color::White; cost.generic as usize],
            hybrid: vec![],
        })
    }
}
impl CodePolicy for LammasuPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.hunted-lammasu-controller.v1"
        } else {
            "rav.hunted-lammasu-opponent.v1"
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
                .find(|card| card.definition == Some("RAV-PLAINS"))
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
            let cost = if self.mutation == Mutation::WrongManaCost {
                ManaCost::with_colors(1, [Color::White, Color::White])
            } else {
                ManaCost::with_colors(2, [Color::White, Color::White])
            };
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
                .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            {
                if land.mana_colors.contains(&Color::White) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::White,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.target(view)
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Hunted Lammasu duel did not reach a clean terminal P0 win".into());
    }
    let source = c.lammasu.ok_or("Hunted Lammasu was not cast")?;
    let token = c.token.ok_or("opponent did not receive the Horror token")?;
    if !c.paid_four || c.power != Some(5) || c.toughness != Some(5) || !c.flying {
        return Err("Hunted Lammasu body or exact {2}{W}{W} payment differs".into());
    }
    if c.target != Some(PlayerId(1))
        || !c.trigger_stacked
        || !c.trigger_resolved
        || c.source_incarnation.is_none()
        || c.source_incarnation != c.resolved_incarnation
    {
        return Err(
            "Hunted Lammasu ETB did not target its opponent and resolve for the same incarnation"
                .into(),
        );
    }
    if c.token_controller != Some(PlayerId(1))
        || !c.token_horror
        || !c.token_black
        || !c.token_creature
        || c.token_power != Some(4)
        || c.token_toughness != Some(4)
        || !c.token_keywords.is_empty()
    {
        return Err("Hunted Lammasu did not create exactly the expected black 4/4 Horror".into());
    }
    if !c.attacked || c.combat_damage < 20 {
        return Err("Hunted Lammasu did not attack through a terminal combat win".into());
    }
    let paid = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 4 && colors.iter().filter(|color| **color == Color::White).count() == 4)).ok_or("exact {2}{W}{W} mana receipt is missing")?;
    let cast = event_index(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Hunted Lammasu spell cast event missing")?;
    let resolved = event_index(
        &c.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Hunted Lammasu spell resolution event missing")?;
    let stacked = event_index(&c.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source: card, ability: TRIGGER_ID, .. } if *card == source)).ok_or("Hunted Lammasu ETB trigger stack event missing")?;
    let token_event = event_index(&c.events, |event| matches!(event, GameEvent::TokenCreated { player: PlayerId(1), token: id } if *id == token)).ok_or("opponent Horror TokenCreated event missing")?;
    let ability = event_index(&c.events, |event| matches!(event, GameEvent::AbilityResolved { source: card, ability: TRIGGER_ID, .. } if *card == source)).ok_or("Hunted Lammasu ETB trigger resolution event missing")?;
    let attack = event_index(&c.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).ok_or("Hunted Lammasu attack event missing")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: card, player: PlayerId(1), amount } if *card == source && *amount > 0)).ok_or("Hunted Lammasu combat damage missing")?;
    let end = event_index(&c.events, |event| {
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
        && resolved < stacked
        && stacked < token_event
        && token_event < ability
        && ability < attack
        && attack < damage
        && damage < end)
    {
        return Err("Hunted Lammasu spell, ETB, token, combat event order differs".into());
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
    let source = c.lammasu;
    let token = c.token;
    let projected = project_events(&c.events);
    let events: Vec<_> = c.events.iter().enumerate().filter_map(|(i, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => source == Some(*card),
            GameEvent::TriggeredAbilityStacked { source: card, ability: TRIGGER_ID, .. } | GameEvent::AbilityResolved { source: card, ability: TRIGGER_ID, .. } => source == Some(*card),
            GameEvent::TokenCreated { token: id, .. } => token == Some(*id),
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => source.is_some_and(|id| attackers.contains(&id)),
            GameEvent::DamageDealtToPlayer { source: card, player: PlayerId(1), amount } => source == Some(*card) && *amount > 0,
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let event: &ProjectedEvent = projected.get(i)?;
        Some(json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical}))
    }).collect();
    json!({"id":"rav-hunted-lammasu-opponent-horror-etb","passed":true,"terminal":c.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"lammasu_object":source.map(|id|id.0),"etb":{"source_incarnation":c.source_incarnation,"resolved_incarnation":c.resolved_incarnation,"target_seat":c.target.map(|p|p.0),"token":{"object":token.map(|id|id.0),"controller_seat":c.token_controller.map(|p|p.0),"black":c.token_black,"creature":c.token_creature,"horror_subtype":c.token_horror,"power":c.token_power,"toughness":c.token_toughness,"keywords":c.token_keywords}},"lammasu":{"mana_cost":"{2}{W}{W}","paid_four_white":c.paid_four,"power":c.power,"toughness":c.toughness,"flying":c.flying,"attacked":c.attacked,"combat_damage":c.combat_damage},"events":events})
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
    let passed = card.collector_number == 22;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            paths.push(next);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
