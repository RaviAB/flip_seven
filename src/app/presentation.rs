use crate::model::{
    BustCardDetail, Card, CardEffect, DrawOdds, GameError, GameEvent, GameResult, GameState,
    Player, PlayerScore, ResolutionTiming, RoundEndReason, RoundOutcome, RoundScore, ScoreBonus,
    ScoreBreakdown, SpecialAction,
};

pub(super) fn special_action_label(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::SecondChance => "Second Chance",
        SpecialAction::FlipThree => "Flip Three",
        SpecialAction::Freeze => "Freeze",
    }
}

pub(super) fn status_for_result(result: GameResult, game: &GameState) -> String {
    match result {
        Ok(GameEvent::CardDealt {
            player_id,
            card,
            effect,
            remaining_flip_three_draws,
            ..
        }) => {
            let player = player_name(game, player_id);
            let prefix = if remaining_flip_three_draws.is_some() {
                format!("Flip Three dealt {card} to {player}.")
            } else {
                match effect {
                    CardEffect::BonusAdded => format!("{player} received bonus {card}."),
                    _ => format!("{player} received {card}."),
                }
            };
            let effect_note = match effect {
                CardEffect::SecondChanceDiscarded => {
                    " No active player could receive the duplicate Second Chance."
                }
                CardEffect::SecondChanceUsed { .. } => " Second Chance was used.",
                CardEffect::Busted { .. } => " The player busted.",
                CardEffect::SpecialQueued { action, timing } => match timing {
                    ResolutionTiming::Immediate => match action {
                        SpecialAction::SecondChance => " Choose a recipient before continuing.",
                        _ => " Choose a target.",
                    },
                    ResolutionTiming::Deferred => " The special action is queued.",
                },
                _ => "",
            };
            let remaining = match remaining_flip_three_draws {
                Some(0) => " Flip Three complete.".to_owned(),
                Some(count) => format!(
                    " Choose {count} more card{}.",
                    if count == 1 { "" } else { "s" }
                ),
                None => String::new(),
            };
            format!("{prefix}{effect_note}{remaining}")
        }
        Ok(GameEvent::Stayed { player_id }) => {
            format!("{} stayed.", player_name(game, player_id))
        }
        Ok(GameEvent::TargetRequired {
            action,
            source_player_id,
            ..
        }) => format!(
            "{} drew {}; choose an active player.",
            player_name(game, source_player_id),
            special_action_label(action)
        ),
        Ok(GameEvent::TargetResolved {
            action,
            target_player_id,
        }) => format!(
            "{} resolved for {}.",
            special_action_label(action),
            player_name(game, target_player_id)
        ),
        Ok(GameEvent::FlipThreeStarted {
            target_player_id,
            remaining_draws,
        }) => format!(
            "Flip Three targeted {}. Choose {remaining_draws} cards from Deal card.",
            player_name(game, target_player_id)
        ),
        Ok(GameEvent::RoundEnded { reason }) => match reason {
            RoundEndReason::FlipSeven { player_id } => {
                format!("Round ended. {} hit Flip 7.", player_name(game, player_id))
            }
            RoundEndReason::AllPlayersDone => {
                "Round ended. All players are done for the round.".to_owned()
            }
            RoundEndReason::NoActivePlayers => "Round ended. No active players remain.".to_owned(),
        },
        Ok(GameEvent::RoundStarted { .. }) => "Started the next round.".to_owned(),
        Ok(GameEvent::Reset { player_count }) => format!(
            "Reset for {player_count} player{}.",
            if player_count == 1 { "" } else { "s" }
        ),
        Ok(GameEvent::Reshuffled { cards_moved }) => format!(
            "Reshuffled {cards_moved} discard card{} into the draw pile.",
            if cards_moved == 1 { "" } else { "s" }
        ),
        Ok(GameEvent::UndoApplied) => "Undid the last action.".to_owned(),
        Err(GameError::RoundInProgress) => "Finish this round before starting another.".to_owned(),
        Err(GameError::RoundOver) => "Round over. Start the next round to continue.".to_owned(),
        Err(GameError::PendingTarget { action }) => format!(
            "Resolve {} before continuing.",
            special_action_label(action)
        ),
        Err(GameError::InvalidTarget) => "Choose an eligible active player.".to_owned(),
        Err(GameError::CardUnavailable { .. }) => {
            "That card is not available in the current draw pool.".to_owned()
        }
        Err(GameError::EmptyDeck) => "No cards are available to draw.".to_owned(),
        Err(GameError::NoPlayers) => "Add at least one player before dealing.".to_owned(),
        Err(GameError::NoDiscard) => "There are no discard cards to reshuffle.".to_owned(),
        Err(GameError::NothingToUndo) => "Nothing to undo.".to_owned(),
    }
}

fn player_name(game: &GameState, player_id: crate::model::PlayerId) -> &str {
    game.players()
        .iter()
        .find(|player| player.id() == player_id)
        .map_or("Player", |player| player.name())
}

pub(super) fn queued_action_suffix(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!(
            " {count} queued special action{} after this.",
            if count == 1 { "" } else { "s" }
        )
    }
}

pub(super) fn score_summary_label(score: &PlayerScore) -> String {
    let avg_bust = score
        .average_bust_risk_basis_points()
        .map(format_basis_points)
        .unwrap_or_else(|| "-".to_owned());
    let current = score
        .current_round_score
        .map(|score| format!(" | Bank {score}"))
        .unwrap_or_default();

    format!(
        "{} | T {} | L {} | R {} | Bust {}{}",
        score.player_name,
        score.total_score,
        score.last_round_score,
        score.rounds_completed,
        avg_bust,
        current
    )
}

pub(super) fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

pub(super) fn round_outcome_label(outcome: RoundOutcome) -> &'static str {
    match outcome {
        RoundOutcome::Stayed => "Stayed",
        RoundOutcome::Frozen => "Frozen",
        RoundOutcome::Busted => "Busted",
        RoundOutcome::FlipSeven => "Flip 7",
        RoundOutcome::RoundEndedActive => "Round end",
    }
}

pub(super) fn display_round_score(player: &Player, score: Option<&PlayerScore>) -> u32 {
    score
        .and_then(|score| score.current_round_score)
        .unwrap_or_else(|| player.round_score(ScoreBonus::None))
}

pub(super) fn player_score_tooltip(player: &Player, score: Option<&PlayerScore>) -> String {
    let mut lines = Vec::new();
    if let Some(score) = score.and_then(|score| score.current_round_score) {
        lines.push(format!("Banked this round: {score}"));
        lines.push(
            "Hand has already moved to discard because this player is no longer active.".to_owned(),
        );
    } else {
        lines.push(score_breakdown_tooltip(
            &player.score_breakdown(ScoreBonus::None),
        ));
    }
    lines.push(format!(
        "Distinct number cards: {}",
        player.distinct_number_count()
    ));
    lines.push(format!("Cards currently in hand: {}", player.hand().len()));
    lines.join("\n")
}

pub(super) fn score_breakdown_tooltip(breakdown: &ScoreBreakdown) -> String {
    format!(
        "Numbers: {}\nMultiplier: x{}\nBonuses: +{}\nFlip 7 bonus: +{}\nTotal: {}",
        breakdown.number_sum,
        breakdown.multiplier,
        breakdown.additive_bonus,
        breakdown.flip_seven_bonus,
        breakdown.total
    )
}

pub(super) fn protected_duplicate_probability(details: &[BustCardDetail]) -> f64 {
    details
        .iter()
        .filter(|detail| detail.protected_by_second_chance)
        .map(|detail| detail.probability)
        .sum()
}

pub(super) fn bust_probability_label(odds: &DrawOdds) -> String {
    let protected = protected_duplicate_probability(&odds.bust_cards);
    if protected > 0.0 {
        format!(
            "{} (SC {})",
            format_percent(odds.bust_probability),
            format_percent(protected)
        )
    } else {
        format_percent(odds.bust_probability)
    }
}

pub(super) fn score_history_tooltip(score: &PlayerScore) -> String {
    let mut lines = vec![
        format!("Total: {}", score.total_score),
        format!("Last completed round: {}", score.last_round_score),
        format!(
            "Average round: {}",
            score
                .average_round_score()
                .map(|value| format!("{value:.1}"))
                .unwrap_or_else(|| "-".to_owned())
        ),
        format!(
            "Best / worst: {} / {}",
            optional_u32(score.best_round_score()),
            optional_u32(score.worst_round_score())
        ),
    ];

    if let Some(current_round_score) = score.current_round_score {
        lines.push(format!("Banked current round: {current_round_score}"));
    }

    if score.round_scores.is_empty() {
        lines.push("No completed round history yet.".to_owned());
    } else {
        lines.push("Completed rounds:".to_owned());
        for round_score in &score.round_scores {
            lines.push(format!(
                "Round {}: {}",
                round_score.round_number, round_score.score
            ));
        }
    }

    lines.join("\n")
}

pub(super) fn score_bust_risk_tooltip(score: &PlayerScore) -> String {
    if score.telemetry.bust_risk_samples == 0 {
        return "No dealt cards have produced a pre-draw risk sample yet.".to_owned();
    }

    format!(
        "Average effective pre-draw bust risk across {} dealt cards.\nPeak risk: {}\nSecond Chance protected duplicate risk on {} sample{}.",
        score.telemetry.bust_risk_samples,
        format_basis_points(score.telemetry.peak_bust_risk_basis_points),
        score.telemetry.second_chance_protected_samples,
        if score.telemetry.second_chance_protected_samples == 1 {
            ""
        } else {
            "s"
        }
    )
}

pub(super) fn round_bust_risk_tooltip(round: &RoundScore) -> String {
    if round.bust_risk_samples == 0 {
        return "No dealt cards produced a pre-draw risk sample in this round.".to_owned();
    }

    format!(
        "Average effective pre-draw bust risk across {} dealt card{} this round.\nPeak risk: {}\nSecond Chance protected duplicate risk on {} sample{}.",
        round.bust_risk_samples,
        if round.bust_risk_samples == 1 {
            ""
        } else {
            "s"
        },
        format_basis_points(round.peak_bust_risk_basis_points),
        round.second_chance_protected_samples,
        if round.second_chance_protected_samples == 1 {
            ""
        } else {
            "s"
        }
    )
}

pub(super) fn deck_tooltip(game: &GameState) -> String {
    format!(
        "Click to inspect deck contents.\nDraw pile: {}\nDiscard pile: {}\nAvailable cards: {}\nWhen the draw pile empties, the discard pile is shuffled back into the deck.",
        game.draw_pile_count(),
        game.discard_pile_count(),
        game.total_available_cards()
    )
}

pub(super) fn card_tooltip(card: Card) -> String {
    match card {
        Card::Number(value) => format!(
            "Number card {value}\nAdds {value} points.\nDrawing a duplicate number busts unless the player has Second Chance."
        ),
        Card::Bonus(crate::model::BonusCard::Plus(value)) => {
            format!("Bonus card +{value}\nAdds {value} points when the player scores.")
        }
        Card::Bonus(crate::model::BonusCard::Double) => {
            "Bonus card x2\nDoubles the sum of number cards before additive bonuses.".to_owned()
        }
        Card::SecondChance => {
            "Second Chance\nPrevents one duplicate-number bust, then moves to discard. A second copy must be given to another eligible active player.".to_owned()
        }
        Card::FlipThree => {
            "Flip Three\nChoose an active player; they draw up to three cards.".to_owned()
        }
        Card::Freeze => {
            "Freeze\nChoose an active player; they bank their current score and their hand moves to discard.".to_owned()
        }
    }
}

pub(super) fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

pub(super) fn format_basis_points(value: u32) -> String {
    format!("{:.1}%", value as f64 / 100.0)
}
