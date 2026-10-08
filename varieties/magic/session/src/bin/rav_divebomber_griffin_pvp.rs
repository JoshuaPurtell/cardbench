//! Full-game diagnostic authority for RAV Divebomber Griffin.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DIVEBOMBER-GRIFFIN";
const CARD_NAME: &str = "Divebomber Griffin";
const COURIER_ID: &str = "RAV-COURIER-HAWK";
const ABILITY_ID: &str = "sacrifice-deal-three-to-attacker-or-blocker";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/divebomber_griffin.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/divebomber_griffin.rs"]
mod candidate_divebomber_griffin;

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
            eprintln!("Divebomber Griffin authority failed: {error}");
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
        return Err("Divebomber Griffin full-game replay was not deterministic".into());
    }
    let mutations = json!({
        "flying_removed_detected_in_game": mutation_detected(Mutation::NoFlying),
        "sacrifice_activation_removed_detected_in_game": mutation_detected(Mutation::RemoveAbility),
        "attacking_or_blocking_target_restriction_removed_detected_in_game": mutation_detected(Mutation::WrongTarget),
        "damage_amount_changed_detected_in_game": mutation_detected(Mutation::WrongDamage),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_removed_detected_in_game",
        "sacrifice_activation_removed_detected_in_game",
        "attacking_or_blocking_target_restriction_removed_detected_in_game",
        "damage_amount_changed_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Divebomber Griffin gameplay mutation escaped the trace oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":14,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["110 Permanents","117 Timing and priority","202.1-202.2 Color and mana cost","302.1 Creature characteristics","601.2 Casting spells","602.2 Activating abilities","602.2b Sacrifice costs","608.2 Resolving abilities","damage and state-based actions","509 Attacking creatures"],"source_contract":"CardBench-authored semantic summary: Divebomber Griffin is a white 3WW 3/2 creature with flying; sacrifice it to deal 3 damage to target attacking or blocking creature.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,"mutation_sensitivity":mutations,"traces":[trace_json(&reference)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Divebomber Griffin source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Divebomber Griffin source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Divebomber Griffin source has unsupported module attributes".into());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
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
                    return Err("Divebomber Griffin imports differ from the source contract".into());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item
                    .attrs
                    .iter()
                    .all(|a| a.path().is_ident("must_use") || a.path().is_ident("doc"))
                    && matches!(&item.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none() =>
            {
                let output = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => return Err("Divebomber Griffin exposes an unsupported function".into()),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err(
                        "Divebomber Griffin functions must return direct typed literals".into(),
                    );
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                    || functions.contains(item.sig.ident.to_string().as_str())
                {
                    return Err(
                        "Divebomber Griffin function differs from the source contract".into(),
                    );
                }
                functions.insert(item.sig.ident.to_string());
            }
            _ => {
                return Err(
                    "Divebomber Griffin source contains an unsupported top-level item".into(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Divebomber Griffin source module differs from the task contract".into());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_divebomber_griffin::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|d| d.id == CARD_ID)
        .ok_or("Divebomber Griffin definition missing from RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(3)
        || definition.toughness != Some(2)
        || definition.keywords != vec![Keyword::Flying]
        || definition.is_basic_land
        || !definition.effects.is_empty()
    {
        return Err("Divebomber Griffin literal is outside the task source contract".into());
    }
    let ability = candidate_divebomber_griffin::activated_ability();
    let registered = rav_activated_ability_bindings()
        .into_iter()
        .find(|b| b.card_definition == CARD_ID)
        .ok_or("Divebomber Griffin sacrifice ability missing from RAV registry")?;
    if ability != registered
        || ability.ability.id != ABILITY_ID
        || ability.ability.mana_cost != ManaCost::new(0)
        || ability.ability.tap_cost
        || ability.ability.sorcery_speed
        || !ability.ability.sacrifice_source
        || ability.ability.sacrifice_creatures != 0
        || ability.ability.sacrifice_lands != 0
        || ability.ability.discard_cards != 0
        || ability.ability.targets != vec![TargetRequirement::AttackingOrBlockingCreature]
        || ability.ability.effects
            != vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::AttackingOrBlockingCreature,
            }]
    {
        return Err("Divebomber Griffin activation is outside the task source contract".into());
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
    RemoveAbility,
    WrongTarget,
    WrongDamage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    griffin: Option<ObjectId>,
    paid: Vec<Color>,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
    attacked_hawk: Option<ObjectId>,
    activation_source: Option<ObjectId>,
    activation_target: Option<ObjectId>,
    activation_resolved: bool,
    source_sacrificed: bool,
    damage: Option<i32>,
    target_died: bool,
    attacker_objects: Vec<ObjectId>,
    combat_damage: i32,
}

fn run_match(mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Divebomber Griffin definition missing")?;
    *definition = candidate_divebomber_griffin::definition();
    if mutation == Mutation::NoFlying {
        definition.keywords.clear();
    }
    let mut abilities = rav_activated_ability_bindings();
    let index = abilities
        .iter()
        .position(|b| b.card_definition == CARD_ID)
        .ok_or("Divebomber Griffin ability missing")?;
    abilities[index] = candidate_divebomber_griffin::activated_ability();
    match mutation {
        Mutation::RemoveAbility => {
            abilities.remove(index);
        }
        Mutation::WrongTarget => {
            abilities[index].ability.targets = vec![TargetRequirement::Creature];
            abilities[index].ability.effects = vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::Creature,
            }];
        }
        Mutation::WrongDamage => {
            abilities[index].ability.effects = vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::AttackingOrBlockingCreature,
            }];
        }
        Mutation::None | Mutation::NoFlying => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: COURIER_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: COURIER_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for deck in [&deck0, &deck1] {
        if deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Divebomber Griffin task deck is not 60 cards".into());
        }
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
        Box::new(GriffinPolicy::new(PlayerId(0), mutation)),
        Box::new(GriffinPolicy::new(PlayerId(1), mutation)),
    ];
    let mut result = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        griffin: None,
        paid: vec![],
        power: None,
        toughness: None,
        flying: false,
        attacked_hawk: None,
        activation_source: None,
        activation_target: None,
        activation_resolved: false,
        source_sacrificed: false,
        damage: None,
        target_died: false,
        attacker_objects: vec![],
        combat_damage: 0,
    };
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || result.accepted >= MAX_MOVES {
            return Err(format!(
                "Divebomber Griffin game exceeded bounds at turn {} / life {:?}",
                game.turn,
                game.players.iter().map(|p| p.life).collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
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
            .map_err(|e| format!("Divebomber policy move rejected: {e}"))?;
        result.accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        for event in &game.event_log[previous..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    result.griffin = Some(*card);
                    result.paid = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    result.griffin = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == result.griffin => {
                    let c = game.characteristics(*card).map_err(|e| e.to_string())?;
                    result.power = c.power;
                    result.toughness = c.toughness;
                    result.flying = c.keywords.contains(&Keyword::Flying);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(1),
                    attackers,
                } => {
                    if result.attacked_hawk.is_none() {
                        result.attacked_hawk = attackers
                            .iter()
                            .copied()
                            .find(|id| game.card_definition(*id).is_ok_and(|d| d.id == COURIER_ID));
                    }
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } => {
                    for id in attackers {
                        if !result.attacker_objects.contains(id) {
                            result.attacker_objects.push(*id);
                        }
                    }
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    ability,
                    ..
                } if *ability == ABILITY_ID => {
                    result.activation_source = Some(*source);
                    result.activation_target = game
                        .stack
                        .last()
                        .and_then(|item| item.targets.first())
                        .and_then(|target| match target {
                            Target::Permanent(id) => Some(*id),
                            _ => None,
                        });
                    result.source_sacrificed = game.zone_of(*source) == Some(Zone::Graveyard);
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if Some(*source) == result.activation_source && *ability == ABILITY_ID => {
                    result.activation_resolved = true
                }
                GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                } if Some(*source) == result.activation_source
                    && Some(*permanent) == result.activation_target =>
                {
                    result.damage = Some(*amount)
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                    ..
                } if Some(*card) == result.attacked_hawk => result.target_died = true,
                GameEvent::DamageDealtToPlayer {
                    player: PlayerId(1),
                    source,
                    amount,
                } if result.attacker_objects.contains(source) => result.combat_damage += *amount,
                _ => {}
            }
        }
    }
    result.events = game.event_log.clone();
    result.winner = game.winner().map(|p| p.0);
    result.turns = game.turn;
    result.rejected = attempted.saturating_sub(result.accepted);
    Ok(result)
}

struct GriffinPolicy {
    player: PlayerId,
    mutation: Mutation,
    griffin_cast: bool,
    activated: bool,
}
impl GriffinPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            griffin_cast: false,
            activated: false,
        }
    }
    fn cast(view: &GameView, id: &'static str, cost: ManaCost) -> Option<PolicyAction> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, count) = available.iter_mut().find(|(known, _)| known == color)?;
            if *count == 0 {
                return None;
            }
            *count -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, count) = available.iter_mut().max_by_key(|(_, count)| *count)?;
            if *count == 0 {
                return None;
            }
            *count -= 1;
            generic.push(*color);
        }
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: cardbench_magic_engine::ManaPaymentSelection {
                generic,
                hybrid: vec![],
            },
        })
    }
}
impl CodePolicy for GriffinPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.divebomber-griffin-controller.v1"
        } else {
            "rav.divebomber-griffin-opponent.v1"
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
                    .filter(|c| c.can_attack && c.card_types.contains(&CardType::Creature))
                    .map(|c| c.id)
                    .collect()
            } else if view
                .opponent_battlefield
                .iter()
                .any(|c| c.definition == Some(CARD_ID))
            {
                view.own_battlefield
                    .iter()
                    .find(|c| c.can_attack && c.definition == Some(COURIER_ID))
                    .map_or_else(Vec::new, |c| vec![c.id])
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
        if self.player == PlayerId(0) && !self.activated && self.mutation != Mutation::RemoveAbility
        {
            let griffin = view
                .own_battlefield
                .iter()
                .find(|c| c.definition == Some(CARD_ID));
            let attacker = view
                .combat_attackers
                .iter()
                .find(|c| c.controller == PlayerId(1) && c.definition == Some(COURIER_ID));
            if let (Some(griffin), Some(attacker)) = (griffin, attacker) {
                let target = if self.mutation == Mutation::WrongTarget {
                    view.opponent_battlefield
                        .iter()
                        .find(|c| c.definition == Some(COURIER_ID) && c.id != attacker.id)
                } else {
                    Some(attacker)
                };
                if let Some(target) = target {
                    self.activated = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: griffin.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![griffin.id],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(target.id)],
                        },
                    };
                }
            }
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
            && !self.griffin_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(action) = Self::cast(
                view,
                CARD_ID,
                ManaCost::with_colors(3, [Color::White, Color::White]),
            ) {
                self.griffin_cast = true;
                return action;
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&Color::White)
            }) {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        if view
            .hand
            .iter()
            .any(|card| card.definition == Some(COURIER_ID))
        {
            if let Some(action) =
                Self::cast(view, COURIER_ID, ManaCost::with_colors(1, [Color::White]))
            {
                return action;
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&Color::White)
            }) {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Divebomber Griffin duel did not reach a clean terminal P0 win".into());
    }
    let griffin = c.griffin.ok_or("Divebomber Griffin was not cast")?;
    let attack = c
        .attacked_hawk
        .ok_or("opponent did not attack with a Courier Hawk")?;
    let source = c
        .activation_source
        .ok_or("Divebomber Griffin sacrifice ability was not activated")?;
    let target = c
        .activation_target
        .ok_or("Divebomber Griffin activation did not target a permanent")?;
    if c.paid.len() != 5
        || c.paid.iter().any(|color| *color != Color::White)
        || c.power != Some(3)
        || c.toughness != Some(2)
        || !c.flying
    {
        return Err("Divebomber Griffin exact {3}{W}{W} payment/body differs".into());
    }
    if source != griffin
        || target != attack
        || !c.source_sacrificed
        || !c.activation_resolved
        || c.damage != Some(3)
        || !c.target_died
    {
        return Err("Divebomber Griffin did not sacrifice itself to deal exactly 3 to the attacking creature".into());
    }
    if c.combat_damage < 20 {
        return Err(
            "Divebomber Griffin duel did not continue through a terminal combat win".into(),
        );
    }
    let paid = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == griffin && colors.len() == 5 && colors.iter().all(|c| *c == Color::White))).ok_or("exact Divebomber Griffin mana receipt missing")?;
    let cast = event_index(
        &c.events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == griffin),
    )
    .ok_or("Divebomber Griffin cast receipt missing")?;
    let resolved = event_index(
        &c.events,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == griffin),
    )
    .ok_or("Divebomber Griffin spell resolution missing")?;
    let attack_index = event_index(&c.events, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&attack))).ok_or("opponent Courier Hawk attack declaration missing")?;
    let sacrificed = event_index(
        &c.events,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == griffin),
    )
    .ok_or("Divebomber Griffin sacrifice cost movement missing")?;
    let activation = event_index(&c.events, |e| matches!(e, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == griffin)).ok_or("Divebomber Griffin activation event missing")?;
    let ability_resolved = event_index(&c.events, |e| matches!(e, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == griffin)).ok_or("Divebomber Griffin ability did not resolve")?;
    let damage = event_index(&c.events, |e| matches!(e, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == griffin && *permanent == attack)).ok_or("exact 3 damage to the attacking Hawk missing")?;
    let target_death = event_index_after(
        &c.events,
        damage,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == attack),
    )
    .ok_or("lethal damage did not move the Courier Hawk to its graveyard")?;
    let end = event_index(&c.events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win event missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < attack_index
        && attack_index < sacrificed
        && sacrificed < activation
        && activation < damage
        && damage < ability_resolved
        && ability_resolved < target_death
        && target_death < end)
    {
        return Err(format!(
            "Divebomber Griffin event order differs: paid={paid} cast={cast} resolved={resolved} attack={attack_index} sacrificed={sacrificed} activated={activation} ability_resolved={ability_resolved} damage={damage} target_death={target_death} end={end}"
        ));
    }
    Ok(())
}
fn mutation_detected(m: Mutation) -> bool {
    run_match(m).and_then(|c| validate_trace(&c)).is_err()
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
        .find_map(|(i, e)| predicate(e).then_some(i))
}
fn trace_json(c: &Captured) -> Value {
    let griffin = c.griffin;
    let target = c.activation_target;
    let attackers = &c.attacker_objects;
    let projected = project_events(&c.events);
    let events: Vec<_> = c.events.iter().enumerate().filter_map(|(i,event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card,.. } | GameEvent::SpellCast { card,.. } | GameEvent::SpellResolved { card } => griffin == Some(*card),
            GameEvent::AttackersDeclared { player: PlayerId(1), attackers: ids } => c.attacked_hawk.is_some_and(|id| ids.contains(&id)),
            GameEvent::CardMoved { card, to: Zone::Graveyard,.. } => griffin == Some(*card) || c.attacked_hawk == Some(*card),
            GameEvent::AbilityActivated { source, ability: ABILITY_ID,.. } | GameEvent::AbilityResolved { source, ability: ABILITY_ID,.. } => griffin == Some(*source),
            GameEvent::DamageDealtToPermanent { source, permanent,.. } => griffin == Some(*source) && target == Some(*permanent),
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers: ids } => ids.iter().any(|id| attackers.contains(id)),
            GameEvent::DamageDealtToPlayer { player: PlayerId(1), source,.. } => attackers.contains(source),
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true, _ => false,
        }; if !include { return None; } let event: &ProjectedEvent = projected.get(i)?; Some(json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical}))
    }).collect();
    json!({"id":"rav-divebomber-griffin-sacrifice-damage-attacker","passed":true,"terminal":c.winner.map_or_else(||"Draw".into(),|seat|format!("Winner(PlayerId({seat}))")),"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"griffin_object":griffin.map(|id|id.0),"mana_colors":c.paid.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"griffin":{"mana_cost":"{3}{W}{W}","power":c.power,"toughness":c.toughness,"flying":c.flying,"sacrificed":c.source_sacrificed},"combat":{"opponent_attacker_object":c.attacked_hawk.map(|id|id.0),"activation_source_object":c.activation_source.map(|id|id.0),"activation_target_object":target.map(|id|id.0),"ability_resolved":c.activation_resolved,"damage":c.damage,"target_died":c.target_died,"p0_combat_damage":c.combat_damage},"events":events})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|a| a == "--candidate") {
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
    let passed = card.collector_number == 14;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(p) => {
            let mut next = prefix.to_vec();
            next.push(p.ident.to_string());
            flatten_use_tree(&next, &p.tree, paths)
        }
        UseTree::Name(n) => {
            let mut next = prefix.to_vec();
            next.push(n.ident.to_string());
            paths.push(next);
            true
        }
        UseTree::Group(g) => g
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output,syn::ReturnType::Type(_,ty)if matches!(&**ty,syn::Type::Path(path)if path.qself.is_none()&&path_is_type(&path.path,expected)))
}
fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
