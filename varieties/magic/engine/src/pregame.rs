//! CR 103.5 London mulligans over real engine zones. This controller owns the
//! game until setup is complete, so callers cannot start play between choices.
//! Opening-hand special abilities require a separate set-specific capability;
//! this entry point is for pools with no such abilities (including RAV).
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MulliganChoice { Keep, Mulligan }

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PregamePrompt {
    Choose { player: PlayerId, round: u8, may_mulligan: bool },
    /// Card identities are available only through the choosing player's view.
    Bottom { player: PlayerId, round: u8, count: usize },
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(seats: usize) -> Game {
        let forest = CardDefinition {
            id: "RAV-FOREST", name: "Forest", set_code: "RAV",
            mana_cost: ManaCost::new(0), colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Land]), is_basic_land: true,
            supported_rules: &["basic-land"], power: None, toughness: None, keywords: vec![], effects: vec![],
        };
        let mut game = Game::new([forest], seats).unwrap();
        for seat in 0..seats {
            game.load_deck_into_library(PlayerId(seat), &DeckList {
                mainboard: vec![crate::DeckEntry { card: "RAV-FOREST".into(), count: 60 }], sideboard: vec![],
            }).unwrap();
        }
        game
    }

    #[test]
    fn simultaneous_redraws_and_immediate_private_bottom_before_next_round() {
        let mut setup = LondonPregame::new(prepared(2), PlayerId(1)).unwrap();
        let original = setup.game.players[1].hand.clone();
        setup.choose(PlayerId(1), 0, MulliganChoice::Mulligan).unwrap();
        assert_eq!(setup.game.players[1].hand, original);
        setup.choose(PlayerId(0), 0, MulliganChoice::Keep).unwrap();
        assert_eq!(setup.prompt(), PregamePrompt::Bottom { player: PlayerId(1), round: 1, count: 1 });
        let card = setup.game.players[1].hand[0];
        let foreign = setup.game.players[0].hand[0];
        assert!(setup.bottom(PlayerId(1), 1, &[foreign]).is_err());
        assert!(setup.bottom(PlayerId(0), 1, &[card]).is_err());
        setup.bottom(PlayerId(1), 1, &[card]).unwrap();
        assert_eq!(setup.game.players[1].hand.len(), 6);
        assert_eq!(setup.game.players[1].library.first(), Some(&card));
        assert!(setup.choose(PlayerId(1), 0, MulliganChoice::Keep).is_err());
        setup.choose(PlayerId(1), 1, MulliganChoice::Keep).unwrap();
        let mut game = setup.finish().unwrap();
        assert_eq!(game.active_player, PlayerId(1));
        for _ in 0..8 {
            if game.step == Step::Draw { break; }
            game.pass_priority(game.priority).unwrap();
        }
        assert_eq!(game.step, Step::Draw);
        assert!(game.pending_draw_replacement.is_none());
        assert_eq!(game.players[1].hand.len(), 6);
        game.validate_invariants().unwrap();
    }

    #[test]
    fn multiplayer_first_is_free_second_bottoms_one_and_starting_player_draws() {
        let mut setup = LondonPregame::new(prepared(4), PlayerId(2)).unwrap();
        for seat in [2, 3, 0, 1] {
            setup.choose(PlayerId(seat), 0, if seat == 2 { MulliganChoice::Mulligan } else { MulliganChoice::Keep }).unwrap();
        }
        assert_eq!(setup.game.players[2].hand.len(), 7);
        assert!(matches!(setup.prompt(), PregamePrompt::Choose { player: PlayerId(2), round: 1, .. }));
        setup.choose(PlayerId(2), 1, MulliganChoice::Mulligan).unwrap();
        assert_eq!(setup.prompt(), PregamePrompt::Bottom { player: PlayerId(2), round: 2, count: 1 });
        let card = setup.game.players[2].hand[0];
        setup.bottom(PlayerId(2), 2, &[card]).unwrap();
        setup.choose(PlayerId(2), 2, MulliganChoice::Keep).unwrap();
        let mut game = setup.finish().unwrap();
        for _ in 0..8 {
            if game.step == Step::Draw { break; }
            game.pass_priority(game.priority).unwrap();
        }
        assert_eq!(game.pending_draw_replacement, Some(PlayerId(2)));
    }

    #[test]
    fn refuses_fixture_boards_and_early_start() {
        let mut game = prepared(2);
        game.put_on_battlefield(PlayerId(0), "RAV-FOREST").unwrap();
        assert!(LondonPregame::new(game, PlayerId(0)).is_err());
        let setup = LondonPregame::new(prepared(2), PlayerId(0)).unwrap();
        assert!(setup.finish().is_err());
    }

    #[test]
    fn strict_two_headed_giant_admission_groups_teammates_and_rejects_wrong_life() {
        let mut game = prepared(4);
        let teams = [vec![PlayerId(0), PlayerId(2)], vec![PlayerId(1), PlayerId(3)]];
        game.configure_teams(&teams, 20).unwrap();
        assert!(LondonPregame::for_two_headed_giant(game.clone(), PlayerId(1)).is_err());
        game.configure_teams(&teams, 30).unwrap();
        let mut setup = LondonPregame::for_two_headed_giant(game, PlayerId(1)).unwrap();
        for seat in [1, 3, 2, 0] {
            assert_eq!(setup.prompt(), PregamePrompt::Choose {
                player: PlayerId(seat), round: 0, may_mulligan: true,
            });
            setup.choose(PlayerId(seat), 0, MulliganChoice::Keep).unwrap();
        }
        let game = setup.finish().unwrap();
        assert_eq!(game.active_player, PlayerId(1));
        assert!(game.players.iter().all(|seat| seat.hand.len() == 7 && seat.library.len() == 53));
        game.validate_invariants().unwrap();
    }

    #[test]
    fn can_mulligan_to_zero_but_not_beyond_and_invalid_bottom_is_atomic() {
        let mut setup = LondonPregame::new(prepared(2), PlayerId(0)).unwrap();
        setup.choose(PlayerId(0), 0, MulliganChoice::Mulligan).unwrap();
        setup.choose(PlayerId(1), 0, MulliganChoice::Keep).unwrap();
        for round in 1..=7 {
            let cards = setup.game.players[0].hand[..usize::from(round)].to_vec();
            if round > 1 {
                let bad = vec![cards[0]; usize::from(round)];
                let before = setup.game.players[0].hand.clone();
                assert!(setup.bottom(PlayerId(0), round, &bad).is_err());
                assert_eq!(setup.game.players[0].hand, before);
            }
            setup.bottom(PlayerId(0), round, &cards).unwrap();
            if round < 7 { setup.choose(PlayerId(0), round, MulliganChoice::Mulligan).unwrap(); }
        }
        assert!(setup.choose(PlayerId(0), 7, MulliganChoice::Mulligan).is_err());
        setup.choose(PlayerId(0), 7, MulliganChoice::Keep).unwrap();
        let game = setup.finish().unwrap();
        assert!(game.players[0].hand.is_empty());
        assert_eq!(game.players[0].library.len(), 60);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PregameEvent {
    Choice { player: PlayerId, round: u8, choice: MulliganChoice },
    Redrawn { player: PlayerId, mulligans: u8 },
    Bottomed { player: PlayerId, count: usize },
}

#[derive(Clone)]
pub struct LondonPregame {
    game: Game,
    first: PlayerId,
    order: Vec<PlayerId>,
    cursor: usize,
    round: u8,
    mulligans: Vec<u8>,
    kept: Vec<bool>,
    redraw: Vec<PlayerId>,
    bottom: Vec<PlayerId>,
    bottom_phase: bool,
    events: Vec<PregameEvent>,
}

impl LondonPregame {
    /// Standard Commander admission over the actual loaded physical cards.
    /// The metadata snapshot is trusted host data, not a policy submission.
    pub fn for_commander(game: Game, first: PlayerId, metadata: &crate::CommanderCatalog) -> Result<Self, RulesError> {
        if game.format.has_teams() || game.format.commander_damage_threshold != Some(21)
            || game.players.iter().any(|seat| seat.life != 40) {
            return Err(RulesError::IllegalAction("standard Commander requires 40 life and 21 commander damage"));
        }
        for seat in &game.players {
            let mut counts = BTreeMap::<String, u8>::new();
            for object in seat.library.iter().chain(&seat.command) {
                let definition = game.card_definition(*object)?;
                let rules = metadata.cards.get(definition.id)
                    .ok_or(RulesError::IllegalAction("Commander metadata is missing a loaded card"))?;
                if rules.name != definition.name {
                    return Err(RulesError::IllegalAction("Commander metadata disagrees with executable card name"));
                }
                let count = counts.entry(definition.id.to_owned()).or_default();
                *count = count.checked_add(1).ok_or(RulesError::IllegalAction("oversized Commander deck"))?;
            }
            let commanders = game.format.commanders.iter().filter(|(_, record)| record.owner == seat.id)
                .map(|(card, _)| {
                    if game.zone_of(*card) != Some(Zone::Command) {
                        return Err(RulesError::IllegalAction("pregame commander must be in the command zone"));
                    }
                    Ok(game.card_definition(*card)?.id.to_owned())
                }).collect::<Result<Vec<_>, RulesError>>()?;
            crate::CommanderDeck {
                deck: DeckList { mainboard: counts.into_iter().map(|(card, count)| crate::DeckEntry { card, count }).collect(), sideboard: vec![] },
                commanders,
            }.validate(metadata).map_err(|_| RulesError::IllegalAction("loaded Commander deck is not format legal"))?;
        }
        Self::new(game, first)
    }

    /// Constructed 2HG admission: four 60-card seats, two two-player teams,
    /// 30 shared life, and the four-copy limit across each team's decks.
    pub fn for_two_headed_giant(game: Game, first: PlayerId) -> Result<Self, RulesError> {
        game.validate_invariants()?;
        if game.players.len() != 4 || game.format.teams.len() != 2
            || game.format.teams.iter().any(|team| team.len() != 2)
            || game.format.commander_damage_threshold.is_some()
            || game.players.iter().any(|seat| seat.life != 30 || seat.library.len() != 60 || !seat.command.is_empty()) {
            return Err(RulesError::IllegalAction("constructed 2HG requires two two-player teams and legal full decks"));
        }
        for team in &game.format.teams {
            let mut counts = BTreeMap::<&str, usize>::new();
            for player in team {
                for card in &game.players[player.0].library {
                    let definition = game.card_definition(*card)?;
                    if !definition.is_basic_land {
                        let count = counts.entry(definition.name).or_default();
                        *count += 1;
                        if *count > 4 {
                            return Err(RulesError::IllegalAction("2HG team exceeds the shared card-name copy limit"));
                        }
                    }
                }
            }
        }
        Self::new(game, first)
    }

    /// The host must load and validate the format's decks first, including
    /// Commander metadata or the shared 2HG card-name limit. Rejects fixture
    /// boards, pre-drawn hands, missing decks, and already-started games.
    pub fn new(mut game: Game, first: PlayerId) -> Result<Self, RulesError> {
        game.player(first)?;
        if game.started || game.players.iter().any(|seat|
            seat.lost || seat.library.len() < 40 || !seat.hand.is_empty()
                || !seat.battlefield.is_empty() || !seat.graveyard.is_empty()
                || !seat.exile.is_empty())
        {
            return Err(RulesError::IllegalAction("London setup requires unstarted full-deck seats"));
        }
        game.validate_invariants()?;
        let n = game.players.len();
        let mut order: Vec<_> = (0..n).map(|offset| PlayerId((first.0 + offset) % n)).collect();
        // Shared-team-turn declarations are grouped by team, starting with the
        // chosen starting team, rather than accidentally alternating teammates.
        if game.format.has_teams() {
            let mut seen = BTreeSet::new();
            let mut grouped = Vec::new();
            for player in &order {
                if let Some(team) = game.format.team_of(*player) {
                    if seen.insert(team) {
                        grouped.extend(order.iter().copied().filter(|p| game.format.team_of(*p) == Some(team)));
                    }
                }
            }
            if grouped.len() != n {
                return Err(RulesError::IllegalAction("every setup seat must belong to a team"));
            }
            order = grouped;
        }
        game.atomic_transition(|game| {
            for player in &order { game.draw_opening_hand(*player, 7)?; }
            Ok(())
        })?;
        Ok(Self { game, first, order, cursor: 0, round: 0,
            mulligans: vec![0; n], kept: vec![false; n], redraw: vec![],
            bottom: vec![], bottom_phase: false, events: vec![] })
    }

    fn bottom_count(&self, player: PlayerId) -> usize {
        usize::from(self.mulligans[player.0].saturating_sub(u8::from(self.game.players.len() > 2)))
    }

    pub fn prompt(&self) -> PregamePrompt {
        if self.bottom_phase {
            return self.bottom.first().map_or(PregamePrompt::Complete, |player|
                PregamePrompt::Bottom { player: *player, round: self.round, count: self.bottom_count(*player) });
        }
        let player = self.order[self.cursor];
        PregamePrompt::Choose { player, round: self.round, may_mulligan: self.bottom_count(player) < 7 }
    }

    pub fn view_for_player(&self, player: PlayerId) -> Result<GameView, RulesError> {
        self.game.view_for_player(player)
    }

    pub fn events(&self) -> &[PregameEvent] { &self.events }

    pub fn choose(&mut self, player: PlayerId, round: u8, choice: MulliganChoice) -> Result<(), RulesError> {
        let before = self.clone();
        let result = self.choose_impl(player, round, choice);
        if result.is_err() { *self = before; }
        result
    }

    fn choose_impl(&mut self, player: PlayerId, round: u8, choice: MulliganChoice) -> Result<(), RulesError> {
        let PregamePrompt::Choose { player: expected, round: expected_round, may_mulligan } = self.prompt() else {
            return Err(RulesError::IllegalAction("no mulligan declaration is pending"));
        };
        if player != expected || round != expected_round || (choice == MulliganChoice::Mulligan && !may_mulligan) {
            return Err(RulesError::IllegalAction("invalid mulligan declaration"));
        }
        // Delay actual redraws until every undecided player has announced.
        // A later seat cannot see an earlier seat's new hand before deciding.
        match choice {
            MulliganChoice::Keep => self.kept[player.0] = true,
            MulliganChoice::Mulligan => self.redraw.push(player),
        }
        self.events.push(PregameEvent::Choice { player, round: self.round, choice });
        self.cursor += 1;
        while self.cursor < self.order.len() && self.kept[self.order[self.cursor].0] {
            self.cursor += 1;
        }
        if self.cursor < self.order.len() { return Ok(()); }
        if self.redraw.is_empty() {
            self.bottom_phase = true;
            return Ok(());
        }
        self.game.atomic_transition(|game| {
            for player in &self.redraw {
                let hand = game.players[player.0].hand.clone();
                for card in hand { game.move_to_zone(card, Zone::Library)?; }
                game.shuffle_library_and_record(*player)?;
                game.draw_opening_hand(*player, 7)?;
            }
            Ok(())
        })?;
        for player in self.redraw.drain(..) {
            self.mulligans[player.0] += 1;
            self.events.push(PregameEvent::Redrawn { player, mulligans: self.mulligans[player.0] });
        }
        self.round += 1;
        self.cursor = self.order.iter().position(|p| !self.kept[p.0]).expect("a redrawing player remains");
        // Bottom immediately after each redraw, before the next round's
        // keep/mulligan announcements (CR 103.5), not only after final keeps.
        self.bottom = self.order.iter().copied()
            .filter(|p| !self.kept[p.0] && self.bottom_count(*p) > 0).collect();
        self.bottom_phase = !self.bottom.is_empty();
        Ok(())
    }

    /// Ordered top-to-bottom within the bottom packet; no opponent may submit
    /// another player's hand identities or replace an already accepted choice.
    pub fn bottom(&mut self, player: PlayerId, round: u8, cards: &[ObjectId]) -> Result<(), RulesError> {
        let PregamePrompt::Bottom { player: expected, round: expected_round, count } = self.prompt() else {
            return Err(RulesError::IllegalAction("no bottom choice is pending"));
        };
        if player != expected || round != expected_round || cards.len() != count
            || cards.iter().collect::<BTreeSet<_>>().len() != count
            || cards.iter().any(|card| !self.game.players[player.0].hand.contains(card))
        {
            return Err(RulesError::IllegalAction("invalid private London bottom choice"));
        }
        self.game.atomic_transition(|game| {
            for card in cards { game.move_to_zone(*card, Zone::Library)?; }
            // Library top is the end of the vector; preserve the supplied
            // top-to-bottom packet order while placing it beneath all others.
            let library = &mut game.players[player.0].library;
            library.retain(|card| !cards.contains(card));
            for card in cards { library.insert(0, *card); }
            game.refresh_public_state_integrity();
            game.validate_invariants()
        })?;
        self.bottom.remove(0);
        if self.bottom.is_empty() { self.bottom_phase = false; }
        self.events.push(PregameEvent::Bottomed { player, count });
        Ok(())
    }

    pub fn finish(mut self) -> Result<Game, RulesError> {
        if self.prompt() != PregamePrompt::Complete {
            return Err(RulesError::IllegalAction("pregame decisions remain unresolved"));
        }
        self.game.atomic_transition(|game| game.begin_game_from_impl(self.first))?;
        Ok(self.game)
    }
}
