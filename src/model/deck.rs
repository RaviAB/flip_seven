use rand::prelude::SliceRandom;
use rand::rng;

use super::{BonusCard, Card};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
    draw_pile: Vec<Card>,
    discard_pile: Vec<Card>,
}

impl Deck {
    pub const NUMERIC_CARD_COUNT: usize = 79;
    pub const ACTION_CARD_COUNT: usize = 9;
    pub const BONUS_CARD_COUNT: usize = 6;
    pub const FULL_CARD_COUNT: usize =
        Self::NUMERIC_CARD_COUNT + Self::ACTION_CARD_COUNT + Self::BONUS_CARD_COUNT;

    pub fn new_full_shuffled() -> Self {
        let mut deck = Self::new_full_ordered();
        deck.shuffle_draw_pile();
        deck
    }

    pub fn new_full_ordered() -> Self {
        let mut draw_pile = Vec::with_capacity(Self::FULL_CARD_COUNT);
        draw_pile.push(Card::Number(0));

        for value in 1..=Card::MAX_NUMBER {
            for _ in 0..value {
                draw_pile.push(Card::Number(value));
            }
        }

        for _ in 0..3 {
            draw_pile.push(Card::SecondChance);
            draw_pile.push(Card::FlipThree);
            draw_pile.push(Card::Freeze);
        }

        draw_pile.extend([
            Card::Bonus(BonusCard::Plus(2)),
            Card::Bonus(BonusCard::Plus(4)),
            Card::Bonus(BonusCard::Plus(6)),
            Card::Bonus(BonusCard::Plus(8)),
            Card::Bonus(BonusCard::Plus(10)),
            Card::Bonus(BonusCard::Double),
        ]);

        Self {
            draw_pile,
            discard_pile: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn from_draw_order(draw_order: impl IntoIterator<Item = Card>) -> Self {
        let mut draw_pile = draw_order.into_iter().collect::<Vec<_>>();
        draw_pile.reverse();
        Self {
            draw_pile,
            discard_pile: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn with_draw_and_discard(
        draw_order: impl IntoIterator<Item = Card>,
        discard: impl IntoIterator<Item = Card>,
    ) -> Self {
        let mut draw_pile = draw_order.into_iter().collect::<Vec<_>>();
        draw_pile.reverse();
        Self {
            draw_pile,
            discard_pile: discard.into_iter().collect(),
        }
    }

    pub fn draw(&mut self) -> Option<Card> {
        if self.draw_pile.is_empty() {
            self.reshuffle_discard_into_draw_pile();
        }

        self.draw_pile.pop()
    }

    pub fn draw_selected(&mut self, card: Card) -> Option<Card> {
        if self.draw_pile.is_empty() {
            if !self.discard_pile.contains(&card) {
                return None;
            }
            self.reshuffle_discard_into_draw_pile();
        }

        let index = self
            .draw_pile
            .iter()
            .position(|candidate| *candidate == card)?;
        Some(self.draw_pile.remove(index))
    }

    pub fn discard(&mut self, card: Card) {
        self.discard_pile.push(card);
    }

    pub fn discard_many(&mut self, cards: impl IntoIterator<Item = Card>) {
        self.discard_pile.extend(cards);
    }

    pub fn draw_pile_count(&self) -> usize {
        self.draw_pile.len()
    }

    pub fn discard_pile_count(&self) -> usize {
        self.discard_pile.len()
    }

    pub fn total_available_count(&self) -> usize {
        self.draw_pile.len() + self.discard_pile.len()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.draw_pile.is_empty() && self.discard_pile.is_empty()
    }

    pub fn draw_pile_counts(&self) -> Vec<(Card, usize)> {
        card_counts(&self.draw_pile)
    }

    pub fn discard_pile_counts(&self) -> Vec<(Card, usize)> {
        card_counts(&self.discard_pile)
    }

    pub fn next_draw_pool_counts(&self) -> Vec<(Card, usize)> {
        if self.draw_pile.is_empty() {
            card_counts(&self.discard_pile)
        } else {
            card_counts(&self.draw_pile)
        }
    }

    #[cfg(test)]
    pub fn draw_pile(&self) -> &[Card] {
        &self.draw_pile
    }

    pub fn reshuffle_discard_into_draw_pile(&mut self) -> usize {
        let moved = self.discard_pile.len();
        if moved == 0 {
            return 0;
        }

        self.draw_pile.append(&mut self.discard_pile);
        self.shuffle_draw_pile();
        moved
    }

    fn shuffle_draw_pile(&mut self) {
        self.draw_pile.shuffle(&mut rng());
    }
}

fn card_counts(cards: &[Card]) -> Vec<(Card, usize)> {
    let mut counts = Vec::<(Card, usize)>::new();

    for card in cards {
        if let Some((_, count)) = counts.iter_mut().find(|(existing, _)| existing == card) {
            *count += 1;
        } else {
            counts.push((*card, 1));
        }
    }

    counts.sort_by_key(|(card, _)| card.sort_key());
    counts
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Deck;
    use crate::model::{BonusCard, Card};

    #[test]
    fn full_deck_has_expected_size() {
        let deck = Deck::new_full_ordered();

        assert_eq!(deck.draw_pile_count(), Deck::FULL_CARD_COUNT);
    }

    #[test]
    fn full_deck_has_expected_number_distribution() {
        let deck = Deck::new_full_ordered();
        let mut counts = HashMap::new();

        for card in deck.draw_pile() {
            if let Card::Number(value) = card {
                *counts.entry(*value).or_insert(0usize) += 1;
            }
        }

        assert_eq!(counts.get(&0), Some(&1));
        for value in 1..=Card::MAX_NUMBER {
            assert_eq!(
                counts.get(&value),
                Some(&(value as usize)),
                "unexpected count for card {value}"
            );
        }
        assert_eq!(counts.len(), 13);
    }

    #[test]
    fn full_deck_has_expected_action_cards() {
        let deck = Deck::new_full_ordered();
        let second_chances = deck
            .draw_pile()
            .iter()
            .filter(|card| matches!(card, Card::SecondChance))
            .count();
        let flip_threes = deck
            .draw_pile()
            .iter()
            .filter(|card| matches!(card, Card::FlipThree))
            .count();
        let freezes = deck
            .draw_pile()
            .iter()
            .filter(|card| matches!(card, Card::Freeze))
            .count();

        assert_eq!(second_chances, 3);
        assert_eq!(flip_threes, 3);
        assert_eq!(freezes, 3);
    }

    #[test]
    fn full_deck_has_expected_bonus_cards() {
        let deck = Deck::new_full_ordered();
        let expected = [
            Card::Bonus(BonusCard::Plus(2)),
            Card::Bonus(BonusCard::Plus(4)),
            Card::Bonus(BonusCard::Plus(6)),
            Card::Bonus(BonusCard::Plus(8)),
            Card::Bonus(BonusCard::Plus(10)),
            Card::Bonus(BonusCard::Double),
        ];

        for card in expected {
            assert_eq!(
                deck.draw_pile()
                    .iter()
                    .filter(|candidate| **candidate == card)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn drawing_reduces_draw_pile_count() {
        let mut deck = Deck::from_draw_order([Card::Number(7)]);

        let drawn = deck.draw();

        assert_eq!(drawn, Some(Card::Number(7)));
        assert_eq!(deck.draw_pile_count(), 0);
    }

    #[test]
    fn drawing_empty_deck_returns_none() {
        let mut deck = Deck::from_draw_order([]);

        assert_eq!(deck.draw(), None);
        assert!(deck.is_empty());
    }

    #[test]
    fn drawing_reshuffles_discard_when_draw_pile_is_empty() {
        let mut deck = Deck::with_draw_and_discard([], [Card::Number(3)]);

        assert_eq!(deck.draw(), Some(Card::Number(3)));
        assert_eq!(deck.draw_pile_count(), 0);
        assert_eq!(deck.discard_pile_count(), 0);
    }

    #[test]
    fn drawing_selected_card_removes_matching_card() {
        let mut deck = Deck::from_draw_order([Card::Number(3), Card::Number(4)]);

        assert_eq!(deck.draw_selected(Card::Number(4)), Some(Card::Number(4)));
        assert_eq!(deck.draw_pile_count(), 1);
        assert_eq!(deck.draw(), Some(Card::Number(3)));
    }

    #[test]
    fn drawing_unavailable_selected_card_does_not_mutate_deck() {
        let mut deck = Deck::from_draw_order([Card::Number(3)]);

        assert_eq!(deck.draw_selected(Card::Number(4)), None);
        assert_eq!(deck.draw_pile_count(), 1);
        assert_eq!(deck.draw(), Some(Card::Number(3)));
    }

    #[test]
    fn drawing_selected_card_reshuffles_discard_only_when_available() {
        let mut deck = Deck::with_draw_and_discard([], [Card::Number(3), Card::Number(4)]);

        assert_eq!(deck.draw_selected(Card::Number(4)), Some(Card::Number(4)));
        assert_eq!(deck.total_available_count(), 1);
    }

    #[test]
    fn manual_reshuffle_moves_discard_into_draw_pile() {
        let mut deck =
            Deck::with_draw_and_discard([Card::Number(1)], [Card::Number(4), Card::Freeze]);

        let moved = deck.reshuffle_discard_into_draw_pile();

        assert_eq!(moved, 2);
        assert_eq!(deck.discard_pile_count(), 0);
        assert_eq!(deck.draw_pile_count(), 3);
    }

    #[test]
    fn unavailable_selected_card_in_discard_does_not_reshuffle() {
        let mut deck = Deck::with_draw_and_discard([], [Card::Number(3)]);

        assert_eq!(deck.draw_selected(Card::Number(4)), None);
        assert_eq!(deck.draw_pile_count(), 0);
        assert_eq!(deck.discard_pile_count(), 1);
    }
}
