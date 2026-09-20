use crate::model::{Card, PlayerStatus, ScoreBonus};

use super::events::{
    CardEffect, DealSource, GameError, GameEvent, GameResult, PendingResolution, PendingStep,
    ResolutionTiming, RoundEndReason, SpecialAction,
};
use super::state::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardApplication {
    Number,
    Bonus,
    SecondChance,
    DiscardedSecondChance,
    UsedSecondChance,
    Busted,
    TargetAction {
        action: SpecialAction,
        source_player_id: crate::model::PlayerId,
    },
}

impl GameState {
    pub(super) fn deal_selected_flip_three_card(&mut self, card: Card) -> GameResult {
        let pending = self
            .pending_action()
            .expect("caller checked pending action");
        let Some((source_player_id, target_player_id, remaining_draws, queue_checkpoint)) =
            pending.flip_three_state()
        else {
            return Err(GameError::PendingTarget {
                action: pending.action(),
            });
        };
        let Some(target_index) = self.player_index(target_player_id) else {
            return Err(GameError::InvalidTarget);
        };
        if !self.live.players[target_index].is_active_in_round() {
            return Err(GameError::InvalidTarget);
        }
        if !self
            .live
            .deck
            .next_draw_pool_counts()
            .iter()
            .any(|(candidate, _)| *candidate == card)
        {
            return Err(GameError::CardUnavailable { card });
        }

        self.save_snapshot();
        self.record_pre_draw_telemetry(target_index);
        let Some(card) = self.live.deck.draw_selected(card) else {
            return Err(GameError::CardUnavailable { card });
        };
        self.live
            .pending_resolution
            .as_mut()
            .expect("pending draw requires a resolution")
            .pop_front();
        let application = self.apply_card_to_player_index(target_index, card);

        if self.live.players[target_index].has_flip_seven() {
            self.live.flip_seven_player_id = Some(target_player_id);
            return Ok(self.end_round(RoundEndReason::FlipSeven {
                player_id: target_player_id,
            }));
        }

        let remaining_draws = remaining_draws.saturating_sub(1);
        let target_still_active = self.live.players[target_index].is_active_in_round();
        let resolution = self
            .live
            .pending_resolution
            .as_mut()
            .expect("pending draw requires a resolution");
        if !target_still_active {
            resolution.cancel_after(queue_checkpoint);
        } else if remaining_draws > 0 {
            resolution.push_immediate(PendingStep::FlipThreeDraws {
                source_player_id,
                target_player_id,
                remaining_draws,
                queue_checkpoint,
            });
        }

        let (effect, target_action) = self.effect_for_application(application, card);
        if let Some((action, action_source)) = target_action {
            let timing = if action == SpecialAction::SecondChance {
                ResolutionTiming::Immediate
            } else {
                ResolutionTiming::Deferred
            };
            let step = PendingStep::ChooseTarget {
                action,
                source_player_id: action_source,
                timing,
            };
            let resolution = self
                .live
                .pending_resolution
                .as_mut()
                .expect("pending draw requires a resolution");
            match timing {
                ResolutionTiming::Immediate => resolution.push_immediate(step),
                ResolutionTiming::Deferred => resolution.push_deferred(step),
            }
        }

        if self.no_active_players() {
            return Ok(self.end_round(RoundEndReason::AllPlayersDone));
        }
        self.complete_pending_if_empty();
        Ok(GameEvent::CardDealt {
            player_id: target_player_id,
            card,
            source: DealSource::Selected,
            effect,
            remaining_flip_three_draws: Some(if target_still_active {
                remaining_draws
            } else {
                0
            }),
        })
    }

    pub(super) fn apply_card_to_current_player(
        &mut self,
        card: Card,
        source: DealSource,
    ) -> GameEvent {
        let player_index = self.live.turn_player_index;
        let player_id = self.live.players[player_index].id();
        let application = self.apply_card_to_player_index(player_index, card);

        if application == CardApplication::Number
            && self.live.players[player_index].has_flip_seven()
        {
            self.live.flip_seven_player_id = Some(player_id);
            return self.end_round(RoundEndReason::FlipSeven { player_id });
        }

        let (effect, target_action) = self.effect_for_application(application, card);
        if let Some((action, action_source)) = target_action {
            let timing = ResolutionTiming::Immediate;
            let mut resolution = PendingResolution::new(player_id);
            resolution.push_deferred(PendingStep::ChooseTarget {
                action,
                source_player_id: action_source,
                timing,
            });
            self.live.pending_resolution = Some(resolution);
            return GameEvent::TargetRequired {
                action,
                source_player_id: action_source,
                timing,
            };
        }

        self.advance_to_next_active_player_after(player_id);
        if application == CardApplication::Busted && self.no_active_players() {
            return self.end_round(RoundEndReason::AllPlayersDone);
        }
        GameEvent::CardDealt {
            player_id,
            card,
            source,
            effect,
            remaining_flip_three_draws: None,
        }
    }

    fn effect_for_application(
        &self,
        application: CardApplication,
        card: Card,
    ) -> (CardEffect, Option<(SpecialAction, crate::model::PlayerId)>) {
        match application {
            CardApplication::Number => (CardEffect::NumberAdded, None),
            CardApplication::Bonus => (CardEffect::BonusAdded, None),
            CardApplication::SecondChance => (CardEffect::SecondChanceKept, None),
            CardApplication::DiscardedSecondChance => (CardEffect::SecondChanceDiscarded, None),
            CardApplication::UsedSecondChance => {
                (CardEffect::SecondChanceUsed { duplicate: card }, None)
            }
            CardApplication::Busted => (CardEffect::Busted { duplicate: card }, None),
            CardApplication::TargetAction {
                action,
                source_player_id,
            } => (
                CardEffect::SpecialQueued {
                    action,
                    timing: if action == SpecialAction::SecondChance {
                        ResolutionTiming::Immediate
                    } else {
                        ResolutionTiming::Deferred
                    },
                },
                Some((action, source_player_id)),
            ),
        }
    }

    fn apply_card_to_player_index(&mut self, player_index: usize, card: Card) -> CardApplication {
        match card {
            Card::Number(value) => {
                if self.live.players[player_index].has_number(value) {
                    self.live.deck.discard(card);
                    if self.live.players[player_index].use_second_chance() {
                        self.live.deck.discard(Card::SecondChance);
                        CardApplication::UsedSecondChance
                    } else {
                        self.finish_player(player_index, PlayerStatus::Busted, ScoreBonus::None);
                        CardApplication::Busted
                    }
                } else {
                    self.live.players[player_index].receive_card(card);
                    CardApplication::Number
                }
            }
            Card::Bonus(_) => {
                self.live.players[player_index].receive_card(card);
                CardApplication::Bonus
            }
            Card::SecondChance => {
                if !self.live.players[player_index].has_second_chance() {
                    self.live.players[player_index].receive_card(card);
                    CardApplication::SecondChance
                } else {
                    let source_player_id = self.live.players[player_index].id();
                    let has_recipient = self.live.players.iter().any(|player| {
                        player.is_active_in_round()
                            && player.id() != source_player_id
                            && !player.has_second_chance()
                    });
                    if has_recipient {
                        CardApplication::TargetAction {
                            action: SpecialAction::SecondChance,
                            source_player_id,
                        }
                    } else {
                        self.live.deck.discard(card);
                        CardApplication::DiscardedSecondChance
                    }
                }
            }
            Card::FlipThree | Card::Freeze => {
                self.live.deck.discard(card);
                CardApplication::TargetAction {
                    action: if card == Card::FlipThree {
                        SpecialAction::FlipThree
                    } else {
                        SpecialAction::Freeze
                    },
                    source_player_id: self.live.players[player_index].id(),
                }
            }
        }
    }
}
