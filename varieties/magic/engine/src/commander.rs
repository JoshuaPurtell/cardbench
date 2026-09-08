//! Fail-closed Commander deck construction over trusted, versioned card data.
//!
//! The engine's executable card definitions are not Oracle: mana cost/colors
//! alone cannot reconstruct color identity, commander exceptions, or legality.
//! The host supplies reviewed metadata, never metadata from a submitted deck.
use std::collections::{BTreeMap, BTreeSet};

use crate::{Color, DeckList};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommanderCardRules {
    /// Canonical English name (including interchangeable-name equivalence).
    pub name: String,
    /// Full identity, including back faces, indicators and defining abilities;
    /// reminder text is excluded. This is not just the printed mana cost.
    pub color_identity: BTreeSet<Color>,
    /// Basic-land-type mana constraints under CR 903.5d.
    pub basic_land_colors: BTreeSet<Color>,
    /// None means unlimited (basic lands or an explicit deckbuilding ability).
    pub maximum_copies: Option<u16>,
    pub legal: bool,
    pub implemented: bool,
    /// Eligible as the sole commander. Pair-only cards use co_commanders.
    pub can_be_commander: bool,
    /// Exact permitted second commander ids after evaluating pairing abilities.
    /// Both records must permit the pairing; arbitrary two legends are illegal.
    pub co_commanders: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommanderCatalog {
    /// Identifies the trusted Oracle/ban-list/eligibility snapshot.
    pub revision: String,
    pub cards: BTreeMap<String, CommanderCardRules>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommanderDeck {
    /// Exactly 100 cards INCLUDING the commander(s); no sideboard.
    pub deck: DeckList,
    pub commanders: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommanderDeckError {
    UnversionedCatalog,
    WrongSize(usize),
    Sideboard,
    CommanderCount,
    MissingCommander(String),
    IneligibleCommander(String),
    IllegalPair,
    UnknownCard(String),
    IllegalCard(String),
    UnsupportedCard(String),
    ZeroCount(String),
    Copies(String),
    ColorIdentity(String),
}

impl CommanderDeck {
    pub fn validate(&self, catalog: &CommanderCatalog) -> Result<(), CommanderDeckError> {
        use CommanderDeckError as E;
        if catalog.revision.trim().is_empty() {
            return Err(E::UnversionedCatalog);
        }
        if !self.deck.sideboard.is_empty() {
            return Err(E::Sideboard);
        }
        let size: usize = self
            .deck
            .mainboard
            .iter()
            .map(|e| usize::from(e.count))
            .sum();
        if size != 100 {
            return Err(E::WrongSize(size));
        }
        if !(1..=2).contains(&self.commanders.len()) {
            return Err(E::CommanderCount);
        }
        let mut ids = BTreeMap::<&str, usize>::new();
        let mut names = BTreeMap::<&str, (usize, Option<u16>)>::new();
        for entry in &self.deck.mainboard {
            if entry.count == 0 {
                return Err(E::ZeroCount(entry.card.clone()));
            }
            let rules = catalog
                .cards
                .get(&entry.card)
                .ok_or_else(|| E::UnknownCard(entry.card.clone()))?;
            if !rules.legal {
                return Err(E::IllegalCard(entry.card.clone()));
            }
            if !rules.implemented {
                return Err(E::UnsupportedCard(entry.card.clone()));
            }
            *ids.entry(&entry.card).or_default() += usize::from(entry.count);
            let (count, limit) = names
                .entry(&rules.name)
                .or_insert((0, rules.maximum_copies));
            // Conflicting alias metadata cannot relax the stricter copy limit.
            *limit = match (*limit, rules.maximum_copies) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a @ Some(_), None) | (None, a @ Some(_)) => a,
                (None, None) => None,
            };
            *count += usize::from(entry.count);
        }
        for (name, (count, limit)) in names {
            if limit.is_some_and(|limit| count > usize::from(limit)) {
                return Err(E::Copies(name.into()));
            }
        }
        let mut identity = BTreeSet::new();
        for id in &self.commanders {
            if ids.get(id.as_str()) != Some(&1) {
                return Err(E::MissingCommander(id.clone()));
            }
            let rules = &catalog.cards[id];
            if self.commanders.len() == 1 && !rules.can_be_commander {
                return Err(E::IneligibleCommander(id.clone()));
            }
            identity.extend(rules.color_identity.iter().copied());
        }
        if self.commanders.len() == 2 {
            let a = &self.commanders[0];
            let b = &self.commanders[1];
            if a == b
                || !catalog.cards[a].co_commanders.contains(b)
                || !catalog.cards[b].co_commanders.contains(a)
            {
                return Err(E::IllegalPair);
            }
        }
        for entry in &self.deck.mainboard {
            let rules = &catalog.cards[&entry.card];
            if !rules.color_identity.is_subset(&identity)
                || !rules.basic_land_colors.is_subset(&identity)
            {
                return Err(E::ColorIdentity(entry.card.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeckEntry;
    fn fixture() -> (CommanderDeck, CommanderCatalog) {
        let leader = CommanderCardRules {
            name: "Leader".into(),
            color_identity: BTreeSet::from([Color::Red]),
            basic_land_colors: BTreeSet::new(),
            maximum_copies: Some(1),
            legal: true,
            implemented: true,
            can_be_commander: true,
            co_commanders: BTreeSet::new(),
        };
        let land = CommanderCardRules {
            name: "Mountain".into(),
            maximum_copies: None,
            color_identity: BTreeSet::new(),
            basic_land_colors: BTreeSet::from([Color::Red]),
            can_be_commander: false,
            ..leader.clone()
        };
        (
            CommanderDeck {
                deck: DeckList {
                    mainboard: vec![
                        DeckEntry {
                            card: "leader".into(),
                            count: 1,
                        },
                        DeckEntry {
                            card: "land".into(),
                            count: 99,
                        },
                    ],
                    sideboard: vec![],
                },
                commanders: vec!["leader".into()],
            },
            CommanderCatalog {
                revision: "test-only".into(),
                cards: BTreeMap::from([("leader".into(), leader), ("land".into(), land)]),
            },
        )
    }
    #[test]
    fn accepts_exactly_one_hundred_with_basic_exception() {
        let (deck, catalog) = fixture();
        assert_eq!(deck.validate(&catalog), Ok(()));
    }
    #[test]
    fn rejects_size_sideboard_missing_ineligible_and_unknown() {
        let (deck, catalog) = fixture();
        let mut bad = deck.clone();
        bad.deck.mainboard[1].count = 98;
        assert_eq!(
            bad.validate(&catalog),
            Err(CommanderDeckError::WrongSize(99))
        );
        let mut bad = deck.clone();
        bad.deck.sideboard.push(bad.deck.mainboard[0].clone());
        assert_eq!(bad.validate(&catalog), Err(CommanderDeckError::Sideboard));
        let mut bad = deck.clone();
        bad.commanders = vec!["absent".into()];
        assert!(bad.validate(&catalog).is_err());
        let mut bad = catalog.clone();
        bad.cards.get_mut("leader").unwrap().can_be_commander = false;
        assert!(deck.validate(&bad).is_err());
        let mut bad = deck.clone();
        bad.deck.mainboard[0].card = "unknown".into();
        assert!(bad.validate(&catalog).is_err());
    }
    #[test]
    fn rejects_off_identity_lands_bans_and_unimplemented_cards() {
        let (deck, catalog) = fixture();
        let mut bad = catalog.clone();
        bad.cards
            .get_mut("land")
            .unwrap()
            .basic_land_colors
            .insert(Color::Blue);
        assert!(matches!(
            deck.validate(&bad),
            Err(CommanderDeckError::ColorIdentity(_))
        ));
        let mut bad = catalog.clone();
        bad.cards.get_mut("leader").unwrap().legal = false;
        assert!(matches!(
            deck.validate(&bad),
            Err(CommanderDeckError::IllegalCard(_))
        ));
        let mut bad = catalog.clone();
        bad.cards.get_mut("leader").unwrap().implemented = false;
        assert!(matches!(
            deck.validate(&bad),
            Err(CommanderDeckError::UnsupportedCard(_))
        ));
    }
    #[test]
    fn aliases_and_duplicate_rows_cannot_bypass_singleton() {
        let (mut deck, mut catalog) = fixture();
        catalog
            .cards
            .insert("alias".into(), catalog.cards["leader"].clone());
        deck.deck.mainboard[1].count = 98;
        deck.deck.mainboard.push(DeckEntry {
            card: "alias".into(),
            count: 1,
        });
        assert!(matches!(
            deck.validate(&catalog),
            Err(CommanderDeckError::Copies(_))
        ));
        deck.deck.mainboard[2].card = "leader".into();
        assert!(matches!(
            deck.validate(&catalog),
            Err(CommanderDeckError::Copies(_))
        ));
    }
    #[test]
    fn pairs_require_mutual_permission_and_union_identity() {
        let (mut deck, mut catalog) = fixture();
        let mut second = catalog.cards["leader"].clone();
        second.name = "Second".into();
        second.color_identity = BTreeSet::from([Color::Blue]);
        catalog.cards.insert("second".into(), second);
        deck.deck.mainboard[1].count = 98;
        deck.deck.mainboard.push(DeckEntry {
            card: "second".into(),
            count: 1,
        });
        deck.commanders.push("second".into());
        assert_eq!(
            deck.validate(&catalog),
            Err(CommanderDeckError::IllegalPair)
        );
        catalog
            .cards
            .get_mut("leader")
            .unwrap()
            .co_commanders
            .insert("second".into());
        assert_eq!(
            deck.validate(&catalog),
            Err(CommanderDeckError::IllegalPair)
        );
        catalog
            .cards
            .get_mut("second")
            .unwrap()
            .co_commanders
            .insert("leader".into());
        catalog
            .cards
            .get_mut("land")
            .unwrap()
            .basic_land_colors
            .insert(Color::Blue);
        assert_eq!(deck.validate(&catalog), Ok(()));
    }
}
