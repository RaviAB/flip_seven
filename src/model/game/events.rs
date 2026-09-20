use std::collections::VecDeque;

use crate::model::{Card, PlayerId};

pub type GameResult = Result<GameEvent, GameError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialAction {
    SecondChance,
    FlipThree,
    Freeze,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DealSource {
    DrawPile,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionTiming {
    Immediate,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardEffect {
    NumberAdded,
    BonusAdded,
    SecondChanceKept,
    SecondChanceDiscarded,
    SecondChanceUsed {
        duplicate: Card,
    },
    Busted {
        duplicate: Card,
    },
    SpecialQueued {
        action: SpecialAction,
        timing: ResolutionTiming,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEndReason {
    AllPlayersDone,
    NoActivePlayers,
    FlipSeven { player_id: PlayerId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    CardDealt {
        player_id: PlayerId,
        card: Card,
        source: DealSource,
        effect: CardEffect,
        remaining_flip_three_draws: Option<u8>,
    },
    Stayed {
        player_id: PlayerId,
    },
    TargetRequired {
        action: SpecialAction,
        source_player_id: PlayerId,
        timing: ResolutionTiming,
    },
    TargetResolved {
        action: SpecialAction,
        target_player_id: PlayerId,
    },
    FlipThreeStarted {
        target_player_id: PlayerId,
        remaining_draws: u8,
    },
    RoundEnded {
        reason: RoundEndReason,
    },
    RoundStarted {
        round_number: u32,
    },
    Reset {
        player_count: usize,
    },
    Reshuffled {
        cards_moved: usize,
    },
    UndoApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    RoundInProgress,
    RoundOver,
    PendingTarget { action: SpecialAction },
    InvalidTarget,
    CardUnavailable { card: Card },
    EmptyDeck,
    NoPlayers,
    NoDiscard,
    NothingToUndo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingStep {
    ChooseTarget {
        action: SpecialAction,
        source_player_id: PlayerId,
        timing: ResolutionTiming,
    },
    FlipThreeDraws {
        source_player_id: PlayerId,
        target_player_id: PlayerId,
        remaining_draws: u8,
        queue_checkpoint: usize,
    },
}

impl PendingStep {
    pub(crate) fn action(self) -> SpecialAction {
        match self {
            Self::ChooseTarget { action, .. } => action,
            Self::FlipThreeDraws { .. } => SpecialAction::FlipThree,
        }
    }

    pub(crate) fn source_player_id(self) -> PlayerId {
        match self {
            Self::ChooseTarget {
                source_player_id, ..
            }
            | Self::FlipThreeDraws {
                source_player_id, ..
            } => source_player_id,
        }
    }

    pub(crate) fn target_player_id(self) -> Option<PlayerId> {
        match self {
            Self::ChooseTarget { .. } => None,
            Self::FlipThreeDraws {
                target_player_id, ..
            } => Some(target_player_id),
        }
    }

    pub(crate) fn remaining_draws(self) -> u8 {
        match self {
            Self::ChooseTarget { .. } => 0,
            Self::FlipThreeDraws {
                remaining_draws, ..
            } => remaining_draws,
        }
    }

    pub(crate) fn needs_target(self) -> bool {
        matches!(self, Self::ChooseTarget { .. })
    }

    pub(crate) fn awaiting_selected_draws(self) -> bool {
        matches!(
            self,
            Self::FlipThreeDraws {
                remaining_draws: 1..,
                ..
            }
        )
    }

    pub(crate) fn target_choice(self) -> Option<(SpecialAction, PlayerId, ResolutionTiming)> {
        match self {
            Self::ChooseTarget {
                action,
                source_player_id,
                timing,
            } => Some((action, source_player_id, timing)),
            Self::FlipThreeDraws { .. } => None,
        }
    }

    pub(crate) fn flip_three_state(self) -> Option<(PlayerId, PlayerId, u8, usize)> {
        match self {
            Self::ChooseTarget { .. } => None,
            Self::FlipThreeDraws {
                source_player_id,
                target_player_id,
                remaining_draws,
                queue_checkpoint,
            } => Some((
                source_player_id,
                target_player_id,
                remaining_draws,
                queue_checkpoint,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingResolution {
    resume_after_player_id: PlayerId,
    steps: VecDeque<PendingStep>,
}

impl PendingResolution {
    pub(crate) fn new(resume_after_player_id: PlayerId) -> Self {
        Self {
            resume_after_player_id,
            steps: VecDeque::new(),
        }
    }

    pub(crate) fn resume_after_player_id(&self) -> PlayerId {
        self.resume_after_player_id
    }

    pub(crate) fn front(&self) -> Option<PendingStep> {
        self.steps.front().copied()
    }

    pub(crate) fn pop_front(&mut self) -> Option<PendingStep> {
        self.steps.pop_front()
    }

    pub(crate) fn push_immediate(&mut self, step: PendingStep) {
        self.steps.push_front(step);
    }

    pub(crate) fn push_deferred(&mut self, step: PendingStep) {
        self.steps.push_back(step);
    }

    pub(crate) fn checkpoint(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn cancel_after(&mut self, checkpoint: usize) {
        self.steps.truncate(checkpoint);
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.steps.is_empty()
    }

    pub(crate) fn queued_target_count(&self) -> usize {
        self.steps
            .iter()
            .skip(1)
            .filter(|step| step.needs_target())
            .count()
    }
}
