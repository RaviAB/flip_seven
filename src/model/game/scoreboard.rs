use crate::model::{Player, PlayerId, PlayerStatus};

use super::card_resolution::FlipSevenBonus;
use super::events::DealOutcome;
use super::state::GameState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundScore {
    pub round_number: u32,
    pub score: u32,
    pub outcome: RoundOutcome,
    pub cards_dealt: usize,
    pub bust_risk_samples: usize,
    pub average_bust_risk_basis_points: u32,
    pub peak_bust_risk_basis_points: u32,
    pub second_chance_protected_samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundOutcome {
    Stayed,
    Frozen,
    Busted,
    FlipSeven,
    RoundEndedActive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerScore {
    pub player_id: PlayerId,
    pub player_name: String,
    pub total_score: u32,
    pub last_round_score: u32,
    pub current_round_score: Option<u32>,
    pub round_scores: Vec<RoundScore>,
    pub rounds_completed: usize,
    pub total_cards_dealt: usize,
    pub stayed_count: usize,
    pub frozen_count: usize,
    pub busted_count: usize,
    pub flip_seven_count: usize,
    pub round_ended_active_count: usize,
    pub bust_risk_sample_count: usize,
    pub bust_risk_sum_basis_points: u64,
    pub peak_bust_risk_basis_points: u32,
    pub second_chance_protected_samples: usize,
    pub current_round_cards_dealt: usize,
    pub current_round_bust_risk_samples: usize,
    pub current_round_bust_risk_sum_basis_points: u64,
    pub current_round_peak_bust_risk_basis_points: u32,
    pub current_round_second_chance_protected_samples: usize,
    pub current_round_outcome: Option<RoundOutcome>,
}

impl PlayerScore {
    pub fn average_round_score(&self) -> Option<f64> {
        (!self.round_scores.is_empty())
            .then(|| self.total_score as f64 / self.round_scores.len() as f64)
    }

    pub fn best_round_score(&self) -> Option<u32> {
        self.round_scores.iter().map(|round| round.score).max()
    }

    pub fn worst_round_score(&self) -> Option<u32> {
        self.round_scores.iter().map(|round| round.score).min()
    }

    pub fn average_bust_risk_basis_points(&self) -> Option<u32> {
        average_basis_points(self.bust_risk_sum_basis_points, self.bust_risk_sample_count)
    }

    pub fn current_round_average_bust_risk_basis_points(&self) -> Option<u32> {
        average_basis_points(
            self.current_round_bust_risk_sum_basis_points,
            self.current_round_bust_risk_samples,
        )
    }
}

impl PlayerScore {
    pub(super) fn new(player: &Player) -> Self {
        Self {
            player_id: player.id(),
            player_name: player.name().to_owned(),
            total_score: 0,
            last_round_score: 0,
            current_round_score: None,
            round_scores: Vec::new(),
            rounds_completed: 0,
            total_cards_dealt: 0,
            stayed_count: 0,
            frozen_count: 0,
            busted_count: 0,
            flip_seven_count: 0,
            round_ended_active_count: 0,
            bust_risk_sample_count: 0,
            bust_risk_sum_basis_points: 0,
            peak_bust_risk_basis_points: 0,
            second_chance_protected_samples: 0,
            current_round_cards_dealt: 0,
            current_round_bust_risk_samples: 0,
            current_round_bust_risk_sum_basis_points: 0,
            current_round_peak_bust_risk_basis_points: 0,
            current_round_second_chance_protected_samples: 0,
            current_round_outcome: None,
        }
    }
}

fn average_basis_points(sum: u64, count: usize) -> Option<u32> {
    (count > 0).then(|| (sum / count as u64) as u32)
}

fn probability_to_basis_points(probability: f64) -> u32 {
    (probability.clamp(0.0, 1.0) * 10_000.0).round() as u32
}
impl GameState {
    pub fn score_to_win_target(&self, player_id: PlayerId, target_score: u32) -> Option<u32> {
        self.player_score(player_id)
            .map(|score| target_score.saturating_sub(score.total_score))
    }

    pub(super) fn end_round(&mut self, reason: String) -> DealOutcome {
        self.round_over = true;
        self.pending_action = None;
        self.queued_actions.clear();

        for player_index in 0..self.players.len() {
            let player_id = self.players[player_index].id();
            if self
                .score_board
                .iter()
                .any(|score| score.player_id == player_id && score.current_round_score.is_some())
            {
                continue;
            }

            let flip_seven_bonus = self.flip_seven_player_id == Some(player_id);
            let outcome = if flip_seven_bonus {
                RoundOutcome::FlipSeven
            } else {
                match self.players[player_index].status() {
                    PlayerStatus::Active => RoundOutcome::RoundEndedActive,
                    PlayerStatus::Stayed => RoundOutcome::Stayed,
                    PlayerStatus::Frozen => RoundOutcome::Frozen,
                    PlayerStatus::Busted => RoundOutcome::Busted,
                }
            };
            self.bank_player_score(
                player_index,
                if flip_seven_bonus {
                    FlipSevenBonus::Yes
                } else {
                    FlipSevenBonus::No
                },
                outcome,
            );
        }

        for score in &mut self.score_board {
            let round_score = score.current_round_score.unwrap_or(0);
            let average_bust_risk_basis_points = average_basis_points(
                score.current_round_bust_risk_sum_basis_points,
                score.current_round_bust_risk_samples,
            )
            .unwrap_or(0);
            score.last_round_score = round_score;
            score.total_score += round_score;
            score.rounds_completed += 1;
            match score
                .current_round_outcome
                .unwrap_or(RoundOutcome::RoundEndedActive)
            {
                RoundOutcome::Stayed => score.stayed_count += 1,
                RoundOutcome::Frozen => score.frozen_count += 1,
                RoundOutcome::Busted => score.busted_count += 1,
                RoundOutcome::FlipSeven => score.flip_seven_count += 1,
                RoundOutcome::RoundEndedActive => score.round_ended_active_count += 1,
            }
            score.round_scores.push(RoundScore {
                round_number: self.round_number,
                score: round_score,
                outcome: score
                    .current_round_outcome
                    .unwrap_or(RoundOutcome::RoundEndedActive),
                cards_dealt: score.current_round_cards_dealt,
                bust_risk_samples: score.current_round_bust_risk_samples,
                average_bust_risk_basis_points,
                peak_bust_risk_basis_points: score.current_round_peak_bust_risk_basis_points,
                second_chance_protected_samples: score
                    .current_round_second_chance_protected_samples,
            });
        }

        DealOutcome::RoundEnded { reason }
    }

    pub(super) fn finish_player(
        &mut self,
        player_index: usize,
        status: PlayerStatus,
        flip_seven_bonus: FlipSevenBonus,
    ) {
        let outcome = match status {
            PlayerStatus::Active => RoundOutcome::RoundEndedActive,
            PlayerStatus::Stayed => RoundOutcome::Stayed,
            PlayerStatus::Frozen => RoundOutcome::Frozen,
            PlayerStatus::Busted => RoundOutcome::Busted,
        };
        match status {
            PlayerStatus::Active => {}
            PlayerStatus::Stayed => self.players[player_index].stay(),
            PlayerStatus::Frozen => self.players[player_index].freeze(),
            PlayerStatus::Busted => self.players[player_index].bust(),
        }
        self.bank_player_score(player_index, flip_seven_bonus, outcome);
    }

    pub(super) fn bank_player_score(
        &mut self,
        player_index: usize,
        flip_seven_bonus: FlipSevenBonus,
        outcome: RoundOutcome,
    ) {
        let player_id = self.players[player_index].id();
        let round_score = self.players[player_index].round_score(flip_seven_bonus.applies());
        let discarded = self.players[player_index].drain_hand();
        self.deck.discard_many(discarded);

        if let Some(score) = self
            .score_board
            .iter_mut()
            .find(|score| score.player_id == player_id)
        {
            score.current_round_score = Some(round_score);
            score.current_round_outcome = Some(outcome);
        }
    }

    pub(super) fn record_pre_draw_telemetry(&mut self, player_index: usize) {
        let Some(player_id) = self.players.get(player_index).map(Player::id) else {
            return;
        };
        let Some(odds) = self.draw_odds_for_player_index(player_index) else {
            return;
        };
        let bust_risk_basis_points = probability_to_basis_points(odds.bust_probability);
        let protected_by_second_chance = odds
            .bust_cards
            .iter()
            .any(|detail| detail.protected_by_second_chance);

        if let Some(score) = self
            .score_board
            .iter_mut()
            .find(|score| score.player_id == player_id)
        {
            score.total_cards_dealt += 1;
            score.bust_risk_sample_count += 1;
            score.bust_risk_sum_basis_points += bust_risk_basis_points as u64;
            score.peak_bust_risk_basis_points = score
                .peak_bust_risk_basis_points
                .max(bust_risk_basis_points);
            score.current_round_cards_dealt += 1;
            score.current_round_bust_risk_samples += 1;
            score.current_round_bust_risk_sum_basis_points += bust_risk_basis_points as u64;
            score.current_round_peak_bust_risk_basis_points = score
                .current_round_peak_bust_risk_basis_points
                .max(bust_risk_basis_points);

            if protected_by_second_chance {
                score.second_chance_protected_samples += 1;
                score.current_round_second_chance_protected_samples += 1;
            }
        }
    }

    pub(super) fn reset_current_round_scoreboard(&mut self) {
        for score in &mut self.score_board {
            score.current_round_score = None;
            score.current_round_cards_dealt = 0;
            score.current_round_bust_risk_samples = 0;
            score.current_round_bust_risk_sum_basis_points = 0;
            score.current_round_peak_bust_risk_basis_points = 0;
            score.current_round_second_chance_protected_samples = 0;
            score.current_round_outcome = None;
        }
    }
}
