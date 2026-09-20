use crate::model::{Card, Deck, Player, PlayerId, PlayerStatus};

use super::events::{PendingResolution, PendingStep, SpecialAction};
use super::scoreboard::PlayerScore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UndoMode {
    Enabled,
    #[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UndoHistory {
    snapshots: Vec<LiveGame>,
    mode: UndoMode,
}

impl UndoHistory {
    fn enabled() -> Self {
        Self {
            snapshots: Vec::new(),
            mode: UndoMode::Enabled,
        }
    }

    fn push(&mut self, snapshot: impl FnOnce() -> LiveGame) {
        if self.mode == UndoMode::Enabled {
            self.snapshots.push(snapshot());
        }
    }

    fn pop(&mut self) -> Option<LiveGame> {
        self.snapshots.pop()
    }

    fn len(&self) -> usize {
        self.snapshots.len()
    }

    #[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
    fn disabled() -> Self {
        Self {
            snapshots: Vec::new(),
            mode: UndoMode::Disabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GameState {
    pub(super) live: LiveGame,
    history: UndoHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveGame {
    pub(super) players: Vec<Player>,
    pub(super) deck: Deck,
    pub(super) turn_player_index: usize,
    pub(super) pending_resolution: Option<PendingResolution>,
    pub(super) score_board: Vec<PlayerScore>,
    pub(super) round_number: u32,
    pub(super) round_start_player_index: usize,
    pub(super) round_over: bool,
    pub(super) flip_seven_player_id: Option<PlayerId>,
}

impl GameState {
    pub(crate) fn new(player_count: usize) -> Self {
        Self::with_deck(player_count, Deck::new_full_shuffled())
    }

    pub(crate) fn with_deck(player_count: usize, deck: Deck) -> Self {
        let players = (0..player_count)
            .map(|index| Player::new(PlayerId::new(index)))
            .collect::<Vec<_>>();
        let score_board = players.iter().map(PlayerScore::new).collect();
        Self {
            live: LiveGame {
                players,
                deck,
                turn_player_index: 0,
                pending_resolution: None,
                score_board,
                round_number: 1,
                round_start_player_index: 0,
                round_over: false,
                flip_seven_player_id: None,
            },
            history: UndoHistory::enabled(),
        }
    }

    pub(crate) fn players(&self) -> &[Player] {
        &self.live.players
    }

    pub(crate) fn score_board(&self) -> &[PlayerScore] {
        &self.live.score_board
    }

    pub(crate) fn player_score(&self, player_id: PlayerId) -> Option<&PlayerScore> {
        self.live
            .score_board
            .iter()
            .find(|score| score.player_id == player_id)
    }

    pub(crate) fn round_number(&self) -> u32 {
        self.live.round_number
    }

    pub(crate) fn round_over(&self) -> bool {
        self.live.round_over
    }

    pub(crate) fn current_player_index(&self) -> Option<usize> {
        if self.live.players.is_empty() {
            return None;
        }
        self.pending_action()
            .map(|step| {
                step.target_player_id()
                    .unwrap_or_else(|| step.source_player_id())
            })
            .and_then(|player_id| self.player_index(player_id))
            .or(Some(self.live.turn_player_index))
    }

    pub(crate) fn current_player(&self) -> Option<&Player> {
        self.current_player_index()
            .and_then(|index| self.live.players.get(index))
    }

    #[cfg(all(test, feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn legal_active_targets(&self) -> Vec<PlayerId> {
        self.live
            .players
            .iter()
            .filter(|player| player.is_active_in_round())
            .map(Player::id)
            .collect()
    }

    pub(crate) fn legal_pending_targets(&self) -> Vec<PlayerId> {
        let Some((action, source_player_id, _)) =
            self.pending_action().and_then(PendingStep::target_choice)
        else {
            return Vec::new();
        };
        self.live
            .players
            .iter()
            .filter(|player| {
                player.is_active_in_round()
                    && (action != SpecialAction::SecondChance
                        || (player.id() != source_player_id && !player.has_second_chance()))
            })
            .map(Player::id)
            .collect()
    }

    pub(crate) fn pending_action(&self) -> Option<PendingStep> {
        self.live
            .pending_resolution
            .as_ref()
            .and_then(PendingResolution::front)
    }

    pub(crate) fn queued_action_count(&self) -> usize {
        self.live
            .pending_resolution
            .as_ref()
            .map_or(0, PendingResolution::queued_target_count)
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.history.len() > 0
    }

    #[cfg(test)]
    pub(crate) fn undo_depth(&self) -> usize {
        self.history.len()
    }

    pub(crate) fn draw_pile_count(&self) -> usize {
        self.live.deck.draw_pile_count()
    }

    pub(crate) fn discard_pile_count(&self) -> usize {
        self.live.deck.discard_pile_count()
    }

    pub(crate) fn total_available_cards(&self) -> usize {
        self.live.deck.total_available_count()
    }

    pub(crate) fn draw_pile_counts(&self) -> Vec<(Card, usize)> {
        self.live.deck.draw_pile_counts()
    }

    pub(crate) fn discard_pile_counts(&self) -> Vec<(Card, usize)> {
        self.live.deck.discard_pile_counts()
    }

    pub(crate) fn next_draw_pool_counts(&self) -> Vec<(Card, usize)> {
        self.live.deck.next_draw_pool_counts()
    }

    pub(super) fn save_snapshot(&mut self) {
        self.history.push(|| self.live.clone());
    }

    pub(super) fn player_index(&self, player_id: PlayerId) -> Option<usize> {
        self.live
            .players
            .iter()
            .position(|player| player.id() == player_id)
    }

    pub(super) fn complete_pending_if_empty(&mut self) {
        let resume_after = self
            .live
            .pending_resolution
            .as_ref()
            .filter(|pending| pending.is_complete())
            .map(PendingResolution::resume_after_player_id);
        if let Some(player_id) = resume_after {
            self.live.pending_resolution = None;
            self.advance_to_next_active_player_after(player_id);
        }
    }

    pub(super) fn advance_to_next_active_player_after(&mut self, player_id: PlayerId) {
        let Some(source_index) = self.player_index(player_id) else {
            self.live.turn_player_index = 0;
            return;
        };
        self.advance_to_next_active_player_from((source_index + 1) % self.live.players.len());
    }

    pub(super) fn advance_to_next_active_player_from(&mut self, start_index: usize) {
        self.live.turn_player_index = self
            .next_active_player_index_from(start_index)
            .unwrap_or(start_index.min(self.live.players.len().saturating_sub(1)));
    }

    pub(super) fn next_active_player_index_from(&self, start_index: usize) -> Option<usize> {
        if self.live.players.is_empty() {
            return None;
        }
        (0..self.live.players.len())
            .map(|offset| (start_index + offset) % self.live.players.len())
            .find(|index| self.live.players[*index].status() == PlayerStatus::Active)
    }

    pub(super) fn no_active_players(&self) -> bool {
        self.live
            .players
            .iter()
            .all(|player| !player.is_active_in_round())
    }

    pub(super) fn pop_undo(&mut self) -> Option<LiveGame> {
        self.history.pop()
    }

    #[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn clone_without_undo(&self) -> Self {
        Self {
            live: self.live.clone(),
            history: UndoHistory::disabled(),
        }
    }

    #[cfg(all(test, feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn replace_deck_for_test(&mut self, deck: Deck) {
        self.live.deck = deck;
    }

    #[cfg(all(test, feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn set_total_score_for_test(&mut self, player_id: PlayerId, total_score: u32) {
        if let Some(score) = self
            .live
            .score_board
            .iter_mut()
            .find(|score| score.player_id == player_id)
        {
            score.total_score = total_score;
        }
    }

    #[cfg(all(test, feature = "simulation", not(target_arch = "wasm32")))]
    pub(crate) fn force_bust_for_test(&mut self, player_id: PlayerId) {
        if let Some(index) = self.player_index(player_id) {
            self.live.players[index].bust();
        }
    }
}

#[cfg(all(test, feature = "simulation", not(target_arch = "wasm32")))]
mod tests {
    use super::{GameState, UndoHistory};
    use crate::model::{Card, Deck, GameError};

    #[test]
    fn disabled_undo_does_not_construct_snapshots() {
        let mut history = UndoHistory::disabled();
        history.push(|| panic!("disabled undo must not copy game state"));
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn replay_clone_leaves_live_history_and_state_intact() {
        let mut live =
            GameState::with_deck(1, Deck::from_draw_order([Card::Number(5), Card::Number(7)]));
        live.deal_selected_card(Card::Number(5)).unwrap();
        let before = live.clone();
        let mut replay = live.clone_without_undo();
        assert_eq!(replay.live, live.live);
        assert_eq!(replay.undo_depth(), 0);
        replay.deal_selected_card(Card::Number(7)).unwrap();
        assert_eq!(replay.undo_depth(), 0);
        assert_eq!(replay.undo(), Err(GameError::NothingToUndo));
        assert_eq!(live, before);
        live.undo().unwrap();
        assert!(live.players()[0].hand().is_empty());
    }
}
