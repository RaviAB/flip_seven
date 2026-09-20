use crate::model::{Card, PlayerId, PlayerStatus};

use super::events::{
    DealSource, GameError, GameEvent, GameResult, PendingStep, RoundEndReason, SpecialAction,
};
use super::state::GameState;
use crate::model::ScoreBonus;

impl GameState {
    pub(crate) fn reset(&mut self, player_count: usize) -> GameResult {
        self.save_snapshot();
        let replacement = Self::new(player_count);
        self.live = replacement.live;
        Ok(GameEvent::Reset { player_count })
    }

    pub(crate) fn start_next_round(&mut self) -> GameResult {
        if !self.live.round_over {
            return Err(GameError::RoundInProgress);
        }
        self.save_snapshot();
        let discarded = self
            .live
            .players
            .iter_mut()
            .flat_map(|player| player.reset_round())
            .collect::<Vec<_>>();
        self.live.deck.discard_many(discarded);
        self.live.pending_resolution = None;
        self.live.flip_seven_player_id = None;
        self.live.round_over = false;
        self.live.round_number += 1;
        self.reset_current_round_scoreboard();
        if !self.live.players.is_empty() {
            self.live.round_start_player_index =
                (self.live.round_start_player_index + 1) % self.live.players.len();
        }
        self.live.turn_player_index = self.live.round_start_player_index;
        self.advance_to_next_active_player_from(self.live.round_start_player_index);
        Ok(GameEvent::RoundStarted {
            round_number: self.live.round_number,
        })
    }

    pub(crate) fn reshuffle_discard(&mut self) -> GameResult {
        if self.live.deck.discard_pile_count() == 0 {
            return Err(GameError::NoDiscard);
        }
        self.save_snapshot();
        let cards_moved = self.live.deck.reshuffle_discard_into_draw_pile();
        Ok(GameEvent::Reshuffled { cards_moved })
    }

    #[allow(dead_code)]
    pub(crate) fn deal_next_card(&mut self) -> GameResult {
        self.deal_card(None, DealSource::DrawPile)
    }

    pub(crate) fn deal_selected_card(&mut self, card: Card) -> GameResult {
        self.deal_card(Some(card), DealSource::Selected)
    }

    fn deal_card(&mut self, selected: Option<Card>, source: DealSource) -> GameResult {
        if self.live.round_over {
            return Err(GameError::RoundOver);
        }
        if let Some(pending) = self.pending_action() {
            if pending.awaiting_selected_draws()
                && let Some(card) = selected
            {
                return self.deal_selected_flip_three_card(card);
            }
            return Err(GameError::PendingTarget {
                action: pending.action(),
            });
        }
        if self.live.players.is_empty() {
            return Err(GameError::NoPlayers);
        }
        let Some(player_index) = self.next_active_player_index_from(self.live.turn_player_index)
        else {
            return Ok(self.end_round(RoundEndReason::NoActivePlayers));
        };
        if let Some(card) = selected {
            if !self
                .live
                .deck
                .next_draw_pool_counts()
                .iter()
                .any(|(candidate, _)| *candidate == card)
            {
                return Err(GameError::CardUnavailable { card });
            }
        } else if self.live.deck.next_draw_pool_counts().is_empty() {
            return Err(GameError::EmptyDeck);
        }

        self.save_snapshot();
        self.live.turn_player_index = player_index;
        self.record_pre_draw_telemetry(player_index);
        let card = match selected {
            Some(card) => self.live.deck.draw_selected(card),
            None => self.live.deck.draw(),
        }
        .ok_or_else(|| {
            selected.map_or(GameError::EmptyDeck, |card| GameError::CardUnavailable {
                card,
            })
        })?;
        Ok(self.apply_card_to_current_player(card, source))
    }

    pub(crate) fn stay_current_player(&mut self) -> GameResult {
        if self.live.round_over {
            return Err(GameError::RoundOver);
        }
        if let Some(pending) = self.pending_action() {
            return Err(GameError::PendingTarget {
                action: pending.action(),
            });
        }
        let Some(player_index) = self.next_active_player_index_from(self.live.turn_player_index)
        else {
            return Ok(self.end_round(RoundEndReason::NoActivePlayers));
        };

        self.save_snapshot();
        self.live.turn_player_index = player_index;
        let player_id = self.live.players[player_index].id();
        self.finish_player(player_index, PlayerStatus::Stayed, ScoreBonus::None);
        self.advance_to_next_active_player_after(player_id);
        if self.no_active_players() {
            Ok(self.end_round(RoundEndReason::AllPlayersDone))
        } else {
            Ok(GameEvent::Stayed { player_id })
        }
    }

    pub(crate) fn choose_target(&mut self, target_player_id: PlayerId) -> GameResult {
        if self.live.round_over {
            return Err(GameError::RoundOver);
        }
        let Some(pending) = self.pending_action() else {
            return Err(GameError::InvalidTarget);
        };
        let Some((action, source_player_id, _)) = pending.target_choice() else {
            return Err(GameError::InvalidTarget);
        };
        let Some(target_index) = self.player_index(target_player_id) else {
            return Err(GameError::InvalidTarget);
        };
        if !self.legal_pending_targets().contains(&target_player_id) {
            return Err(GameError::InvalidTarget);
        }

        self.save_snapshot();
        let resolution = self
            .live
            .pending_resolution
            .as_mut()
            .expect("pending step requires a resolution");
        resolution.pop_front();
        match action {
            SpecialAction::SecondChance => {
                self.live.players[target_index].receive_card(Card::SecondChance);
                self.complete_pending_if_empty();
                if self.no_active_players() {
                    return Ok(self.end_round(RoundEndReason::AllPlayersDone));
                }
                Ok(GameEvent::TargetResolved {
                    action,
                    target_player_id,
                })
            }
            SpecialAction::Freeze => {
                self.finish_player(target_index, PlayerStatus::Frozen, ScoreBonus::None);
                self.complete_pending_if_empty();
                if self.no_active_players() {
                    return Ok(self.end_round(RoundEndReason::AllPlayersDone));
                }
                Ok(GameEvent::TargetResolved {
                    action,
                    target_player_id,
                })
            }
            SpecialAction::FlipThree => {
                let checkpoint = resolution.checkpoint();
                resolution.push_immediate(PendingStep::FlipThreeDraws {
                    source_player_id,
                    target_player_id,
                    remaining_draws: 3,
                    queue_checkpoint: checkpoint,
                });
                Ok(GameEvent::FlipThreeStarted {
                    target_player_id,
                    remaining_draws: 3,
                })
            }
        }
    }

    pub(crate) fn undo(&mut self) -> GameResult {
        let Some(snapshot) = self.pop_undo() else {
            return Err(GameError::NothingToUndo);
        };
        self.live = snapshot;
        Ok(GameEvent::UndoApplied)
    }
}
