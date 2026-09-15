use crate::model::{GameState, PlayerId};

use super::decision::{AiDecision, DecisionContext, DecisionWithRationale};
use super::report::SimulationSettings;

#[derive(Debug, Clone, Copy)]
struct StaticWinWeights {
    score_pressure_weight: f64,
    field_rank_weight: f64,
    endgame_urgency_weight: f64,
    bust_penalty: f64,
    ev_reward: f64,
    flip_seven_reward: f64,
    second_chance_bonus: f64,
    decision_margin: f64,
}

impl StaticWinWeights {
    const CALIBRATED_V1: Self = Self {
        score_pressure_weight: 0.95,
        field_rank_weight: 0.34,
        endgame_urgency_weight: 0.46,
        bust_penalty: 0.48,
        ev_reward: 11.0,
        flip_seven_reward: 1.02,
        second_chance_bonus: 0.40,
        decision_margin: 0.0,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StaticWinFeatures {
    pub(super) player_count: usize,
    pub(super) active_players_remaining: usize,
    pub(super) relative_rank_after_stay: f64,
    pub(super) average_margin_after_stay: f64,
    pub(super) worst_margin_after_stay: f64,
    pub(super) field_pressure: f64,
    pub(super) endgame_urgency: f64,
    pub(super) current_round_score: u32,
    pub(super) current_round_score_ratio: f64,
    pub(super) bust_probability: f64,
    pub(super) effective_bust_probability: f64,
    pub(super) expected_score_delta: f64,
    pub(super) expected_score_delta_ratio: f64,
    pub(super) flip_seven_probability: f64,
    pub(super) flip_seven_distance: usize,
    pub(super) number_card_count: usize,
    pub(super) has_second_chance: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StaticWinEvaluation {
    pub(super) stay_equity: f64,
    pub(super) draw_equity: f64,
    pub(super) decision_margin: f64,
    pub(super) features: StaticWinFeatures,
}

#[derive(Debug, Clone, Copy)]
struct EquityTerm {
    name: &'static str,
    value: f64,
}

pub(super) fn max_win_static_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationSettings,
) -> DecisionWithRationale {
    let Some(odds) = context.draw_odds.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No draw odds are available.".to_owned(),
        };
    };
    let Some(score) = game.player_score(context.player_id) else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No player score is available.".to_owned(),
        };
    };

    let score_to_win = settings.target_score.saturating_sub(score.total_score);
    if odds.current_score > 0 && odds.current_score >= score_to_win {
        return DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: banking {} reaches the {} point target.",
                odds.current_score, settings.target_score
            ),
        };
    }

    if odds.current_score == 0 {
        return DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: "Draw: no points to bank.".to_owned(),
        };
    }

    let Some((evaluation, terms)) = static_win_evaluation_with_terms(game, context, settings)
    else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No static win-equity features are available.".to_owned(),
        };
    };

    let should_draw = evaluation.draw_equity >= evaluation.stay_equity + evaluation.decision_margin;
    let decision = if should_draw {
        AiDecision::Draw
    } else {
        AiDecision::Stay
    };
    let action = if should_draw { "Draw" } else { "Stay" };

    DecisionWithRationale {
        decision: Some(decision),
        rationale: format!(
            "{action}: static equity stay {:.3}, draw {:.3}, margin {:.3}. Top terms: {}.",
            evaluation.stay_equity,
            evaluation.draw_equity,
            evaluation.decision_margin,
            top_terms(&terms)
        ),
    }
}

#[cfg(test)]
pub(super) fn static_win_evaluation(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationSettings,
) -> Option<StaticWinEvaluation> {
    static_win_evaluation_with_terms(game, context, settings).map(|(evaluation, _)| evaluation)
}

pub(super) fn static_win_features(
    game: &GameState,
    player_id: PlayerId,
    settings: &SimulationSettings,
) -> Option<StaticWinFeatures> {
    let odds = game.draw_odds_for_player(player_id)?;
    let player = game
        .players()
        .iter()
        .find(|player| player.id() == player_id)?;
    let score = game.player_score(player_id)?;
    let target = settings.target_score.max(1) as f64;
    let stay_total = score.total_score.saturating_add(odds.current_score);
    let opponent_totals = game
        .score_board()
        .iter()
        .filter(|score| score.player_id != player_id)
        .map(|score| score.total_score + score.current_round_score.unwrap_or(0))
        .collect::<Vec<_>>();
    let player_count = game.players().len();
    let active_players_remaining = game
        .players()
        .iter()
        .filter(|player| player.is_active_in_round())
        .count();
    let better_count = opponent_totals
        .iter()
        .filter(|opponent_total| **opponent_total > stay_total)
        .count();
    let relative_rank_after_stay = if player_count > 1 {
        (player_count.saturating_sub(better_count + 1)) as f64 / (player_count - 1) as f64
    } else {
        1.0
    };
    let margins = opponent_totals
        .iter()
        .map(|opponent_total| (stay_total as f64 - *opponent_total as f64) / target)
        .collect::<Vec<_>>();
    let average_margin_after_stay = if margins.is_empty() {
        0.0
    } else {
        margins.iter().sum::<f64>() / margins.len() as f64
    }
    .clamp(-1.0, 1.0);
    let worst_margin_after_stay = margins
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let field_pressure = if opponent_totals.is_empty() {
        0.0
    } else {
        opponent_totals
            .iter()
            .map(|opponent_total| ((*opponent_total as f64 - stay_total as f64) / target).max(0.0))
            .sum::<f64>()
            / opponent_totals.len() as f64
    }
    .clamp(0.0, 1.0);
    let highest_projected_total = game
        .score_board()
        .iter()
        .map(|score| {
            if score.player_id == player_id {
                stay_total
            } else {
                score.total_score + score.current_round_score.unwrap_or(0)
            }
        })
        .max()
        .unwrap_or(stay_total);
    let endgame_urgency = (highest_projected_total as f64 / target).clamp(0.0, 1.0);
    let number_card_count = player.distinct_number_count();
    let has_second_chance = player.has_second_chance();
    let effective_bust_probability = if has_second_chance {
        odds.bust_probability * 0.35
    } else {
        odds.bust_probability
    };

    Some(StaticWinFeatures {
        player_count,
        active_players_remaining,
        relative_rank_after_stay,
        average_margin_after_stay,
        worst_margin_after_stay,
        field_pressure,
        endgame_urgency,
        current_round_score: odds.current_score,
        current_round_score_ratio: odds.current_score as f64 / target,
        bust_probability: odds.bust_probability,
        effective_bust_probability,
        expected_score_delta: odds.expected_score_delta,
        expected_score_delta_ratio: odds.expected_score_delta / target,
        flip_seven_probability: odds.flip_seven_probability,
        flip_seven_distance: 7usize.saturating_sub(number_card_count),
        number_card_count,
        has_second_chance,
    })
}

fn static_win_evaluation_with_terms(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationSettings,
) -> Option<(StaticWinEvaluation, Vec<EquityTerm>)> {
    let features = static_win_features(game, context.player_id, settings)?;
    let weights = StaticWinWeights::CALIBRATED_V1;
    let target = settings.target_score.max(1) as f64;
    let own_total = game.player_score(context.player_id)?.total_score;
    let stay_total_ratio =
        (own_total.saturating_add(features.current_round_score) as f64 / target).clamp(0.0, 1.4);
    let active_remaining_after_stay = features.active_players_remaining.saturating_sub(1) as f64
        / features.player_count.max(1) as f64;
    let rank_deficit = 1.0 - features.relative_rank_after_stay;
    let chase_pressure =
        (features.field_pressure + rank_deficit * 0.18) * (0.70 + features.endgame_urgency * 0.65);
    let flip_seven_closeness = (features.number_card_count as f64 / 6.0).clamp(0.0, 1.0);
    let low_bank_build = (1.0 - (features.current_round_score as f64 / 18.0).clamp(0.0, 1.0))
        * (1.0 - features.endgame_urgency * 0.35);
    let bust_multiplier = 0.42
        + (features.current_round_score as f64 / 32.0).clamp(0.0, 1.25)
        + features.endgame_urgency * 0.38;

    let stay_terms = [
        EquityTerm {
            name: "bank progress",
            value: 0.95 * stay_total_ratio,
        },
        EquityTerm {
            name: "field rank",
            value: weights.field_rank_weight * (features.relative_rank_after_stay - 0.5),
        },
        EquityTerm {
            name: "score margins",
            value: weights.score_pressure_weight
                * (features.average_margin_after_stay * 0.42
                    + features.worst_margin_after_stay * 0.22),
        },
        EquityTerm {
            name: "endgame bank",
            value: weights.endgame_urgency_weight
                * features.endgame_urgency
                * features.current_round_score_ratio,
        },
        EquityTerm {
            name: "active field",
            value: -0.06 * active_remaining_after_stay,
        },
    ];
    let stay_equity = stay_terms.iter().map(|term| term.value).sum::<f64>();

    let draw_terms = [
        EquityTerm {
            name: "score pressure",
            value: weights.score_pressure_weight * chase_pressure,
        },
        EquityTerm {
            name: "next-card EV",
            value: weights.ev_reward * features.expected_score_delta_ratio,
        },
        EquityTerm {
            name: "Flip 7 upside",
            value: weights.flip_seven_reward
                * features.flip_seven_probability
                * (0.65 + flip_seven_closeness * 0.75 + features.endgame_urgency * 0.20),
        },
        EquityTerm {
            name: "Second Chance",
            value: if features.has_second_chance {
                weights.second_chance_bonus * (0.08 + features.bust_probability * 0.70)
            } else {
                0.0
            },
        },
        EquityTerm {
            name: "build round",
            value: 0.20 * low_bank_build,
        },
        EquityTerm {
            name: "bust risk",
            value: -weights.bust_penalty * features.effective_bust_probability * bust_multiplier,
        },
        EquityTerm {
            name: "secure bank",
            value: -0.08 * features.current_round_score_ratio * (1.0 + features.endgame_urgency),
        },
    ];
    let draw_equity = stay_equity + draw_terms.iter().map(|term| term.value).sum::<f64>();
    let terms = stay_terms.into_iter().chain(draw_terms).collect::<Vec<_>>();

    Some((
        StaticWinEvaluation {
            stay_equity,
            draw_equity,
            decision_margin: weights.decision_margin,
            features,
        },
        terms,
    ))
}

fn top_terms(terms: &[EquityTerm]) -> String {
    let mut terms = terms.to_vec();
    terms.sort_by(|left, right| right.value.abs().total_cmp(&left.value.abs()));
    terms
        .into_iter()
        .take(4)
        .map(|term| format!("{} {:+.3}", term.name, term.value))
        .collect::<Vec<_>>()
        .join(", ")
}
