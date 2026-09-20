#[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
use crate::model::PlayerId;
use crate::model::{BonusCard, Card, Player, ScoreBonus, round_score_for_cards};

use super::state::GameState;

#[derive(Debug, Clone, PartialEq)]
pub struct DrawOdds {
    pub next_pool_size: usize,
    pub current_score: u32,
    pub expected_score_after_draw: f64,
    pub expected_score_delta: f64,
    pub bust_probability: f64,
    pub flip_seven_probability: f64,
    pub bust_cards: Vec<BustCardDetail>,
    pub expected_value_details: Vec<ExpectedValueDetail>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BustCardDetail {
    pub number: u8,
    pub count: usize,
    pub probability: f64,
    pub protected_by_second_chance: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedValueDetail {
    pub card: Card,
    pub count: usize,
    pub probability: f64,
    pub score_after_draw: u32,
    pub weighted_score: f64,
}

impl GameState {
    pub(crate) fn current_player_odds(&self) -> Option<DrawOdds> {
        if let Some(pending_action) = self.pending_action()
            && pending_action.awaiting_selected_draws()
            && let Some(target_player_id) = pending_action.target_player_id()
            && let Some(target_index) = self.player_index(target_player_id)
        {
            return self.draw_odds_for_player_index(target_index);
        }

        self.current_player_index()
            .and_then(|index| self.draw_odds_for_player_index(index))
    }

    #[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn draw_odds_for_player(&self, player_id: PlayerId) -> Option<DrawOdds> {
        self.player_index(player_id)
            .and_then(|index| self.draw_odds_for_player_index(index))
    }

    pub(super) fn draw_odds_for_player_index(&self, player_index: usize) -> Option<DrawOdds> {
        let player = self.live.players.get(player_index)?;
        if self.live.round_over || !player.is_active_in_round() {
            return None;
        }

        let pool_counts = self.live.deck.next_draw_pool_counts();
        let next_pool_size = pool_counts.iter().map(|(_, count)| *count).sum::<usize>();
        if next_pool_size == 0 {
            return Some(DrawOdds {
                next_pool_size: 0,
                current_score: player.round_score(ScoreBonus::None),
                expected_score_after_draw: player.round_score(ScoreBonus::None) as f64,
                expected_score_delta: 0.0,
                bust_probability: 0.0,
                flip_seven_probability: 0.0,
                bust_cards: Vec::new(),
                expected_value_details: Vec::new(),
            });
        }

        let current_score = player.round_score(ScoreBonus::None);
        let has_second_chance = player.has_second_chance();
        let distinct_numbers = player.distinct_number_count();
        let mut expected_score_after_draw = 0.0;
        let mut bust_count = 0usize;
        let mut flip_seven_count = 0usize;
        let mut bust_cards = Vec::new();
        let mut expected_value_details = Vec::new();

        for (card, count) in pool_counts {
            let score_after = score_after_one_draw(player, card);
            expected_score_after_draw += score_after as f64 * count as f64;
            let probability = count as f64 / next_pool_size as f64;
            expected_value_details.push(ExpectedValueDetail {
                card,
                count,
                probability,
                score_after_draw: score_after,
                weighted_score: score_after as f64 * probability,
            });

            if let Card::Number(value) = card {
                let duplicate = player.has_number(value);
                if duplicate {
                    if !has_second_chance {
                        bust_count += count;
                    }
                    bust_cards.push(BustCardDetail {
                        number: value,
                        count,
                        probability,
                        protected_by_second_chance: has_second_chance,
                    });
                } else if !duplicate && distinct_numbers == 6 {
                    flip_seven_count += count;
                }
            }
        }

        expected_score_after_draw /= next_pool_size as f64;

        Some(DrawOdds {
            next_pool_size,
            current_score,
            expected_score_after_draw,
            expected_score_delta: expected_score_after_draw - current_score as f64,
            bust_probability: bust_count as f64 / next_pool_size as f64,
            flip_seven_probability: flip_seven_count as f64 / next_pool_size as f64,
            bust_cards,
            expected_value_details,
        })
    }
}

fn score_after_one_draw(player: &Player, card: Card) -> u32 {
    match card {
        Card::Number(value) => {
            if player.has_number(value) {
                if player.has_second_chance() {
                    let mut cards = player.hand().to_vec();
                    if let Some(index) = cards
                        .iter()
                        .position(|card| matches!(card, Card::SecondChance))
                    {
                        cards.remove(index);
                    }
                    round_score_for_cards(&cards, ScoreBonus::None)
                } else {
                    0
                }
            } else {
                let mut cards = player.hand().to_vec();
                cards.push(card);
                round_score_for_cards(
                    &cards,
                    if player.distinct_number_count() == 6 {
                        ScoreBonus::FlipSeven
                    } else {
                        ScoreBonus::None
                    },
                )
            }
        }
        Card::Bonus(BonusCard::Plus(_)) | Card::Bonus(BonusCard::Double) => {
            let mut cards = player.hand().to_vec();
            cards.push(card);
            round_score_for_cards(&cards, ScoreBonus::None)
        }
        Card::SecondChance | Card::FlipThree | Card::Freeze => player.round_score(ScoreBonus::None),
    }
}
