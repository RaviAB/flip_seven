use crate::model::{Card, Deck, Player, PlayerId, PlayerStatus};

use super::events::PendingAction;
use super::scoreboard::PlayerScore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UndoTracking {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub(super) players: Vec<Player>,
    pub(super) deck: Deck,
    pub(super) current_player_index: usize,
    pub(super) pending_action: Option<PendingAction>,
    pub(super) queued_actions: Vec<PendingAction>,
    pub(super) score_board: Vec<PlayerScore>,
    pub(super) round_number: u32,
    pub(super) round_start_player_index: usize,
    pub(super) round_over: bool,
    pub(super) flip_seven_player_id: Option<PlayerId>,
    pub(super) history: Vec<GameSnapshot>,
    pub(super) undo_tracking: UndoTracking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GameSnapshot {
    pub(super) players: Vec<Player>,
    pub(super) deck: Deck,
    pub(super) current_player_index: usize,
    pub(super) pending_action: Option<PendingAction>,
    pub(super) queued_actions: Vec<PendingAction>,
    pub(super) score_board: Vec<PlayerScore>,
    pub(super) round_number: u32,
    pub(super) round_start_player_index: usize,
    pub(super) round_over: bool,
    pub(super) flip_seven_player_id: Option<PlayerId>,
}

impl GameState {
    pub fn new(player_count: usize) -> Self {
        Self::with_deck(player_count, Deck::new_full_shuffled())
    }

    pub fn with_deck(player_count: usize, deck: Deck) -> Self {
        let players = (0..player_count)
            .map(|index| Player::new(PlayerId::new(index)))
            .collect::<Vec<_>>();
        let score_board = players.iter().map(PlayerScore::new).collect();

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
            undo_tracking: UndoTracking::Enabled,
        }
    }

    pub fn reset(&mut self, player_count: usize) {
        self.save_snapshot();
        *self = Self {
            history: std::mem::take(&mut self.history),
            undo_tracking: UndoTracking::Enabled,
            ..Self::new(player_count)
        };
    }
}

impl GameState {
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

    pub fn legal_active_targets(&self) -> Vec<PlayerId> {
        self.players
            .iter()
            .filter(|player| player.is_active_in_round())
            .map(Player::id)
            .collect()
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

    pub(crate) fn disable_undo_tracking(&mut self) {
        self.undo_tracking = UndoTracking::Disabled;
        self.history.clear();
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

    pub(super) fn save_snapshot(&mut self) {
        if self.undo_tracking == UndoTracking::Enabled {
            self.history.push(self.snapshot());
        }
    }

    pub(super) fn push_snapshot(&mut self, snapshot: GameSnapshot) {
        if self.undo_tracking == UndoTracking::Enabled {
            self.history.push(snapshot);
        }
    }

    pub(super) fn snapshot(&self) -> GameSnapshot {
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

    pub(super) fn player_index(&self, player_id: PlayerId) -> Option<usize> {
        self.players
            .iter()
            .position(|player| player.id() == player_id)
    }

    pub(super) fn advance_to_next_active_player_after(&mut self, player_id: PlayerId) {
        let Some(source_index) = self.player_index(player_id) else {
            self.current_player_index = 0;
            return;
        };

        self.advance_to_next_active_player_from((source_index + 1) % self.players.len());
    }

    pub(super) fn advance_to_next_active_player_from(&mut self, start_index: usize) {
        self.current_player_index = self
            .next_active_player_index_from(start_index)
            .unwrap_or(start_index.min(self.players.len().saturating_sub(1)));
    }

    pub(super) fn next_active_player_index_from(&self, start_index: usize) -> Option<usize> {
        if self.players.is_empty() {
            return None;
        }

        (0..self.players.len())
            .map(|offset| (start_index + offset) % self.players.len())
            .find(|index| self.players[*index].status() == PlayerStatus::Active)
    }

    pub(super) fn no_active_players(&self) -> bool {
        self.players
            .iter()
            .all(|player| !player.is_active_in_round())
    }
}
