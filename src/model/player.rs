use std::collections::HashSet;

use super::{BonusCard, Card};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(usize);

impl PlayerId {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn index(self) -> usize {
        self.0
    }

    pub fn display_number(self) -> usize {
        self.0 + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStatus {
    Active,
    Stayed,
    Frozen,
    Busted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreBreakdown {
    pub number_sum: u32,
    pub multiplier: u32,
    pub additive_bonus: u32,
    pub flip_seven_bonus: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    id: PlayerId,
    name: String,
    hand: Vec<Card>,
    status: PlayerStatus,
}

impl Player {
    pub fn new(id: PlayerId) -> Self {
        Self {
            name: format!("Player {}", id.display_number()),
            id,
            hand: Vec::new(),
            status: PlayerStatus::Active,
        }
    }

    pub fn id(&self) -> PlayerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn hand(&self) -> &[Card] {
        &self.hand
    }

    pub fn status(&self) -> PlayerStatus {
        self.status
    }

    pub fn is_active_in_round(&self) -> bool {
        self.status == PlayerStatus::Active
    }

    pub fn receive_card(&mut self, card: Card) {
        self.hand.push(card);
    }

    pub fn has_number(&self, value: u8) -> bool {
        self.hand
            .iter()
            .any(|card| matches!(card, Card::Number(existing) if *existing == value))
    }

    pub fn has_second_chance(&self) -> bool {
        self.hand
            .iter()
            .any(|card| matches!(card, Card::SecondChance))
    }

    pub fn use_second_chance(&mut self) -> bool {
        let Some(index) = self
            .hand
            .iter()
            .position(|card| matches!(card, Card::SecondChance))
        else {
            return false;
        };

        self.hand.remove(index);
        true
    }

    pub fn stay(&mut self) {
        self.status = PlayerStatus::Stayed;
    }

    pub fn freeze(&mut self) {
        self.status = PlayerStatus::Frozen;
    }

    pub fn bust(&mut self) {
        self.status = PlayerStatus::Busted;
    }

    pub fn reset_round(&mut self) -> Vec<Card> {
        self.status = PlayerStatus::Active;
        std::mem::take(&mut self.hand)
    }

    pub fn drain_hand(&mut self) -> Vec<Card> {
        std::mem::take(&mut self.hand)
    }

    pub fn distinct_number_count(&self) -> usize {
        self.hand
            .iter()
            .filter_map(|card| match card {
                Card::Number(value) => Some(*value),
                _ => None,
            })
            .collect::<HashSet<_>>()
            .len()
    }

    pub fn has_flip_seven(&self) -> bool {
        self.distinct_number_count() >= 7
    }

    pub fn round_score(&self, flip_seven_bonus: bool) -> u32 {
        if self.status == PlayerStatus::Busted {
            return 0;
        }

        score_breakdown_for_cards(&self.hand, flip_seven_bonus).total
    }

    pub fn score_breakdown(&self, flip_seven_bonus: bool) -> ScoreBreakdown {
        if self.status == PlayerStatus::Busted {
            return ScoreBreakdown {
                number_sum: 0,
                multiplier: 1,
                additive_bonus: 0,
                flip_seven_bonus: 0,
                total: 0,
            };
        }

        score_breakdown_for_cards(&self.hand, flip_seven_bonus)
    }
}

pub fn round_score_for_cards(cards: &[Card], flip_seven_bonus: bool) -> u32 {
    score_breakdown_for_cards(cards, flip_seven_bonus).total
}

pub fn score_breakdown_for_cards(cards: &[Card], flip_seven_bonus: bool) -> ScoreBreakdown {
    let number_sum = cards
        .iter()
        .filter_map(|card| match card {
            Card::Number(value) => Some(*value as u32),
            _ => None,
        })
        .sum::<u32>();
    let additive_bonus = cards
        .iter()
        .filter_map(|card| match card {
            Card::Bonus(BonusCard::Plus(value)) => Some(*value as u32),
            _ => None,
        })
        .sum::<u32>();
    let multiplier = if cards
        .iter()
        .any(|card| matches!(card, Card::Bonus(BonusCard::Double)))
    {
        2
    } else {
        1
    };
    let flip_seven_bonus = if flip_seven_bonus { 15 } else { 0 };
    let total = number_sum * multiplier + additive_bonus + flip_seven_bonus;

    ScoreBreakdown {
        number_sum,
        multiplier,
        additive_bonus,
        flip_seven_bonus,
        total,
    }
}
