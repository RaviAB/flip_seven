use crate::model::{Card, PlayerStatus};

use super::events::{DealOutcome, PendingAction, SpecialAction};
use super::state::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SpecialHandling {
    ResolveNow,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlipSevenBonus {
    No,
    Yes,
}

impl FlipSevenBonus {
    pub(super) fn applies(self) -> bool {
        self == Self::Yes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardApplication {
    Number,
    Bonus,
    SecondChance,
    UsedSecondChance,
    Busted,
    NeedsTarget(SpecialAction),
    QueuedSpecial(SpecialAction),
}

impl CardApplication {
    fn note(self) -> Option<String> {
        match self {
            Self::UsedSecondChance => Some("Second Chance was used.".to_owned()),
            Self::Busted => Some("Player busted.".to_owned()),
            Self::QueuedSpecial(action) => Some(format!(
                "Queued {} for later resolution.",
                special_action_label(action)
            )),
            Self::Number | Self::Bonus | Self::SecondChance | Self::NeedsTarget(_) => None,
        }
    }
}

impl GameState {
    pub(super) fn deal_selected_flip_three_card(
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
        let application =
            self.apply_card_to_player_index(target_index, card, SpecialHandling::Queue);
        let mut notes = application.note().into_iter().collect::<Vec<_>>();

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

    pub(super) fn apply_card_to_current_player(&mut self, card: Card) -> DealOutcome {
        let player_index = self.current_player_index;
        let player_id = self.players[player_index].id();
        let player_name = self.players[player_index].name().to_owned();

        match self.apply_card_to_player_index(player_index, card, SpecialHandling::ResolveNow) {
            CardApplication::Number => {
                if self.players[player_index].has_flip_seven() {
                    self.flip_seven_player_id = Some(player_id);
                    return self.end_round(format!("{player_name} hit Flip 7."));
                }

                self.advance_to_next_active_player_after(player_id);
                DealOutcome::DealtNumber {
                    player_id,
                    player_name,
                    card,
                }
            }
            CardApplication::Bonus => {
                self.advance_to_next_active_player_after(player_id);
                DealOutcome::Bonus {
                    player_id,
                    player_name,
                    card,
                }
            }
            CardApplication::SecondChance => {
                self.advance_to_next_active_player_after(player_id);
                DealOutcome::SecondChance {
                    player_id,
                    player_name,
                }
            }
            CardApplication::UsedSecondChance => {
                self.advance_to_next_active_player_after(player_id);
                DealOutcome::UsedSecondChance {
                    player_id,
                    player_name,
                    duplicate: card,
                }
            }
            CardApplication::Busted => {
                self.advance_to_next_active_player_after(player_id);
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
            CardApplication::NeedsTarget(action) => DealOutcome::SpecialNeedsTarget {
                action,
                source_player_id: player_id,
                source_player_name: player_name,
            },
            CardApplication::QueuedSpecial(_) => DealOutcome::DeckEmpty,
        }
    }

    fn apply_card_to_player_index(
        &mut self,
        player_index: usize,
        card: Card,
        special_handling: SpecialHandling,
    ) -> CardApplication {
        match card {
            Card::Number(value) => {
                if self.players[player_index].has_number(value) {
                    self.deck.discard(card);
                    if self.players[player_index].use_second_chance() {
                        self.deck.discard(Card::SecondChance);
                        CardApplication::UsedSecondChance
                    } else {
                        self.finish_player(player_index, PlayerStatus::Busted, FlipSevenBonus::No);
                        CardApplication::Busted
                    }
                } else {
                    self.players[player_index].receive_card(card);
                    CardApplication::Number
                }
            }
            Card::Bonus(_) => {
                self.players[player_index].receive_card(card);
                CardApplication::Bonus
            }
            Card::SecondChance => {
                self.players[player_index].receive_card(card);
                CardApplication::SecondChance
            }
            Card::FlipThree => {
                self.deck.discard(card);
                let action = PendingAction {
                    action: SpecialAction::FlipThree,
                    source_player_id: self.players[player_index].id(),
                    target_player_id: None,
                    remaining_draws: 0,
                };
                if special_handling == SpecialHandling::Queue {
                    self.queued_actions.push(action);
                    CardApplication::QueuedSpecial(SpecialAction::FlipThree)
                } else {
                    self.pending_action = Some(action);
                    CardApplication::NeedsTarget(SpecialAction::FlipThree)
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
                if special_handling == SpecialHandling::Queue {
                    self.queued_actions.push(action);
                    CardApplication::QueuedSpecial(SpecialAction::Freeze)
                } else {
                    self.pending_action = Some(action);
                    CardApplication::NeedsTarget(SpecialAction::Freeze)
                }
            }
        }
    }

    pub(super) fn activate_next_queued_action(&mut self) {
        if self.pending_action.is_none() && !self.queued_actions.is_empty() {
            self.pending_action = Some(self.queued_actions.remove(0));
        }
    }
}

fn special_action_label(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::FlipThree => "Flip Three",
        SpecialAction::Freeze => "Freeze",
    }
}
