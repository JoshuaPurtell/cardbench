//! Tests for the second engine pass: generic rule fixes, no card names.
use crate::ids::{CardDefId, CardInstanceId, PlayerId};
use crate::markers::Marker;
use crate::modifiers::{StatModifierEntry, StatModifierKind, StatModifierValue};
use crate::runtime_hooks::{AttackOverrides, RuntimeHooks};
use crate::types::{Resistance, Stage, Type, Weakness};
use crate::zone::CardInstance;
use crate::{
    execute,     execute_effect_with_source, Action, Attack, AttackCost, CardMeta, CardSelector,
    EffectAst, GameEvent, GameState, PokemonSelector, Prompt, SelectionDestination, Target,
    TargetPlayer, TriggerKind, TriggerPredicate, TriggerSubscription,
};
use tcg_rules_ex::{Phase, RulesetConfig};

fn pokemon(name: &str, types: Vec<Type>, weakness: Option<Type>, resistance: Option<Type>, stage: Stage) -> CardMeta {
    CardMeta {
        name: name.to_string(),
        is_basic: stage == Stage::Basic,
        is_tool: false,
        is_stadium: false,
        is_pokemon: true,
        is_energy: false,
        hp: 100,
        energy_kind: None,
        provides: Vec::new(),
        trainer_kind: None,
        is_ex: false,
        is_star: false,
        is_delta: false,
        stage,
        types,
        weakness: weakness.map(|type_| Weakness { type_, multiplier: 2 }),
        resistance: resistance.map(|type_| Resistance { type_, value: 30 }),
        retreat_cost: Some(1),
        trainer_effect: None,
        evolves_from: None,
        attacks: Vec::new(),
        card_type: String::new(),
        delta_species: false,
    }
}

fn energy(name: &str, provides: Vec<Type>) -> CardMeta {
    CardMeta {
        is_pokemon: false,
        is_energy: true,
        is_basic: false,
        hp: 0,
        energy_kind: Some("Basic".to_string()),
        provides,
        types: Vec::new(),
        weakness: None,
        resistance: None,
        retreat_cost: None,
        ..pokemon(name, Vec::new(), None, None, Stage::Basic)
    }
}

fn tool(name: &str, effect: serde_json::Value) -> CardMeta {
    CardMeta {
        is_pokemon: false,
        is_tool: true,
        is_basic: false,
        hp: 0,
        trainer_kind: Some("Tool".to_string()),
        trainer_effect: Some(effect),
        types: Vec::new(),
        weakness: None,
        resistance: None,
        retreat_cost: None,
        ..pokemon(name, Vec::new(), None, None, Stage::Basic)
    }
}

fn new_game() -> GameState {
    let deck = |owner: PlayerId| {
        (0..30)
            .map(|_| CardInstance::new(CardDefId::new("FILL"), owner))
            .collect::<Vec<_>>()
    };
    let mut game = GameState::new(deck(PlayerId::P1), deck(PlayerId::P2), 7, RulesetConfig::default());
    game.card_meta.insert(CardDefId::new("FILL"), pokemon("Fill", vec![Type::Colorless], None, None, Stage::Basic));
    game.turn.phase = Phase::Main;
    game.turn.player = PlayerId::P1;
    game.turn.number = 2;
    game
}

fn index(player: PlayerId) -> usize {
    if player == PlayerId::P1 { 0 } else { 1 }
}

fn put(game: &mut GameState, player: PlayerId, def: &str, active: bool) -> CardInstanceId {
    let slot = game.slot_from_card(CardInstance::new(CardDefId::new(def), player));
    let id = slot.card.id;
    if active {
        game.players[index(player)].active = Some(slot);
    } else {
        game.players[index(player)].bench.push(slot);
    }
    id
}

fn attack(name: &str, damage: u16, type_: Type, effect: Option<EffectAst>) -> Attack {
    Attack {
        name: name.to_string(),
        damage,
        attack_type: type_,
        cost: AttackCost { total_energy: 0, types: Vec::new() },
        effect_ast: effect,
    }
}

fn give_attack(game: &mut GameState, id: CardInstanceId, attack: &Attack) {
    game.find_pokemon_slot_mut(id).unwrap().attacks = vec![attack.clone()];
}

fn counters(game: &GameState, id: CardInstanceId) -> u16 {
    game.slot_by_id(id).map(|slot| slot.damage_counters).unwrap_or(0)
}

fn declare(game: &mut GameState, attack: Attack) {
    game.apply_action(PlayerId::P1, Action::DeclareAttack { attack }).unwrap();
}

// 1. Preventing damage keeps the attack's other effects; effect prevention is per recipient.
#[test]
fn prevent_damage_keeps_other_effects_and_is_per_target() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("ATK"), pokemon("Atk", vec![Type::Fire], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("DEF"), pokemon("Def", vec![Type::Water], None, None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "ATK", true);
    let defender = put(&mut game, PlayerId::P2, "DEF", true);
    let mut hooks = RuntimeHooks::empty();
    hooks.attack_overrides = |_, _, _, _| AttackOverrides { prevent_damage: true, ..AttackOverrides::default() };
    hooks.prevents_attack_effects = |game, _, target| game.players[1].active.as_ref().map(|s| s.card.id) == Some(target);
    hooks.post_attack = |game, attacker, _, _| {
        let _ = game.add_marker(attacker, Marker::new("FromHook"));
    };
    game.set_hooks(hooks);
    let effect = EffectAst::Sequence {
        effects: vec![
            EffectAst::ApplySpecialCondition { target: Target::OppActive, condition: tcg_rules_ex::SpecialCondition::Asleep },
            EffectAst::AddMarker { target: Target::SelfActive, name: "Recoiled".into(), expires_after_turns: None },
        ],
    };
    let a = attack("Blast", 30, Type::Fire, Some(effect));
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    assert_eq!(counters(&game, defender), 0, "damage is prevented");
    assert!(game.slot_by_id(defender).unwrap().special_conditions.is_empty(), "effect on the protected Pokémon is prevented");
    assert!(game.has_marker(attacker, "Recoiled"), "the attack's effect on its own attacker still happens");
    assert!(game.has_marker(attacker, "FromHook"), "post-attack hooks still run when damage is prevented");
}

// 2. Placing damage counters is not damage: Bench-damage prevention does not stop it.
#[test]
fn placing_counters_ignores_bench_damage_prevention() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Grass], None, None, Stage::Basic));
    put(&mut game, PlayerId::P1, "MON", true);
    put(&mut game, PlayerId::P2, "MON", true);
    let benched = put(&mut game, PlayerId::P2, "MON", false);
    let mut entry = StatModifierEntry::new_amount(StatModifierKind::PreventBenchDamage, 0);
    entry.value = StatModifierValue::Bool(true);
    game.add_stat_modifier(entry);
    execute_effect_with_source(&mut game, &EffectAst::PlaceDamageCounters { target: Target::OppBench, counters: 2 }, None).unwrap();
    assert_eq!(counters(&game, benched), 2);
    execute_effect_with_source(&mut game, &EffectAst::DealDamage { target: Target::OppBench, amount: 20 }, None).unwrap();
    assert_eq!(counters(&game, benched), 2, "real damage to the Bench is still prevented");
}

// 3. A dual-type attacker hits Weakness to either of its types.
#[test]
fn dual_type_attacker_applies_weakness_for_either_type() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("DUAL"), pokemon("Dual", vec![Type::Fire, Type::Metal], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("METALWEAK"), pokemon("Weak", vec![Type::Grass], Some(Type::Metal), None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "DUAL", true);
    let defender = put(&mut game, PlayerId::P2, "METALWEAK", true);
    let a = attack("Slam", 20, Type::Fire, None);
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    assert_eq!(counters(&game, defender), 4, "Fire/Metal attacker doubles against Metal Weakness");
}

// 4. A "no Weakness" override also covers damage an attack's effect does to the Active.
#[test]
fn weakness_removal_covers_effect_damage() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("ATK"), pokemon("Atk", vec![Type::Fire], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("FIREWEAK"), pokemon("Weak", vec![Type::Grass], Some(Type::Fire), None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "ATK", true);
    let defender = put(&mut game, PlayerId::P2, "FIREWEAK", true);
    let mut hooks = RuntimeHooks::empty();
    hooks.attack_overrides = |_, _, _, _| AttackOverrides { ignore_weakness: true, ..AttackOverrides::default() };
    game.set_hooks(hooks);
    let a = attack("Sear", 0, Type::Fire, Some(EffectAst::DealDamage { target: Target::OppActive, amount: 20 }));
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    assert_eq!(counters(&game, defender), 2, "effect damage must not apply the removed Weakness");
}

// 5. A custom prompt opened while resolving another keeps its own id.
#[test]
fn chained_custom_prompt_keeps_its_id() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Grass], None, None, Stage::Basic));
    let own = put(&mut game, PlayerId::P1, "MON", true);
    put(&mut game, PlayerId::P2, "MON", true);
    let mut hooks = RuntimeHooks::empty();
    hooks.resolve_custom_prompt = |game, id, _, targets| {
        let own = game.players[0].active.as_ref().unwrap().card.id;
        if id == "T:first" {
            game.set_pending_prompt_custom(
                Prompt::ChoosePokemonTargets { player: PlayerId::P1, min: 1, max: 1, valid_targets: targets.to_vec(), effect_description: String::new() },
                PlayerId::P1,
                "T:second".into(),
                None,
            );
            true
        } else if id == "T:second" {
            let _ = game.add_marker(own, Marker::new("Second"));
            true
        } else {
            false
        }
    };
    game.set_hooks(hooks);
    game.set_pending_prompt_custom(
        Prompt::ChoosePokemonTargets { player: PlayerId::P1, min: 1, max: 1, valid_targets: vec![own], effect_description: String::new() },
        PlayerId::P1,
        "T:first".into(),
        None,
    );
    game.apply_action(PlayerId::P1, Action::ChoosePokemonTargets { target_ids: vec![own] }).unwrap();
    game.apply_action(PlayerId::P1, Action::ChoosePokemonTargets { target_ids: vec![own] }).unwrap();
    assert!(game.has_marker(own, "Second"), "the follow-up custom prompt must reach its hook");
}

// 6. Typed costs are matched, not paid greedily.
#[test]
fn typed_cost_uses_matching() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Fire], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("E-FW"), energy("Fire Water", vec![Type::Fire, Type::Water]));
    game.card_meta.insert(CardDefId::new("E-F"), energy("Fire", vec![Type::Fire]));
    let attacker = put(&mut game, PlayerId::P1, "MON", true);
    let slot = game.find_pokemon_slot_mut(attacker).unwrap();
    slot.attached_energy.push(CardInstance::new(CardDefId::new("E-FW"), PlayerId::P1));
    slot.attached_energy.push(CardInstance::new(CardDefId::new("E-F"), PlayerId::P1));
    let mut a = attack("Steam", 0, Type::Fire, None);
    a.cost = AttackCost { total_energy: 2, types: vec![Type::Fire, Type::Water] };
    assert!(game.attack_cost_met(attacker, &a), "Fire pays Fire, the Fire/Water Energy pays Water");
}

// 7a. A coin flip is logged once, after AttackDeclared; effect damage and draws reach the event log.
#[test]
fn effect_events_are_logged_once() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Grass], None, None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "MON", true);
    let defender = put(&mut game, PlayerId::P2, "MON", true);
    let effect = EffectAst::Sequence {
        effects: vec![
            EffectAst::FlipCoins { count: 1, on_heads: Box::new(EffectAst::NoOp), on_tails: Box::new(EffectAst::NoOp) },
            EffectAst::DealDamage { target: Target::OppActive, amount: 10 },
            EffectAst::DrawCards { player: TargetPlayer::Current, count: 1 },
        ],
    };
    let a = attack("Mix", 0, Type::Grass, Some(effect));
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    let coins = game.event_log.iter().filter(|e| matches!(e, GameEvent::CoinFlipped { .. })).count();
    assert_eq!(coins, 1, "one flip, one CoinFlipped");
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::DamageDealt { pokemon_id, amount: 10 } if *pokemon_id == defender)));
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::CardDrawn { player: PlayerId::P1, .. })));
    let declared = game.event_log.iter().position(|e| matches!(e, GameEvent::AttackDeclared { .. }));
    let flipped = game.event_log.iter().position(|e| matches!(e, GameEvent::CoinFlipped { .. }));
    assert!(declared.is_some() && flipped.is_some() && declared.unwrap() < flipped.unwrap(),
        "CoinFlipped must follow AttackDeclared so attack windows see the flip");
}

// 7b. Tools and Stadiums discarded by an in-play choice, or with a Knocked Out Pokémon, are logged.
#[test]
fn tool_and_stadium_discards_are_logged() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Grass], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("TOOL"), tool("Tool", serde_json::json!({})));
    let own = put(&mut game, PlayerId::P1, "MON", true);
    let opp = put(&mut game, PlayerId::P2, "MON", true);
    let tool_card = CardInstance::new(CardDefId::new("TOOL"), PlayerId::P2);
    let tool_id = tool_card.id;
    game.find_pokemon_slot_mut(opp).unwrap().attached_tool = Some(tool_card);
    let stadium = CardInstance::new(CardDefId::new("FILL"), PlayerId::P2);
    let stadium_id = stadium.id;
    game.stadium_in_play = Some(stadium);
    game.set_pending_prompt(Prompt::ChooseCardsInPlay { player: PlayerId::P1, options: vec![tool_id, stadium_id], min: 0, max: 2 }, PlayerId::P1);
    game.apply_action(PlayerId::P1, Action::ChooseCardsInPlay { card_ids: vec![tool_id, stadium_id] }).unwrap();
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::ToolDiscarded { tool_id: id, .. } if *id == tool_id)));
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::StadiumDiscarded { stadium_id: id, .. } if *id == stadium_id)));
    // Knock Out with a Tool attached.
    let ko_tool = CardInstance::new(CardDefId::new("TOOL"), PlayerId::P1);
    let ko_tool_id = ko_tool.id;
    game.find_pokemon_slot_mut(own).unwrap().attached_tool = Some(ko_tool);
    game.find_pokemon_slot_mut(own).unwrap().damage_counters = 20;
    let outcomes = crate::check_knockouts_all(&mut game);
    assert_eq!(outcomes.len(), 1);
    assert!(game.pending_broadcast_events.iter().any(|e| matches!(e, GameEvent::ToolDiscarded { tool_id: id, .. } if *id == ko_tool_id)));
}

// 8. An EndOfTurnIfAttacked Tool stays attached for the rest of the turn, then is discarded with ToolDiscarded.
#[test]
fn end_of_turn_tool_discards_at_end_of_turn() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Grass], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("BERRY"), tool("Berry", serde_json::json!({"discard": "EndOfTurnIfAttacked"})));
    let attacker = put(&mut game, PlayerId::P1, "MON", true);
    put(&mut game, PlayerId::P2, "MON", true);
    let berry = CardInstance::new(CardDefId::new("BERRY"), PlayerId::P1);
    let berry_id = berry.id;
    game.find_pokemon_slot_mut(attacker).unwrap().attached_tool = Some(berry);
    let a = attack("Tap", 10, Type::Grass, None);
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    assert!(game.slot_by_id(attacker).unwrap().attached_tool.is_some(), "still attached right after the attack");
    for _ in 0..20 {
        if game.event_log.iter().any(|e| matches!(e, GameEvent::ToolDiscarded { tool_id, .. } if *tool_id == berry_id)) {
            break;
        }
        match game.step() {
            crate::StepResult::Prompt { .. } | crate::StepResult::GameOver { .. } => break,
            _ => {}
        }
    }
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::ToolDiscarded { tool_id, .. } if *tool_id == berry_id)));
    assert!(game.slot_by_id(attacker).unwrap().attached_tool.is_none());
}

fn energy_deck(game: &mut GameState, n: usize) {
    game.card_meta.insert(CardDefId::new("E-W"), energy("Water", vec![Type::Water]));
    for _ in 0..n {
        game.players[0].deck.add(CardInstance::new(CardDefId::new("E-W"), PlayerId::P1));
    }
}

fn energy_search() -> EffectAst {
    EffectAst::SearchDeckWithSelector {
        player: TargetPlayer::Current,
        selector: CardSelector { is_energy: Some(true), ..CardSelector::default() },
        count: 1,
        min: Some(0),
        max: Some(1),
        destination: SelectionDestination::Hand,
        shuffle: false,
        reveal: false,
    }
}

fn take_first_offer(game: &mut GameState) {
    let options = match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChooseCardsFromDeck { options, .. }) => options.clone(),
        other => panic!("expected a deck prompt, got {other:?}"),
    };
    execute(game, Action::TakeCardsFromDeck { card_ids: vec![options[0]] }).unwrap();
}

// 9. A Sequence whose last step prompts does not re-arm that prompt after it resolves.
#[test]
fn sequence_last_prompt_is_not_rearmed() {
    let mut game = new_game();
    energy_deck(&mut game, 5);
    put(&mut game, PlayerId::P1, "FILL", true);
    let sequence = EffectAst::Sequence { effects: vec![energy_search(), energy_search()] };
    let _ = execute_effect_with_source(&mut game, &sequence, None).unwrap();
    take_first_offer(&mut game);
    take_first_offer(&mut game);
    assert!(game.pending_prompt.is_none(), "the second search must not prompt again");
}

// 9b. A queued Custom that opens a follow-up prompt must not be re-stored (Peal of Thunder).
#[test]
fn queued_custom_followup_prompt_is_not_rearmed() {
    let mut game = new_game();
    energy_deck(&mut game, 3);
    let own = put(&mut game, PlayerId::P1, "FILL", true);
    let mut hooks = RuntimeHooks::empty();
    hooks.execute_power = |game, name, source| {
        if name != "Pick" {
            return false;
        }
        game.set_pending_prompt_custom(
            Prompt::ChoosePokemonTargets {
                player: PlayerId::P1,
                min: 1,
                max: 1,
                valid_targets: vec![source],
                effect_description: String::new(),
            },
            PlayerId::P1,
            "T:attach".into(),
            Some(source),
        );
        true
    };
    game.set_hooks(hooks);
    let options: Vec<_> = game.players[0].deck.cards().iter().map(|card| card.id).take(1).collect();
    game.set_pending_effect_prompt(
        Prompt::ChooseCardsFromDeck {
            player: PlayerId::P1,
            count: 1,
            options: options.clone(),
            revealed_cards: Vec::new(),
            min: Some(0),
            max: Some(1),
            destination: SelectionDestination::Hand,
            shuffle: false,
        },
        PlayerId::P1,
        EffectAst::Custom {
            id: "T:pick".into(),
            name: Some("Pick".into()),
            data: Default::default(),
        },
        Some(own),
    );
    execute(&mut game, Action::TakeCardsFromDeck { card_ids: vec![options[0]] }).unwrap();
    assert!(
        matches!(
            game.pending_prompt.as_ref().map(|pending| &pending.prompt),
            Some(Prompt::ChoosePokemonTargets { .. })
        ),
        "the follow-up attach prompt must stay armed, got {:?}",
        game.pending_prompt.as_ref().map(|pending| &pending.prompt)
    );
    assert!(game.pending_effect_ast.is_none(), "the queued Custom must not be re-stored");
}

// 10. A search that finds nothing still shuffles when the effect says so.
#[test]
fn empty_search_still_shuffles() {
    let mut game = new_game();
    let before = game.players[0].deck.order();
    let effect = EffectAst::SearchDeckWithSelector {
        player: TargetPlayer::Current,
        selector: CardSelector { is_energy: Some(true), ..CardSelector::default() },
        count: 1,
        min: Some(0),
        max: Some(1),
        destination: SelectionDestination::Hand,
        shuffle: true,
        reveal: false,
    };
    execute_effect_with_source(&mut game, &effect, None).unwrap();
    assert!(game.pending_prompt.is_none());
    assert_ne!(game.players[0].deck.order(), before, "deck must be shuffled");
}

// 11. The retreat payment prompt lists the attached Energy.
#[test]
fn retreat_prompt_lists_attached_energy() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("E-W"), energy("Water", vec![Type::Water]));
    let active = put(&mut game, PlayerId::P1, "FILL", true);
    let bench = put(&mut game, PlayerId::P1, "FILL", false);
    let e = CardInstance::new(CardDefId::new("E-W"), PlayerId::P1);
    let energy_id = e.id;
    game.find_pokemon_slot_mut(active).unwrap().attached_energy.push(e);
    execute(&mut game, Action::Retreat { to_bench_id: bench }).unwrap();
    match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChooseAttachedEnergy { options, .. }) => assert_eq!(options, &vec![energy_id]),
        other => panic!("expected a retreat payment prompt, got {other:?}"),
    }
}

// 12. Devolving re-runs the card hooks' attachment refresh (Special Energy rules live in hooks).
#[test]
fn devolve_refreshes_attachment_rules() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("BASIC"), pokemon("Basic", vec![Type::Grass], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("STAGE1"), pokemon("Stage", vec![Type::Grass], None, None, Stage::Stage1));
    let evolved = put(&mut game, PlayerId::P1, "STAGE1", true);
    game.find_pokemon_slot_mut(evolved)
        .unwrap()
        .evolution_stack
        .push(CardInstance::new(CardDefId::new("BASIC"), PlayerId::P1));
    let mut hooks = RuntimeHooks::empty();
    hooks.apply_tool_stadium_effects = |game| {
        if let Some(id) = game.players[0].active.as_ref().map(|s| s.card.id) {
            let _ = game.add_marker(id, Marker::new("Refreshed"));
        }
    };
    game.set_hooks(hooks);
    let new_id = game.devolve_pokemon(evolved, 0, false).unwrap();
    assert!(game.has_marker(new_id, "Refreshed"));
}

// 13. A Knocked Out evolved Pokémon takes its lower stages to the discard pile.
#[test]
fn knockout_discards_evolution_stack() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("BASIC"), pokemon("Basic", vec![Type::Grass], None, None, Stage::Basic));
    game.card_meta.insert(CardDefId::new("STAGE1"), pokemon("Stage", vec![Type::Grass], None, None, Stage::Stage1));
    put(&mut game, PlayerId::P1, "FILL", true);
    let evolved = put(&mut game, PlayerId::P2, "STAGE1", true);
    put(&mut game, PlayerId::P2, "FILL", false);
    let lower = CardInstance::new(CardDefId::new("BASIC"), PlayerId::P2);
    let lower_id = lower.id;
    let slot = game.find_pokemon_slot_mut(evolved).unwrap();
    slot.evolution_stack.push(lower);
    slot.damage_counters = 10;
    crate::check_knockouts_all(&mut game);
    assert!(game.players[1].discard.contains(evolved));
    assert!(game.players[1].discard.contains(lower_id), "the lower stage must not vanish");
}

// 14. Searching onto a full Bench never loses a card.
#[test]
fn bench_search_is_capped_and_never_loses_cards() {
    let mut game = new_game();
    put(&mut game, PlayerId::P1, "FILL", true);
    for _ in 0..4 {
        put(&mut game, PlayerId::P1, "FILL", false);
    }
    let options: Vec<_> = game.players[0].deck.order().into_iter().take(2).collect();
    game.set_pending_prompt(
        Prompt::ChooseCardsFromDeck {
            player: PlayerId::P1,
            count: 2,
            options: options.clone(),
            revealed_cards: Vec::new(),
            min: Some(0),
            max: Some(2),
            destination: SelectionDestination::Bench,
            shuffle: true,
        },
        PlayerId::P1,
    );
    match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChooseCardsFromDeck { count, max, .. }) => {
            assert_eq!((*count, *max), (1, Some(1)), "capped at the one free Bench spot");
        }
        other => panic!("{other:?}"),
    }
    let deck_before = game.players[0].deck.count();
    // Forcing two cards past the (older) prompt check must fail before anything moves.
    let err = crate::action::execute(&mut game, Action::TakeCardsFromDeck { card_ids: options.clone() });
    assert!(err.is_err());
    assert_eq!(game.players[0].deck.count(), deck_before, "no card may be lost");
    assert!(game.pending_prompt.is_some(), "the search is still pending");
}

// 15. A target prompt never asks for more targets than it offers.
#[test]
fn target_prompt_min_is_clamped_to_options() {
    let mut game = new_game();
    let only = put(&mut game, PlayerId::P2, "FILL", true);
    game.set_pending_prompt(
        Prompt::ChoosePokemonTargets { player: PlayerId::P1, min: 2, max: 2, valid_targets: vec![only], effect_description: String::new() },
        PlayerId::P1,
    );
    match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChoosePokemonTargets { min, max, .. }) => assert_eq!((*min, *max), (1, 1)),
        other => panic!("{other:?}"),
    }
}

// 15b. Attach-from-discard does not open when the pile has fewer matches than the printed min.
#[test]
fn attach_from_discard_does_not_prompt_below_min() {
    let mut game = new_game();
    put(&mut game, PlayerId::P1, "FILL", true);
    game.card_meta.insert(CardDefId::new("E-W"), energy("Water", vec![Type::Water]));
    game.players[0].discard.add(CardInstance::new(CardDefId::new("E-W"), PlayerId::P1));
    let energy_selector = CardSelector { is_energy: Some(true), ..CardSelector::default() };
    let target_selector = PokemonSelector::default();
    assert!(
        !game.begin_attach_energy_from_discard(
            PlayerId::P1, energy_selector.clone(), target_selector.clone(), 3, 3, 3, false
        ),
        "one energy cannot start a min-3 attach"
    );
    assert!(game.pending_prompt.is_none());
    for _ in 0..2 {
        game.players[0].discard.add(CardInstance::new(CardDefId::new("E-W"), PlayerId::P1));
    }
    assert!(game.begin_attach_energy_from_discard(
        PlayerId::P1, energy_selector, target_selector, 3, 3, 3, false
    ));
    match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChooseCardsFromDiscard { min, max, options, .. }) => {
            assert_eq!((*min, *max), (Some(3), Some(3)));
            assert_eq!(options.len(), 3);
        }
        other => panic!("{other:?}"),
    }
}

// 16. A Knock Out from an attack's target prompt lets the attack finish; ChooseAttack is never re-raised.
#[test]
fn prompt_knockout_mid_attack_finishes_the_attack() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Fighting], None, None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "MON", true);
    put(&mut game, PlayerId::P2, "MON", true);
    let benched = put(&mut game, PlayerId::P2, "MON", false);
    game.find_pokemon_slot_mut(benched).unwrap().damage_counters = 9;
    let effect = EffectAst::ChoosePokemonTargets {
        player: TargetPlayer::Current,
        selector: crate::PokemonSelector { is_active: Some(false), ..crate::PokemonSelector::default() },
        min: 1,
        max: 1,
        effect: Box::new(EffectAst::PlaceDamageCounters { target: Target::Selected, counters: 1 }),
    };
    let a = attack("Pure", 0, Type::Fighting, Some(effect));
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    let targets = match game.pending_prompt.as_ref().map(|p| &p.prompt) {
        Some(Prompt::ChoosePokemonTargets { valid_targets, .. }) => valid_targets.clone(),
        other => panic!("expected the attack's target prompt, got {other:?}"),
    };
    assert!(targets.contains(&benched));
    game.apply_action(PlayerId::P1, Action::ChoosePokemonTargets { target_ids: vec![benched] }).unwrap();
    assert!(game.pending_attack.is_none(), "the attack must finish");
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::PokemonKnockedOut { pokemon_id } if *pokemon_id == benched)));
    for _ in 0..5 {
        match game.step() {
            crate::StepResult::Prompt { prompt: Prompt::ChooseAttack { .. }, .. } => panic!("ChooseAttack re-raised"),
            crate::StepResult::Prompt { .. } | crate::StepResult::GameOver { .. } => break,
            _ => {}
        }
    }
}

// 16b. A Knock Out of the Active between two prompted steps of one attack waits for the attack.
#[test]
fn active_knockout_between_prompted_steps_does_not_stall() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Fighting], None, None, Stage::Basic));
    let attacker = put(&mut game, PlayerId::P1, "MON", true);
    let defender = put(&mut game, PlayerId::P2, "MON", true);
    put(&mut game, PlayerId::P2, "MON", false);
    game.find_pokemon_slot_mut(defender).unwrap().damage_counters = 9;
    let step = || EffectAst::ChoosePokemonTargets {
        player: TargetPlayer::Current,
        selector: crate::PokemonSelector { scope: crate::PokemonScope::OppAll, ..crate::PokemonSelector::default() },
        min: 1,
        max: 1,
        effect: Box::new(EffectAst::PlaceDamageCounters { target: Target::Selected, counters: 1 }),
    };
    let a = attack("Pure", 0, Type::Fighting, Some(EffectAst::Sequence { effects: vec![step(), step()] }));
    give_attack(&mut game, attacker, &a);
    declare(&mut game, a);
    game.apply_action(PlayerId::P1, Action::ChoosePokemonTargets { target_ids: vec![defender] }).unwrap();
    // The second step still belongs to the attacker; the KO waits.
    let pending = game.pending_prompt.as_ref().map(|p| (p.for_player, p.prompt.clone()));
    assert!(matches!(pending, Some((PlayerId::P1, Prompt::ChoosePokemonTargets { .. }))), "second step prompt, got {pending:?}");
    let target = match &pending.unwrap().1 {
        Prompt::ChoosePokemonTargets { valid_targets, .. } => valid_targets[0],
        _ => unreachable!(),
    };
    game.apply_action(PlayerId::P1, Action::ChoosePokemonTargets { target_ids: vec![target] }).unwrap();
    assert!(game.pending_attack.is_none(), "the attack finished");
    assert!(game.event_log.iter().any(|e| matches!(e, GameEvent::PokemonKnockedOut { pokemon_id } if *pokemon_id == defender)));
    for _ in 0..6 {
        match game.step() {
            crate::StepResult::Prompt { prompt: Prompt::ChooseAttack { .. }, .. } => panic!("ChooseAttack re-raised"),
            crate::StepResult::Prompt { .. } | crate::StepResult::GameOver { .. } => break,
            _ => {}
        }
    }
}

// 17. Damage from a reactive OnPowerActivated trigger is settled in the same action.
#[test]
fn reactive_power_trigger_knockout_is_settled_immediately() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Psychic], None, None, Stage::Basic));
    let source = put(&mut game, PlayerId::P1, "MON", true);
    let reactive = put(&mut game, PlayerId::P2, "MON", true);
    game.find_pokemon_slot_mut(source).unwrap().damage_counters = 9;
    game.register_trigger(TriggerSubscription {
        source_id: reactive,
        trigger: TriggerKind::OnPowerActivated,
        predicate: TriggerPredicate::Always,
        effect_id: "MON:Reactive".into(),
        match_subject: false,
    });
    let mut hooks = RuntimeHooks::empty();
    hooks.power_effect_id_for = |id, name| {
        (id.as_str() == "MON" && name == "Test Power").then(|| "MON:Test Power".into())
    };
    hooks.execute_power = |game, name, source_id| {
        if name == "Test Power" {
            true
        } else if name == "Reactive" {
            game.place_damage_counters(source_id, 1, None, true)
        } else {
            false
        }
    };
    game.set_hooks(hooks);

    game.apply_action(
        PlayerId::P1,
        Action::UsePower { source_id: source, power_name: "Test Power".into() },
    )
    .unwrap();

    assert!(game.slot_by_id(source).is_none(), "trigger damage must not leave a KO in Main");
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::PokemonKnockedOut { pokemon_id } if *pokemon_id == source)
    }));
}

// 18. Custom between-turn hooks run after checkup and need their own KO pass.
#[test]
fn custom_between_turns_knockout_is_settled_before_main() {
    let mut game = new_game();
    game.card_meta.insert(CardDefId::new("MON"), pokemon("Mon", vec![Type::Psychic], None, None, Stage::Basic));
    put(&mut game, PlayerId::P1, "MON", true);
    put(&mut game, PlayerId::P2, "MON", true);
    let target = put(&mut game, PlayerId::P2, "MON", false);
    game.find_pokemon_slot_mut(target).unwrap().damage_counters = 9;
    let mut hooks = RuntimeHooks::empty();
    hooks.between_turns = |game| {
        let target = game.players[1].bench[0].card.id;
        let _ = game.place_damage_counters(target, 1, None, true);
    };
    game.set_hooks(hooks);
    game.turn.phase = Phase::BetweenTurns;

    let _ = game.step();

    assert!(game.slot_by_id(target).is_none(), "custom checkup damage must be settled immediately");
    let damage_index = game.event_log.iter().position(|event| {
        matches!(event, GameEvent::DamageDealt { pokemon_id, .. } if *pokemon_id == target)
    });
    let knockout_index = game.event_log.iter().position(|event| {
        matches!(event, GameEvent::PokemonKnockedOut { pokemon_id } if *pokemon_id == target)
    });
    assert!(damage_index.is_some() && knockout_index.is_some() && damage_index < knockout_index);
}
