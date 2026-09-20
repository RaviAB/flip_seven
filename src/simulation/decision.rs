use crate::model::{DrawOdds, GameState, PendingStep, PlayerId, SpecialAction};

use super::config::SimulationConfig;
use super::kind::StrategyKind;
use super::monte_carlo::evaluate_monte_carlo_actions;
use super::simulator::SimulationFailure;
use super::static_equity::max_win_static_decision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDecision {
    Stay,
    Draw,
    ChooseTarget(PlayerId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionContext {
    pub player_id: PlayerId,
    pub pending_action: Option<PendingStep>,
    pub legal_targets: Vec<PlayerId>,
    pub draw_odds: Option<DrawOdds>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyRecommendation {
    pub strategy: StrategyKind,
    pub decision: Option<AiDecision>,
    pub rationale: String,
}

#[derive(Debug, Clone)]
pub(super) struct DecisionWithRationale {
    pub(super) decision: Option<AiDecision>,
    pub(super) rationale: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WinProbabilityProfile {
    pub(super) pressure: f64,
    pub(super) bust_risk_cap: f64,
    pub(super) building_bust_risk_cap: f64,
    pub(super) flip_seven_risk_cap: f64,
    pub(super) flip_seven_chase_probability: f64,
    pub(super) minimum_bank_score: u32,
    pub(super) required_ev_edge: f64,
}

pub(super) fn recommend_decision(
    game: &GameState,
    strategy: StrategyKind,
    settings: &SimulationConfig,
) -> Result<StrategyRecommendation, SimulationFailure> {
    let Some(context) = decision_context(game) else {
        return Ok(StrategyRecommendation {
            strategy,
            decision: None,
            rationale: "No legal AI decision is pending.".to_owned(),
        });
    };

    let decision = if context
        .pending_action
        .as_ref()
        .is_some_and(|step| step.needs_target())
    {
        match strategy {
            StrategyKind::MaxWinProbability | StrategyKind::MaxWinStatic => {
                target_by_rollout_value(game, &context, strategy, settings)?
            }
            _ => target_by_heuristic(game, &context, strategy),
        }
    } else {
        match strategy {
            StrategyKind::Conservative | StrategyKind::Balanced | StrategyKind::Aggressive => {
                risk_profile_decision(game, &context, strategy, settings)
            }
            StrategyKind::MaxRoundEv => max_round_ev_decision(game, &context, settings),
            StrategyKind::MaxWinProbability => {
                max_win_probability_decision(game, &context, settings)
            }
            StrategyKind::MaxWinStatic => {
                max_win_static_strategy_decision(game, &context, settings)
            }
            StrategyKind::StayAtNumberCount(_) | StrategyKind::StayAtScore(_) => {
                simple_threshold_decision(game, &context, strategy, settings)
            }
        }
    };

    Ok(StrategyRecommendation {
        strategy,
        decision: decision.decision,
        rationale: decision.rationale,
    })
}

fn risk_profile_decision(
    game: &GameState,
    context: &DecisionContext,
    strategy: StrategyKind,
    settings: &SimulationConfig,
) -> DecisionWithRationale {
    let Some(odds) = context.draw_odds.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No draw odds are available.".to_owned(),
        };
    };

    let profile = risk_profile(strategy);
    let score_to_win = game
        .score_to_win_target(context.player_id, settings.target_score)
        .unwrap_or(u32::MAX);
    if odds.current_score > 0 && odds.current_score >= score_to_win {
        return DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: banking {} reaches the {} point target.",
                odds.current_score, settings.target_score
            ),
        };
    }

    let flip_seven_chase = odds.flip_seven_probability >= profile.flip_seven_chase_probability
        && odds.bust_probability <= profile.flip_seven_risk_cap;
    let ev_ok =
        odds.expected_score_after_draw >= odds.current_score as f64 + profile.required_ev_edge;
    let risk_ok = odds.bust_probability <= profile.bust_risk_cap;
    let building = odds.current_score < profile.minimum_bank_score;

    if building || flip_seven_chase || (ev_ok && risk_ok) {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: {} accepts {:.1}% bust risk for next-card EV {:.2}.",
                strategy.label(),
                odds.bust_probability * 100.0,
                odds.expected_score_after_draw
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: {} banks {} with {:.1}% bust risk on a draw.",
                strategy.label(),
                odds.current_score,
                odds.bust_probability * 100.0
            ),
        }
    }
}

fn simple_threshold_decision(
    game: &GameState,
    context: &DecisionContext,
    strategy: StrategyKind,
    settings: &SimulationConfig,
) -> DecisionWithRationale {
    let Some(odds) = context.draw_odds.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No draw odds are available.".to_owned(),
        };
    };

    let Some(player) = game
        .players()
        .iter()
        .find(|player| player.id() == context.player_id)
    else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No player is available.".to_owned(),
        };
    };

    let score_to_win = game
        .score_to_win_target(context.player_id, settings.target_score)
        .unwrap_or(u32::MAX);
    if odds.current_score > 0 && odds.current_score >= score_to_win {
        return DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: banking {} reaches the {} point target.",
                odds.current_score, settings.target_score
            ),
        };
    }

    if player.has_second_chance() {
        return DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: {} keeps drawing while protected by Second Chance.",
                strategy.label()
            ),
        };
    }

    let should_stay = match strategy {
        StrategyKind::StayAtNumberCount(threshold) => {
            player.hand().iter().filter(|card| card.is_number()).count() >= threshold as usize
        }
        StrategyKind::StayAtScore(threshold) => odds.current_score >= threshold,
        _ => false,
    };

    if should_stay {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: {} threshold reached with {} points.",
                strategy.label(),
                odds.current_score
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!("Draw: {} threshold has not been reached.", strategy.label()),
        }
    }
}

fn max_round_ev_decision(
    _game: &GameState,
    context: &DecisionContext,
    _settings: &SimulationConfig,
) -> DecisionWithRationale {
    let Some(odds) = context.draw_odds.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No draw odds are available.".to_owned(),
        };
    };

    if odds.expected_score_after_draw > odds.current_score as f64 {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: next-card EV {:.2} beats staying at {} by {:.2}.",
                odds.expected_score_after_draw, odds.current_score, odds.expected_score_delta
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: bank {} instead of next-card EV {:.2}.",
                odds.current_score, odds.expected_score_after_draw
            ),
        }
    }
}

fn max_win_probability_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationConfig,
) -> DecisionWithRationale {
    let legal = legal_ai_decisions(game);
    if legal.is_empty() {
        return DecisionWithRationale {
            decision: None,
            rationale: "No legal decision is available.".to_owned(),
        };
    }

    max_win_probability_draw_stay_decision(game, context, settings)
}

fn max_win_probability_draw_stay_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationConfig,
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

    let own_total = score.total_score;
    let leader_total = leading_opponent_score(game, context.player_id);
    let stay_total = own_total + odds.current_score;
    let score_to_win = settings.target_score.saturating_sub(own_total);
    if odds.current_score > 0 && odds.current_score >= score_to_win {
        return DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: banking {} reaches {}. Own {}, leading opponent {}.",
                odds.current_score, settings.target_score, own_total, leader_total
            ),
        };
    }

    let profile = win_probability_profile(leader_total, stay_total, settings);
    if odds.current_score == 0 {
        return DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: no points to bank. Own {}, leading opponent {}, pressure {:.2}.",
                own_total, leader_total, profile.pressure
            ),
        };
    }

    let ev_ok =
        odds.expected_score_after_draw >= odds.current_score as f64 + profile.required_ev_edge;
    let risk_ok = odds.bust_probability <= profile.bust_risk_cap;
    let building = odds.current_score < profile.minimum_bank_score
        && odds.bust_probability <= profile.building_bust_risk_cap;
    let flip_seven_chase = odds.flip_seven_probability >= profile.flip_seven_chase_probability
        && odds.bust_probability <= profile.flip_seven_risk_cap;
    let should_draw = building || flip_seven_chase || (ev_ok && risk_ok);

    if should_draw {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: pressure {:.2} with own {}, leading opponent {}, stay total {}. Bust {:.1}% <= cap {:.1}%, EV delta {:.2} vs required {:.2}.",
                profile.pressure,
                own_total,
                leader_total,
                stay_total,
                odds.bust_probability * 100.0,
                profile.bust_risk_cap * 100.0,
                odds.expected_score_delta,
                profile.required_ev_edge
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: pressure {:.2} with own {}, leading opponent {}, banked total {}. Bust {:.1}% exceeds cap {:.1}% or EV delta {:.2} is too low.",
                profile.pressure,
                own_total,
                leader_total,
                stay_total,
                odds.bust_probability * 100.0,
                profile.bust_risk_cap * 100.0,
                odds.expected_score_delta
            ),
        }
    }
}

fn max_win_static_strategy_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: &SimulationConfig,
) -> DecisionWithRationale {
    let legal = legal_ai_decisions(game);
    if legal.is_empty() {
        return DecisionWithRationale {
            decision: None,
            rationale: "No legal decision is available.".to_owned(),
        };
    }

    max_win_static_decision(game, context, settings)
}

fn leading_opponent_score(game: &GameState, player_id: PlayerId) -> u32 {
    game.score_board()
        .iter()
        .filter(|score| score.player_id != player_id)
        .map(|score| score.total_score)
        .max()
        .unwrap_or(0)
}

pub(super) fn win_probability_profile(
    leader_total: u32,
    stay_total: u32,
    settings: &SimulationConfig,
) -> WinProbabilityProfile {
    let target = settings.target_score.max(1) as f64;
    let score_pressure = (leader_total as f64 - stay_total as f64) / target;
    let pressure = score_pressure.clamp(-0.35, 0.35) / 0.35;
    WinProbabilityProfile {
        pressure,
        bust_risk_cap: (0.34 + pressure * 0.24).clamp(0.16, 0.58),
        building_bust_risk_cap: (0.45 + pressure * 0.25).clamp(0.20, 0.70),
        flip_seven_risk_cap: (0.55 + pressure * 0.25).clamp(0.30, 0.80),
        flip_seven_chase_probability: (0.12 - pressure * 0.06).clamp(0.06, 0.18),
        minimum_bank_score: ((16.0 + pressure * 12.0).round() as u32).clamp(4, 28),
        required_ev_edge: (-3.0 * pressure).clamp(-3.0, 3.0),
    }
}

fn target_by_heuristic(
    game: &GameState,
    context: &DecisionContext,
    strategy: StrategyKind,
) -> DecisionWithRationale {
    let Some(pending_action) = context.pending_action.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No target decision is pending.".to_owned(),
        };
    };

    let mut candidates = context.legal_targets.clone();
    if pending_action.action() == SpecialAction::Freeze && candidates.len() > 1 {
        candidates.retain(|target| *target != context.player_id);
    }

    let chosen = match pending_action.action() {
        SpecialAction::SecondChance => candidates
            .into_iter()
            .min_by_key(|target| projected_player_value(game, *target)),
        SpecialAction::Freeze => candidates
            .into_iter()
            .max_by_key(|target| projected_player_value(game, *target)),
        SpecialAction::FlipThree => {
            if strategy == StrategyKind::Aggressive
                && context
                    .draw_odds
                    .as_ref()
                    .is_some_and(|odds| odds.bust_probability <= 0.25)
                && context.legal_targets.contains(&context.player_id)
            {
                Some(context.player_id)
            } else {
                candidates
                    .into_iter()
                    .filter(|target| {
                        *target != context.player_id || context.legal_targets.len() == 1
                    })
                    .max_by_key(|target| projected_player_value(game, *target))
            }
        }
    };

    let Some(target_id) = chosen else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No active legal targets are available.".to_owned(),
        };
    };

    DecisionWithRationale {
        decision: Some(AiDecision::ChooseTarget(target_id)),
        rationale: format!(
            "Choose {} for {} by {} target heuristic.",
            player_name(game, target_id),
            format_args!("{:?}", pending_action.action()),
            strategy.label()
        ),
    }
}

fn target_by_rollout_value(
    game: &GameState,
    context: &DecisionContext,
    _strategy: StrategyKind,
    settings: &SimulationConfig,
) -> Result<DecisionWithRationale, SimulationFailure> {
    let Some(pending_action) = context.pending_action.as_ref() else {
        return Ok(DecisionWithRationale {
            decision: None,
            rationale: "No target decision is pending.".to_owned(),
        });
    };

    let decisions = context
        .legal_targets
        .iter()
        .copied()
        .map(AiDecision::ChooseTarget)
        .collect::<Vec<_>>();

    let Some(evaluation) =
        evaluate_monte_carlo_actions(game, context.player_id, &decisions, settings)?
    else {
        return Ok(DecisionWithRationale {
            decision: None,
            rationale: "No active legal targets are available.".to_owned(),
        });
    };

    let target_name = match evaluation.decision {
        AiDecision::ChooseTarget(player_id) => player_name(game, player_id),
        AiDecision::Stay | AiDecision::Draw => "-".to_owned(),
    };
    let basis = format!(
        "Monte Carlo win {:.1}%, utility {:.3}; {} completed of {} samples",
        evaluation.win_rate * 100.0,
        evaluation.average_utility,
        evaluation.completed_samples,
        evaluation.samples
    );

    Ok(DecisionWithRationale {
        decision: Some(evaluation.decision),
        rationale: format!(
            "Choose {target_name} for {}: {basis}.",
            format_args!("{:?}", pending_action.action())
        ),
    })
}

pub(super) fn current_strategy_player_id(game: &GameState) -> Option<PlayerId> {
    if game.round_over() {
        return None;
    }

    if let Some(pending_action) = game.pending_action() {
        return pending_action
            .needs_target()
            .then_some(pending_action.source_player_id());
    }

    game.current_player()
        .filter(|player| player.is_active_in_round())
        .map(|player| player.id())
}

fn decision_context(game: &GameState) -> Option<DecisionContext> {
    let player_id = current_strategy_player_id(game)?;
    let draw_odds = game.draw_odds_for_player(player_id);

    Some(DecisionContext {
        player_id,
        pending_action: game.pending_action(),
        legal_targets: game.legal_pending_targets(),
        draw_odds,
    })
}

pub(super) fn legal_ai_decisions(game: &GameState) -> Vec<AiDecision> {
    if game.round_over() {
        return Vec::new();
    }

    if let Some(pending_action) = game.pending_action() {
        if pending_action.needs_target() {
            return game
                .legal_pending_targets()
                .into_iter()
                .map(AiDecision::ChooseTarget)
                .collect();
        }

        return Vec::new();
    }

    if game
        .current_player()
        .is_some_and(|player| player.is_active_in_round())
    {
        vec![AiDecision::Stay, AiDecision::Draw]
    } else {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
struct RiskProfile {
    bust_risk_cap: f64,
    flip_seven_risk_cap: f64,
    flip_seven_chase_probability: f64,
    minimum_bank_score: u32,
    required_ev_edge: f64,
}

fn risk_profile(strategy: StrategyKind) -> RiskProfile {
    match strategy {
        StrategyKind::Conservative => RiskProfile {
            bust_risk_cap: 0.18,
            flip_seven_risk_cap: 0.35,
            flip_seven_chase_probability: 0.18,
            minimum_bank_score: 10,
            required_ev_edge: 2.0,
        },
        StrategyKind::Balanced => RiskProfile {
            bust_risk_cap: 0.32,
            flip_seven_risk_cap: 0.50,
            flip_seven_chase_probability: 0.14,
            minimum_bank_score: 16,
            required_ev_edge: 0.0,
        },
        StrategyKind::Aggressive => RiskProfile {
            bust_risk_cap: 0.55,
            flip_seven_risk_cap: 0.75,
            flip_seven_chase_probability: 0.08,
            minimum_bank_score: 24,
            required_ev_edge: -2.0,
        },
        StrategyKind::MaxRoundEv | StrategyKind::MaxWinProbability | StrategyKind::MaxWinStatic => {
            RiskProfile {
                bust_risk_cap: 1.0,
                flip_seven_risk_cap: 1.0,
                flip_seven_chase_probability: 1.0,
                minimum_bank_score: 0,
                required_ev_edge: 0.0,
            }
        }
        StrategyKind::StayAtNumberCount(_) | StrategyKind::StayAtScore(_) => RiskProfile {
            bust_risk_cap: 1.0,
            flip_seven_risk_cap: 1.0,
            flip_seven_chase_probability: 1.0,
            minimum_bank_score: 0,
            required_ev_edge: 0.0,
        },
    }
}

pub(super) fn projected_player_value(game: &GameState, player_id: PlayerId) -> u32 {
    game.player_score(player_id)
        .map(|score| {
            score.total_score + score.current_round_score.unwrap_or(score.last_round_score)
        })
        .unwrap_or(0)
}

fn player_name(game: &GameState, player_id: PlayerId) -> String {
    game.players()
        .iter()
        .find(|player| player.id() == player_id)
        .map(|player| player.name().to_owned())
        .unwrap_or_else(|| format!("Player {}", player_id.display_number()))
}
