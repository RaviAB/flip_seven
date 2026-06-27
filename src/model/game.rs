use super::{
    BonusCard, Card, Deck, Player, PlayerId, PlayerStatus, round_score_for_cards,
    strategy::{AiDecision, DecisionContext},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialAction {
    FlipThree,
    Freeze,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    action: SpecialAction,
    source_player_id: PlayerId,
    target_player_id: Option<PlayerId>,
    remaining_draws: u8,
}

impl PendingAction {
    pub fn action(&self) -> SpecialAction {
        self.action
    }

    pub fn source_player_id(&self) -> PlayerId {
        self.source_player_id
    }

    pub fn target_player_id(&self) -> Option<PlayerId> {
        self.target_player_id
    }

    pub fn remaining_draws(&self) -> u8 {
        self.remaining_draws
    }

    pub fn needs_target(&self) -> bool {
        self.target_player_id.is_none()
    }

    pub fn awaiting_selected_draws(&self) -> bool {
        self.action == SpecialAction::FlipThree
            && self.target_player_id.is_some()
            && self.remaining_draws > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionChoice {
    Player(PlayerId),
}

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

fn average_basis_points(sum: u64, count: usize) -> Option<u32> {
    (count > 0).then(|| (sum / count as u64) as u32)
}

fn probability_to_basis_points(probability: f64) -> u32 {
    (probability.clamp(0.0, 1.0) * 10_000.0).round() as u32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DealOutcome {
    DealtNumber {
        player_id: PlayerId,
        player_name: String,
        card: Card,
    },
    Bonus {
        player_id: PlayerId,
        player_name: String,
        card: Card,
    },
    SecondChance {
        player_id: PlayerId,
        player_name: String,
    },
    UsedSecondChance {
        player_id: PlayerId,
        player_name: String,
        duplicate: Card,
    },
    Busted {
        player_id: PlayerId,
        player_name: String,
        duplicate: Card,
    },
    Stayed {
        player_id: PlayerId,
        player_name: String,
    },
    FlipSeven {
        player_id: PlayerId,
        player_name: String,
    },
    RoundEnded {
        reason: String,
    },
    NewRoundStarted,
    SpecialNeedsTarget {
        action: SpecialAction,
        source_player_id: PlayerId,
        source_player_name: String,
    },
    SpecialResolved {
        action: SpecialAction,
        target_player_id: PlayerId,
        target_player_name: String,
        drawn_cards: Vec<Card>,
        notes: Vec<String>,
    },
    FlipThreeTargetSelected {
        target_player_id: PlayerId,
        target_player_name: String,
        remaining_draws: u8,
    },
    FlipThreeCardDealt {
        target_player_id: PlayerId,
        target_player_name: String,
        card: Card,
        remaining_draws: u8,
        notes: Vec<String>,
    },
    WaitingForTarget {
        action: SpecialAction,
    },
    InvalidTarget,
    UndoApplied,
    NothingToUndo,
    NoPlayers,
    NoActivePlayers,
    RoundOver,
    DeckEmpty,
    SelectedCardUnavailable,
    DiscardReshuffled {
        cards_moved: usize,
    },
    NoDiscardToReshuffle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    players: Vec<Player>,
    deck: Deck,
    current_player_index: usize,
    pending_action: Option<PendingAction>,
    queued_actions: Vec<PendingAction>,
    score_board: Vec<PlayerScore>,
    round_number: u32,
    round_start_player_index: usize,
    round_over: bool,
    flip_seven_player_id: Option<PlayerId>,
    history: Vec<GameSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameSnapshot {
    players: Vec<Player>,
    deck: Deck,
    current_player_index: usize,
    pending_action: Option<PendingAction>,
    queued_actions: Vec<PendingAction>,
    score_board: Vec<PlayerScore>,
    round_number: u32,
    round_start_player_index: usize,
    round_over: bool,
    flip_seven_player_id: Option<PlayerId>,
}

impl GameState {
    pub fn new(player_count: usize) -> Self {
        Self::with_deck(player_count, Deck::new_full_shuffled())
    }

    pub fn with_deck(player_count: usize, deck: Deck) -> Self {
        let players = (0..player_count)
            .map(|index| Player::new(PlayerId::new(index)))
            .collect::<Vec<_>>();
        let score_board = players
            .iter()
            .map(|player| PlayerScore {
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
            })
            .collect();

        Self {
            players,
            deck,
            current_player_index: 0,
            pending_action: None,
            queued_actions: Vec::new(),
            score_board,
            round_number: 1,
            round_start_player_index: 0,
            round_over: false,
            flip_seven_player_id: None,
            history: Vec::new(),
        }
    }

    pub fn reset(&mut self, player_count: usize) {
        self.save_snapshot();
        *self = Self {
            history: std::mem::take(&mut self.history),
            ..Self::new(player_count)
        };
    }

    pub fn start_next_round(&mut self) -> DealOutcome {
        if !self.round_over {
            return DealOutcome::RoundOver;
        }

        self.save_snapshot();
        for player in &mut self.players {
            self.deck.discard_many(player.reset_round());
        }
        self.pending_action = None;
        self.queued_actions.clear();
        self.flip_seven_player_id = None;
        self.round_over = false;
        self.round_number += 1;
        for score in &mut self.score_board {
            score.current_round_score = None;
            score.current_round_cards_dealt = 0;
            score.current_round_bust_risk_samples = 0;
            score.current_round_bust_risk_sum_basis_points = 0;
            score.current_round_peak_bust_risk_basis_points = 0;
            score.current_round_second_chance_protected_samples = 0;
            score.current_round_outcome = None;
        }
        if !self.players.is_empty() {
            self.round_start_player_index =
                (self.round_start_player_index + 1) % self.players.len();
        }
        self.current_player_index = self.round_start_player_index;
        self.advance_to_next_active_player_from(self.round_start_player_index);

        DealOutcome::NewRoundStarted
    }

    pub fn reshuffle_discard_into_draw_pile(&mut self) -> DealOutcome {
        if self.deck.discard_pile_count() == 0 {
            return DealOutcome::NoDiscardToReshuffle;
        }

        self.save_snapshot();
        let cards_moved = self.deck.reshuffle_discard_into_draw_pile();
        DealOutcome::DiscardReshuffled { cards_moved }
    }

    pub fn deal_next_card(&mut self) -> DealOutcome {
        if self.round_over {
            return DealOutcome::RoundOver;
        }

        if let Some(pending_action) = &self.pending_action {
            return DealOutcome::WaitingForTarget {
                action: pending_action.action,
            };
        }

        if self.players.is_empty() {
            return DealOutcome::NoPlayers;
        }

        let Some(player_index) = self.next_active_player_index_from(self.current_player_index)
        else {
            return self.end_round("No active players remain.".to_owned());
        };

        let snapshot = self.snapshot();
        if self.deck.next_draw_pool_counts().is_empty() {
            return DealOutcome::DeckEmpty;
        }

        self.history.push(snapshot);
        self.current_player_index = player_index;
        self.record_pre_draw_telemetry(player_index);
        let Some(card) = self.deck.draw() else {
            return DealOutcome::DeckEmpty;
        };

        self.apply_card_to_current_player(card, true)
    }

    pub fn deal_selected_card(&mut self, card: Card) -> DealOutcome {
        if self.round_over {
            return DealOutcome::RoundOver;
        }

        if let Some(pending_action) = self.pending_action.clone() {
            if pending_action.awaiting_selected_draws() {
                return self.deal_selected_flip_three_card(card, pending_action);
            }

            return DealOutcome::WaitingForTarget {
                action: pending_action.action,
            };
        }

        if self.players.is_empty() {
            return DealOutcome::NoPlayers;
        }

        let Some(player_index) = self.next_active_player_index_from(self.current_player_index)
        else {
            return self.end_round("No active players remain.".to_owned());
        };

        let snapshot = self.snapshot();
        if !self
            .deck
            .next_draw_pool_counts()
            .iter()
            .any(|(candidate, _)| *candidate == card)
        {
            return DealOutcome::SelectedCardUnavailable;
        }

        self.history.push(snapshot);
        self.current_player_index = player_index;
        self.record_pre_draw_telemetry(player_index);
        let Some(card) = self.deck.draw_selected(card) else {
            return DealOutcome::SelectedCardUnavailable;
        };

        self.apply_card_to_current_player(card, true)
    }

    fn deal_selected_flip_three_card(
        &mut self,
        card: Card,
        pending_action: PendingAction,
    ) -> DealOutcome {
        let Some(target_player_id) = pending_action.target_player_id else {
            return DealOutcome::WaitingForTarget {
                action: pending_action.action,
            };
        };
        let Some(target_index) = self.player_index(target_player_id) else {
            return DealOutcome::InvalidTarget;
        };

        if !self.players[target_index].is_active_in_round() {
            return DealOutcome::InvalidTarget;
        }

        if !self
            .deck
            .next_draw_pool_counts()
            .iter()
            .any(|(candidate, _)| *candidate == card)
        {
            return DealOutcome::SelectedCardUnavailable;
        }

        self.save_snapshot();
        self.record_pre_draw_telemetry(target_index);
        let Some(card) = self.deck.draw_selected(card) else {
            return DealOutcome::SelectedCardUnavailable;
        };

        let target_player_name = self.players[target_index].name().to_owned();
        let mut notes = vec![self.apply_card_to_player_index(target_index, card, true)];

        if self.players[target_index].has_flip_seven() {
            self.flip_seven_player_id = Some(target_player_id);
            return self.end_round(format!("{target_player_name} hit Flip 7."));
        }

        let remaining_draws = pending_action.remaining_draws.saturating_sub(1);
        let target_still_active = self.players[target_index].is_active_in_round();

        if remaining_draws > 0 && target_still_active {
            self.pending_action = Some(PendingAction {
                remaining_draws,
                ..pending_action
            });
        } else {
            self.pending_action = None;
            if !target_still_active {
                notes.push(format!("{target_player_name} is no longer active."));
            }
            self.advance_to_next_active_player_after(pending_action.source_player_id);

            if self.no_active_players() {
                return self.end_round("All players are done for the round.".to_owned());
            }

            self.activate_next_queued_action();
        }

        DealOutcome::FlipThreeCardDealt {
            target_player_id,
            target_player_name,
            card,
            remaining_draws: if target_still_active {
                remaining_draws
            } else {
                0
            },
            notes,
        }
    }

    pub fn stay_current_player(&mut self) -> DealOutcome {
        if self.round_over {
            return DealOutcome::RoundOver;
        }
        if let Some(pending_action) = &self.pending_action {
            return DealOutcome::WaitingForTarget {
                action: pending_action.action,
            };
        }
        let Some(player_index) = self.next_active_player_index_from(self.current_player_index)
        else {
            return self.end_round("No active players remain.".to_owned());
        };

        self.save_snapshot();
        self.current_player_index = player_index;
        let player_id = self.players[player_index].id();
        let player_name = self.players[player_index].name().to_owned();
        self.finish_player(player_index, PlayerStatus::Stayed, false);
        self.advance_to_next_active_player_after(player_id);

        if self.no_active_players() {
            self.end_round("All players are done for the round.".to_owned())
        } else {
            DealOutcome::Stayed {
                player_id,
                player_name,
            }
        }
    }

    pub fn resolve_pending_action(&mut self, choice: ActionChoice) -> DealOutcome {
        if self.round_over {
            return DealOutcome::RoundOver;
        }

        let Some(pending_action) = self.pending_action.clone() else {
            return DealOutcome::InvalidTarget;
        };

        let ActionChoice::Player(target_player_id) = choice;
        let Some(target_index) = self.player_index(target_player_id) else {
            return DealOutcome::InvalidTarget;
        };

        if !self.players[target_index].is_active_in_round() {
            return DealOutcome::InvalidTarget;
        }

        self.save_snapshot();
        self.pending_action = None;
        let mut notes = Vec::new();
        match pending_action.action {
            SpecialAction::Freeze => {
                self.finish_player(target_index, PlayerStatus::Frozen, false);
                notes.push(format!("{} is frozen.", self.players[target_index].name()));
            }
            SpecialAction::FlipThree => {
                self.current_player_index = target_index;
                self.pending_action = Some(PendingAction {
                    target_player_id: Some(target_player_id),
                    remaining_draws: 3,
                    ..pending_action
                });
                return DealOutcome::FlipThreeTargetSelected {
                    target_player_id,
                    target_player_name: self.players[target_index].name().to_owned(),
                    remaining_draws: 3,
                };
            }
        }

        if self.players[target_index].has_flip_seven() {
            return self.end_round(format!("{} hit Flip 7.", self.players[target_index].name()));
        }

        self.advance_to_next_active_player_after(pending_action.source_player_id);

        if self.no_active_players() {
            return self.end_round("All players are done for the round.".to_owned());
        }

        self.activate_next_queued_action();

        DealOutcome::SpecialResolved {
            action: pending_action.action,
            target_player_id,
            target_player_name: self.players[target_index].name().to_owned(),
            drawn_cards: Vec::new(),
            notes,
        }
    }

    pub fn undo(&mut self) -> DealOutcome {
        let Some(snapshot) = self.history.pop() else {
            return DealOutcome::NothingToUndo;
        };

        self.players = snapshot.players;
        self.deck = snapshot.deck;
        self.current_player_index = snapshot.current_player_index;
        self.pending_action = snapshot.pending_action;
        self.queued_actions = snapshot.queued_actions;
        self.score_board = snapshot.score_board;
        self.round_number = snapshot.round_number;
        self.round_start_player_index = snapshot.round_start_player_index;
        self.round_over = snapshot.round_over;
        self.flip_seven_player_id = snapshot.flip_seven_player_id;
        DealOutcome::UndoApplied
    }

    pub fn players(&self) -> &[Player] {
        &self.players
    }

    pub fn score_board(&self) -> &[PlayerScore] {
        &self.score_board
    }

    pub fn player_score(&self, player_id: PlayerId) -> Option<&PlayerScore> {
        self.score_board
            .iter()
            .find(|score| score.player_id == player_id)
    }

    pub fn round_number(&self) -> u32 {
        self.round_number
    }

    pub fn round_over(&self) -> bool {
        self.round_over
    }

    pub fn current_player_index(&self) -> Option<usize> {
        (!self.players.is_empty()).then_some(self.current_player_index)
    }

    pub fn current_player(&self) -> Option<&Player> {
        self.current_player_index()
            .and_then(|index| self.players.get(index))
    }

    pub fn current_ai_player_id(&self) -> Option<PlayerId> {
        if self.round_over {
            return None;
        }

        if let Some(pending_action) = &self.pending_action {
            return pending_action
                .needs_target()
                .then_some(pending_action.source_player_id());
        }

        self.current_player()
            .filter(|player| player.is_active_in_round())
            .map(Player::id)
    }

    pub fn decision_context(&self) -> Option<DecisionContext> {
        let player_id = self.current_ai_player_id()?;
        let draw_odds = self.draw_odds_for_player(player_id);

        Some(DecisionContext {
            player_id,
            pending_action: self.pending_action.clone(),
            legal_targets: self.legal_active_targets(),
            draw_odds,
        })
    }

    pub fn legal_ai_decisions(&self) -> Vec<AiDecision> {
        if self.round_over {
            return Vec::new();
        }

        if let Some(pending_action) = &self.pending_action {
            if pending_action.needs_target() {
                return self
                    .legal_active_targets()
                    .into_iter()
                    .map(AiDecision::ChooseTarget)
                    .collect();
            }

            return Vec::new();
        }

        if self
            .current_player()
            .is_some_and(Player::is_active_in_round)
        {
            vec![AiDecision::Stay, AiDecision::Draw]
        } else {
            Vec::new()
        }
    }

    pub fn apply_ai_decision(&mut self, decision: AiDecision) -> DealOutcome {
        match decision {
            AiDecision::Stay => self.stay_current_player(),
            AiDecision::ChooseTarget(player_id) => {
                self.resolve_pending_action(ActionChoice::Player(player_id))
            }
            AiDecision::Draw => DealOutcome::SelectedCardUnavailable,
        }
    }

    pub fn legal_active_targets(&self) -> Vec<PlayerId> {
        self.players
            .iter()
            .filter(|player| player.is_active_in_round())
            .map(Player::id)
            .collect()
    }

    pub fn current_player_odds(&self) -> Option<DrawOdds> {
        if let Some(pending_action) = &self.pending_action
            && pending_action.awaiting_selected_draws()
            && let Some(target_player_id) = pending_action.target_player_id()
            && let Some(target_index) = self.player_index(target_player_id)
        {
            return self.draw_odds_for_player_index(target_index);
        }

        self.current_player_index()
            .and_then(|index| self.draw_odds_for_player_index(index))
    }

    pub fn draw_odds_for_player(&self, player_id: PlayerId) -> Option<DrawOdds> {
        self.player_index(player_id)
            .and_then(|index| self.draw_odds_for_player_index(index))
    }

    pub fn score_to_win_target(&self, player_id: PlayerId, target_score: u32) -> Option<u32> {
        self.player_score(player_id)
            .map(|score| target_score.saturating_sub(score.total_score))
    }

    pub fn clone_for_simulation(&self) -> Self {
        Self {
            history: Vec::new(),
            ..self.clone()
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_deck_for_test(&mut self, deck: Deck) {
        self.deck = deck;
    }

    #[cfg(test)]
    pub(crate) fn set_total_score_for_test(&mut self, player_id: PlayerId, total_score: u32) {
        if let Some(score) = self
            .score_board
            .iter_mut()
            .find(|score| score.player_id == player_id)
        {
            score.total_score = total_score;
        }
    }

    #[cfg(test)]
    pub(crate) fn force_bust_for_test(&mut self, player_id: PlayerId) {
        if let Some(index) = self.player_index(player_id) {
            self.players[index].bust();
        }
    }

    fn draw_odds_for_player_index(&self, player_index: usize) -> Option<DrawOdds> {
        let player = self.players.get(player_index)?;
        if self.round_over || !player.is_active_in_round() {
            return None;
        }

        let pool_counts = self.deck.next_draw_pool_counts();
        let next_pool_size = pool_counts.iter().map(|(_, count)| *count).sum::<usize>();
        if next_pool_size == 0 {
            return Some(DrawOdds {
                next_pool_size: 0,
                current_score: player.round_score(false),
                expected_score_after_draw: player.round_score(false) as f64,
                expected_score_delta: 0.0,
                bust_probability: 0.0,
                flip_seven_probability: 0.0,
                bust_cards: Vec::new(),
                expected_value_details: Vec::new(),
            });
        }

        let current_score = player.round_score(false);
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

    pub fn pending_action(&self) -> Option<&PendingAction> {
        self.pending_action.as_ref()
    }

    pub fn queued_action_count(&self) -> usize {
        self.queued_actions.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn draw_pile_count(&self) -> usize {
        self.deck.draw_pile_count()
    }

    pub fn discard_pile_count(&self) -> usize {
        self.deck.discard_pile_count()
    }

    pub fn total_available_cards(&self) -> usize {
        self.deck.total_available_count()
    }

    pub fn draw_pile_counts(&self) -> Vec<(Card, usize)> {
        self.deck.draw_pile_counts()
    }

    pub fn discard_pile_counts(&self) -> Vec<(Card, usize)> {
        self.deck.discard_pile_counts()
    }

    pub fn next_draw_pool_counts(&self) -> Vec<(Card, usize)> {
        self.deck.next_draw_pool_counts()
    }

    fn apply_card_to_current_player(
        &mut self,
        card: Card,
        advance_after_card: bool,
    ) -> DealOutcome {
        let player_index = self.current_player_index;
        let player_id = self.players[player_index].id();
        let player_name = self.players[player_index].name().to_owned();

        match self
            .apply_card_to_player_index(player_index, card, false)
            .as_str()
        {
            "number" => {
                if self.players[player_index].has_flip_seven() {
                    self.flip_seven_player_id = Some(player_id);
                    return self.end_round(format!("{player_name} hit Flip 7."));
                }

                if advance_after_card {
                    self.advance_to_next_active_player_after(player_id);
                }
                DealOutcome::DealtNumber {
                    player_id,
                    player_name,
                    card,
                }
            }
            "bonus" => {
                if advance_after_card {
                    self.advance_to_next_active_player_after(player_id);
                }
                DealOutcome::Bonus {
                    player_id,
                    player_name,
                    card,
                }
            }
            "second" => {
                if advance_after_card {
                    self.advance_to_next_active_player_after(player_id);
                }
                DealOutcome::SecondChance {
                    player_id,
                    player_name,
                }
            }
            "used_second" => {
                if advance_after_card {
                    self.advance_to_next_active_player_after(player_id);
                }
                DealOutcome::UsedSecondChance {
                    player_id,
                    player_name,
                    duplicate: card,
                }
            }
            "busted" => {
                if advance_after_card {
                    self.advance_to_next_active_player_after(player_id);
                }
                let outcome = DealOutcome::Busted {
                    player_id,
                    player_name,
                    duplicate: card,
                };

                if self.no_active_players() {
                    self.end_round("All players are done for the round.".to_owned())
                } else {
                    outcome
                }
            }
            "flip_three" => DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::FlipThree,
                source_player_id: player_id,
                source_player_name: player_name,
            },
            "freeze" => DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::Freeze,
                source_player_id: player_id,
                source_player_name: player_name,
            },
            _ => DealOutcome::DeckEmpty,
        }
    }

    fn apply_card_to_player_index(
        &mut self,
        player_index: usize,
        card: Card,
        queue_specials: bool,
    ) -> String {
        match card {
            Card::Number(value) => {
                if self.players[player_index].has_number(value) {
                    self.deck.discard(card);
                    if self.players[player_index].use_second_chance() {
                        self.deck.discard(Card::SecondChance);
                        "used_second".to_owned()
                    } else {
                        self.finish_player(player_index, PlayerStatus::Busted, false);
                        "busted".to_owned()
                    }
                } else {
                    self.players[player_index].receive_card(card);
                    "number".to_owned()
                }
            }
            Card::Bonus(_) => {
                self.players[player_index].receive_card(card);
                "bonus".to_owned()
            }
            Card::SecondChance => {
                self.players[player_index].receive_card(card);
                "second".to_owned()
            }
            Card::FlipThree => {
                self.deck.discard(card);
                let action = PendingAction {
                    action: SpecialAction::FlipThree,
                    source_player_id: self.players[player_index].id(),
                    target_player_id: None,
                    remaining_draws: 0,
                };
                if queue_specials {
                    self.queued_actions.push(action);
                    "queued Flip Three for later resolution".to_owned()
                } else {
                    self.pending_action = Some(action);
                    "flip_three".to_owned()
                }
            }
            Card::Freeze => {
                self.deck.discard(card);
                let action = PendingAction {
                    action: SpecialAction::Freeze,
                    source_player_id: self.players[player_index].id(),
                    target_player_id: None,
                    remaining_draws: 0,
                };
                if queue_specials {
                    self.queued_actions.push(action);
                    "queued Freeze for later resolution".to_owned()
                } else {
                    self.pending_action = Some(action);
                    "freeze".to_owned()
                }
            }
        }
    }

    fn end_round(&mut self, reason: String) -> DealOutcome {
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
            self.bank_player_score(player_index, flip_seven_bonus, outcome);
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

    fn finish_player(&mut self, player_index: usize, status: PlayerStatus, flip_seven_bonus: bool) {
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

    fn bank_player_score(
        &mut self,
        player_index: usize,
        flip_seven_bonus: bool,
        outcome: RoundOutcome,
    ) {
        let player_id = self.players[player_index].id();
        let round_score = self.players[player_index].round_score(flip_seven_bonus);
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

    fn record_pre_draw_telemetry(&mut self, player_index: usize) {
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

    fn activate_next_queued_action(&mut self) {
        if self.pending_action.is_none() && !self.queued_actions.is_empty() {
            self.pending_action = Some(self.queued_actions.remove(0));
        }
    }

    fn save_snapshot(&mut self) {
        self.history.push(self.snapshot());
    }

    fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            players: self.players.clone(),
            deck: self.deck.clone(),
            current_player_index: self.current_player_index,
            pending_action: self.pending_action.clone(),
            queued_actions: self.queued_actions.clone(),
            score_board: self.score_board.clone(),
            round_number: self.round_number,
            round_start_player_index: self.round_start_player_index,
            round_over: self.round_over,
            flip_seven_player_id: self.flip_seven_player_id,
        }
    }

    fn player_index(&self, player_id: PlayerId) -> Option<usize> {
        self.players
            .iter()
            .position(|player| player.id() == player_id)
    }

    fn advance_to_next_active_player_after(&mut self, player_id: PlayerId) {
        let Some(source_index) = self.player_index(player_id) else {
            self.current_player_index = 0;
            return;
        };

        self.advance_to_next_active_player_from((source_index + 1) % self.players.len());
    }

    fn advance_to_next_active_player_from(&mut self, start_index: usize) {
        self.current_player_index = self
            .next_active_player_index_from(start_index)
            .unwrap_or(start_index.min(self.players.len().saturating_sub(1)));
    }

    fn next_active_player_index_from(&self, start_index: usize) -> Option<usize> {
        if self.players.is_empty() {
            return None;
        }

        (0..self.players.len())
            .map(|offset| (start_index + offset) % self.players.len())
            .find(|index| self.players[*index].status() == PlayerStatus::Active)
    }

    fn no_active_players(&self) -> bool {
        self.players
            .iter()
            .all(|player| !player.is_active_in_round())
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
                    round_score_for_cards(&cards, false)
                } else {
                    0
                }
            } else {
                let mut cards = player.hand().to_vec();
                cards.push(card);
                round_score_for_cards(&cards, player.distinct_number_count() == 6)
            }
        }
        Card::Bonus(BonusCard::Plus(_)) | Card::Bonus(BonusCard::Double) => {
            let mut cards = player.hand().to_vec();
            cards.push(card);
            round_score_for_cards(&cards, false)
        }
        Card::SecondChance | Card::FlipThree | Card::Freeze => player.round_score(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
        GameState::with_deck(player_count, Deck::from_draw_order(cards))
    }

    #[test]
    fn first_deal_goes_to_first_player_and_advances_turn() {
        let mut game = game_with_draw_order(3, vec![Card::Number(7)]);

        let outcome = game.deal_next_card();

        assert!(matches!(
            outcome,
            DealOutcome::DealtNumber {
                player_id,
                ..
            } if player_id.index() == 0
        ));
        assert_eq!(game.players()[0].hand().len(), 1);
        assert_eq!(game.players()[1].hand().len(), 0);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn dealing_cycles_through_players_in_order() {
        let mut game = game_with_draw_order(
            3,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
            ],
        );

        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();

        assert_eq!(game.players()[0].hand().len(), 2);
        assert_eq!(game.players()[1].hand().len(), 1);
        assert_eq!(game.players()[2].hand().len(), 1);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn duplicate_number_busts_player_without_second_chance() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(5)]);

        game.deal_next_card();
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Busted);
        assert_eq!(game.players()[0].hand().len(), 0);
        assert_eq!(game.discard_pile_count(), 2);
        assert_eq!(game.score_board()[0].last_round_score, 0);
        assert_eq!(game.score_board()[0].round_scores[0].score, 0);
    }

    #[test]
    fn second_chance_is_removed_instead_of_busting_on_duplicate() {
        let mut game = game_with_draw_order(
            1,
            vec![Card::Number(5), Card::SecondChance, Card::Number(5)],
        );

        game.deal_next_card();
        game.deal_next_card();
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::UsedSecondChance { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
        assert_eq!(game.players()[0].hand(), &[Card::Number(5)]);
    }

    #[test]
    fn stay_ends_round_when_all_players_are_done_and_scores() {
        let mut game =
            game_with_draw_order(1, vec![Card::Number(5), Card::Bonus(BonusCard::Plus(10))]);

        game.deal_next_card();
        game.deal_next_card();
        let outcome = game.stay_current_player();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Stayed);
        assert_eq!(game.score_board()[0].last_round_score, 15);
        assert_eq!(game.score_board()[0].total_score, 15);
    }

    #[test]
    fn x2_bonus_doubles_numbers_before_additive_bonuses() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(5),
                Card::Bonus(BonusCard::Double),
                Card::Bonus(BonusCard::Plus(10)),
            ],
        );

        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();
        game.stay_current_player();

        assert_eq!(game.score_board()[0].last_round_score, 20);
    }

    #[test]
    fn flip_seven_ends_round_with_bonus() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );

        for _ in 0..6 {
            game.deal_next_card();
        }
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert!(game.round_over());
        assert_eq!(game.score_board()[0].last_round_score, 36);
    }

    #[test]
    fn next_round_preserves_score_and_discards_previous_hands() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.stay_current_player();
        let discard_before = game.discard_pile_count();

        assert_eq!(discard_before, 1);
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
        assert_eq!(game.score_board()[0].total_score, 5);
        assert_eq!(game.discard_pile_count(), 1);
        assert_eq!(game.round_number(), 2);
    }

    #[test]
    fn next_round_rotates_starting_player() {
        let mut game = game_with_draw_order(3, vec![]);

        assert_eq!(game.current_player_index(), Some(0));
        game.stay_current_player();
        game.stay_current_player();
        game.stay_current_player();

        assert!(game.round_over());
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert_eq!(game.current_player_index(), Some(1));

        game.stay_current_player();
        game.stay_current_player();
        game.stay_current_player();
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert_eq!(game.current_player_index(), Some(2));
    }

    #[test]
    fn manual_reshuffle_moves_discard_into_draw_pile_and_is_undoable() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.stay_current_player();
        assert_eq!(game.draw_pile_count(), 0);
        assert_eq!(game.discard_pile_count(), 1);

        assert_eq!(
            game.reshuffle_discard_into_draw_pile(),
            DealOutcome::DiscardReshuffled { cards_moved: 1 }
        );
        assert_eq!(game.draw_pile_count(), 1);
        assert_eq!(game.discard_pile_count(), 0);

        assert_eq!(game.undo(), DealOutcome::UndoApplied);
        assert_eq!(game.draw_pile_count(), 0);
        assert_eq!(game.discard_pile_count(), 1);
    }

    #[test]
    fn scoreboard_tracks_score_per_round() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(6)]);

        game.deal_next_card();
        game.stay_current_player();
        game.start_next_round();
        game.deal_next_card();
        game.stay_current_player();

        assert_eq!(game.score_board()[0].round_scores.len(), 2);
        assert_eq!(game.score_board()[0].round_scores[0].round_number, 1);
        assert_eq!(game.score_board()[0].round_scores[0].score, 5);
        assert_eq!(game.score_board()[0].round_scores[1].round_number, 2);
        assert_eq!(game.score_board()[0].round_scores[1].score, 6);
        assert_eq!(game.score_board()[0].total_score, 11);
        assert_eq!(game.score_board()[0].rounds_completed, 2);
        assert_eq!(game.score_board()[0].average_round_score(), Some(5.5));
        assert_eq!(game.score_board()[0].best_round_score(), Some(6));
        assert_eq!(game.score_board()[0].worst_round_score(), Some(5));
    }

    #[test]
    fn scoreboard_tracks_selected_deal_bust_risk_samples() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);
        game.deal_selected_card(Card::Number(6));

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 2);
        assert_eq!(score.current_round_cards_dealt, 2);
        assert_eq!(score.bust_risk_sample_count, 2);
        assert_eq!(score.average_bust_risk_basis_points(), Some(2500));
        assert_eq!(
            score.current_round_average_bust_risk_basis_points(),
            Some(2500)
        );
        assert_eq!(score.peak_bust_risk_basis_points, 5000);
        assert_eq!(score.current_round_peak_bust_risk_basis_points, 5000);
    }

    #[test]
    fn scoreboard_tracks_second_chance_protected_risk_samples() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);

        game.deal_next_card();
        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);
        game.deal_selected_card(Card::Number(6));

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 3);
        assert_eq!(score.average_bust_risk_basis_points(), Some(0));
        assert_eq!(score.second_chance_protected_samples, 1);
        assert_eq!(score.current_round_second_chance_protected_samples, 1);
    }

    #[test]
    fn scoreboard_tracks_round_outcomes_and_cards_dealt() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.stay_current_player();

        let player_one = &game.score_board()[0];
        let player_two = &game.score_board()[1];
        assert_eq!(player_one.round_scores[0].outcome, RoundOutcome::Stayed);
        assert_eq!(player_one.stayed_count, 1);
        assert_eq!(player_one.round_scores[0].cards_dealt, 1);
        assert_eq!(player_two.round_scores[0].outcome, RoundOutcome::Frozen);
        assert_eq!(player_two.frozen_count, 1);
        assert_eq!(player_two.round_scores[0].cards_dealt, 0);
    }

    #[test]
    fn scoreboard_tracks_flip_seven_outcome() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );

        for _ in 0..7 {
            game.deal_next_card();
        }

        let score = &game.score_board()[0];
        assert_eq!(score.round_scores[0].outcome, RoundOutcome::FlipSeven);
        assert_eq!(score.flip_seven_count, 1);
        assert_eq!(score.round_scores[0].cards_dealt, 7);
    }

    #[test]
    fn undo_restores_scoreboard_telemetry() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(6)]);

        game.deal_next_card();
        assert_eq!(game.score_board()[0].total_cards_dealt, 1);

        assert_eq!(game.undo(), DealOutcome::UndoApplied);

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 0);
        assert_eq!(score.current_round_cards_dealt, 0);
        assert_eq!(score.bust_risk_sample_count, 0);
        assert_eq!(score.average_bust_risk_basis_points(), None);
    }

    #[test]
    fn freeze_waits_for_player_selection_and_freezes_target() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        let outcome = game.deal_next_card();
        assert!(matches!(
            outcome,
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert!(game.pending_action().is_some());

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::SpecialResolved {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert_eq!(game.players()[1].status(), PlayerStatus::Frozen);
        assert_eq!(game.current_player_index(), Some(0));
    }

    #[test]
    fn flip_three_waits_for_player_selection_and_deals_three_to_target() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
            ],
        );

        game.deal_next_card();
        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected {
                target_player_id,
                remaining_draws: 3,
                ..
            } if target_player_id == PlayerId::new(1)
        ));
        assert!(matches!(
            game.deal_selected_card(Card::Number(1)),
            DealOutcome::FlipThreeCardDealt {
                remaining_draws: 2,
                ..
            }
        ));
        game.deal_selected_card(Card::Number(2));
        game.deal_selected_card(Card::Number(3));
        assert_eq!(game.players()[1].hand().len(), 3);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn selected_number_deals_exact_card() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3), Card::Number(4)]);

        let outcome = game.deal_selected_card(Card::Number(4));

        assert!(matches!(
            outcome,
            DealOutcome::DealtNumber {
                card: Card::Number(4),
                ..
            }
        ));
        assert_eq!(game.players()[0].hand(), &[Card::Number(4)]);
        assert_eq!(game.draw_pile_count(), 1);
    }

    #[test]
    fn unavailable_selected_card_does_not_mutate_game() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3)]);

        let outcome = game.deal_selected_card(Card::Number(4));

        assert_eq!(outcome, DealOutcome::SelectedCardUnavailable);
        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.draw_pile_count(), 1);
        assert!(!game.can_undo());
    }

    #[test]
    fn selected_special_card_uses_pending_target_flow() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        let outcome = game.deal_selected_card(Card::Freeze);

        assert!(matches!(
            outcome,
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert!(game.pending_action().is_some());
    }

    #[test]
    fn selected_flip_three_supports_nested_flip_three_resolution() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
            ],
        );

        assert!(matches!(
            game.deal_selected_card(Card::FlipThree),
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::FlipThree,
                ..
            }
        ));

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        assert!(matches!(
            game.deal_selected_card(Card::FlipThree),
            DealOutcome::FlipThreeCardDealt {
                remaining_draws: 2,
                ..
            }
        ));
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));
        assert_eq!(
            game.players()[1].hand(),
            &[Card::Number(1), Card::Number(2)]
        );
        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::FlipThree)
        ));

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(0)));
        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        game.deal_selected_card(Card::Number(3));
        game.deal_selected_card(Card::Number(4));
        game.deal_selected_card(Card::Number(5));
        assert_eq!(
            game.players()[0].hand(),
            &[Card::Number(3), Card::Number(4), Card::Number(5)]
        );
        assert!(game.pending_action().is_none());
    }

    #[test]
    fn selected_deal_can_be_undone_with_deck_restored() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3), Card::Number(4)]);

        game.deal_selected_card(Card::Number(4));
        assert_eq!(game.undo(), DealOutcome::UndoApplied);

        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.draw_pile_count(), 2);
        assert_eq!(
            game.deal_selected_card(Card::Number(4)),
            DealOutcome::DealtNumber {
                player_id: PlayerId::new(0),
                player_name: "Player 1".to_owned(),
                card: Card::Number(4),
            }
        );
    }

    #[test]
    fn flip_three_keeps_second_chance_and_continues_dealing() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::SecondChance,
                Card::Number(1),
                Card::Number(2),
            ],
        );

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.deal_selected_card(Card::SecondChance);
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));

        assert_eq!(
            game.players()[1].hand(),
            &[Card::SecondChance, Card::Number(1), Card::Number(2)]
        );
        assert!(game.pending_action().is_none());
    }

    #[test]
    fn flip_three_queues_freeze_and_finishes_current_sequence() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Freeze,
                Card::Number(1),
                Card::Number(2),
            ],
        );

        game.deal_next_card();
        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        game.deal_selected_card(Card::Freeze);
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));
        assert_eq!(
            game.players()[1].hand(),
            &[Card::Number(1), Card::Number(2)]
        );
        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::Freeze)
        ));
    }

    #[test]
    fn queued_specials_resolve_fifo_after_current_sequence() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Freeze,
                Card::FlipThree,
                Card::Number(2),
                Card::Number(3),
            ],
        );

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.deal_selected_card(Card::Freeze);
        game.deal_selected_card(Card::FlipThree);
        game.deal_selected_card(Card::Number(2));

        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::Freeze)
        ));
        assert_eq!(game.queued_action_count(), 1);

        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(0)));

        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::FlipThree)
        ));
        assert_eq!(game.queued_action_count(), 0);
    }

    #[test]
    fn undo_restores_previous_deal_state() {
        let mut game = game_with_draw_order(2, vec![Card::Number(1), Card::Number(2)]);

        game.deal_next_card();
        game.deal_next_card();

        assert_eq!(game.undo(), DealOutcome::UndoApplied);
        assert_eq!(game.players()[0].hand().len(), 1);
        assert_eq!(game.players()[1].hand().len(), 0);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn odds_report_bust_flip_seven_and_expected_value() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );
        for _ in 0..6 {
            game.deal_next_card();
        }
        game.deck = Deck::from_draw_order([
            Card::Number(1),
            Card::Number(7),
            Card::Bonus(BonusCard::Plus(10)),
            Card::Freeze,
        ]);

        let odds = game.current_player_odds().expect("odds");

        assert_eq!(odds.next_pool_size, 4);
        assert_eq!(odds.bust_probability, 0.25);
        assert_eq!(odds.flip_seven_probability, 0.25);
        assert_eq!(odds.bust_cards.len(), 1);
        assert_eq!(odds.bust_cards[0].number, 1);
        assert_eq!(odds.bust_cards[0].count, 1);
        assert!(!odds.bust_cards[0].protected_by_second_chance);
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(1) && detail.score_after_draw == 0)
        );
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(7) && detail.score_after_draw == 43)
        );
        assert!(odds.expected_score_after_draw > odds.current_score as f64);
    }

    #[test]
    fn odds_have_no_bust_contributors_when_second_chance_is_available() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);

        game.deal_next_card();
        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);

        let odds = game.current_player_odds().expect("odds");

        assert_eq!(odds.bust_probability, 0.0);
        assert_eq!(odds.bust_cards.len(), 1);
        assert_eq!(odds.bust_cards[0].number, 5);
        assert!(odds.bust_cards[0].protected_by_second_chance);
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(5) && detail.score_after_draw == 5)
        );
    }

    #[test]
    fn deal_draws_from_discard_when_draw_pile_is_empty() {
        let mut game = GameState::with_deck(1, Deck::with_draw_and_discard([], [Card::Number(4)]));

        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::DealtNumber { .. }));
        assert_eq!(game.players()[0].hand(), &[Card::Number(4)]);
    }
}
