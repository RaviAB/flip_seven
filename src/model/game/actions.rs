use crate::model::{Card, PlayerStatus};

use super::card_resolution::FlipSevenBonus;
use super::events::{ActionChoice, DealOutcome, PendingAction, SpecialAction};
use super::state::GameState;

impl GameState {
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
        self.reset_current_round_scoreboard();
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

        self.push_snapshot(snapshot);
        self.current_player_index = player_index;
        self.record_pre_draw_telemetry(player_index);
        let Some(card) = self.deck.draw() else {
            return DealOutcome::DeckEmpty;
        };

        self.apply_card_to_current_player(card)
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

        self.push_snapshot(snapshot);
        self.current_player_index = player_index;
        self.record_pre_draw_telemetry(player_index);
        let Some(card) = self.deck.draw_selected(card) else {
            return DealOutcome::SelectedCardUnavailable;
        };

        self.apply_card_to_current_player(card)
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
        self.finish_player(player_index, PlayerStatus::Stayed, FlipSevenBonus::No);
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
                self.finish_player(target_index, PlayerStatus::Frozen, FlipSevenBonus::No);
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

        self.advance_to_next_active_player_after(pending_action.resume_after_player_id);

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
}
