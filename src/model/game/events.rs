use crate::model::{Card, PlayerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialAction {
    FlipThree,
    Freeze,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub(super) action: SpecialAction,
    pub(super) source_player_id: PlayerId,
    pub(super) resume_after_player_id: PlayerId,
    pub(super) target_player_id: Option<PlayerId>,
    pub(super) remaining_draws: u8,
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
