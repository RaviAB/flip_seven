use std::fmt;

use serde::{Deserialize, Serialize};

use super::kind::{StrategyKind, unique_strategies};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonMode {
    MixedPool,
    HeadToHead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub comparison_mode: ComparisonMode,
    pub rollout_count: usize,
    pub decision_rollouts: usize,
    pub rng_seed: u64,
    pub target_score: u32,
    pub max_rounds: u32,
    pub player_count: usize,
    pub strategies: Vec<StrategyKind>,
    pub mirrored_seating: bool,
    pub parallel: bool,
    /// Minimum seeded runs per seating before checking convergence.
    pub min_matches: Option<usize>,
    /// Maximum seeded runs per seating when convergence is enabled.
    pub max_matches: Option<usize>,
    pub win_ci_width: Option<f64>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            comparison_mode: ComparisonMode::MixedPool,
            rollout_count: 1_000,
            decision_rollouts: 24,
            rng_seed: 7,
            target_score: 200,
            max_rounds: 40,
            player_count: 2,
            strategies: vec![StrategyKind::Conservative, StrategyKind::Balanced],
            mirrored_seating: true,
            parallel: true,
            min_matches: None,
            max_matches: None,
            win_ci_width: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimulationError {
    ZeroRollouts,
    ZeroDecisionRollouts,
    InvalidPlayerCount { player_count: usize },
    EmptyStrategies,
    ZeroTargetScore,
    ZeroMaxRounds,
    InvalidMatchRange { min: usize, max: usize },
    IncompleteConvergenceSettings,
    InvalidConfidenceWidth { width: f64 },
    HeadToHeadRequiresTwoPlayers,
    HeadToHeadRequiresTwoStrategies,
    ExecutionFailed { reason: String },
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutionFailed { reason } => write!(formatter, "simulation failed: {reason}"),
            Self::ZeroRollouts => formatter.write_str("rollout_count must be greater than zero"),
            Self::ZeroDecisionRollouts => {
                formatter.write_str("decision_rollouts must be greater than zero")
            }
            Self::InvalidPlayerCount { player_count } => write!(
                formatter,
                "player_count must be between 1 and 18, got {player_count}"
            ),
            Self::EmptyStrategies => formatter.write_str("at least one strategy is required"),
            Self::ZeroTargetScore => formatter.write_str("target_score must be greater than zero"),
            Self::ZeroMaxRounds => formatter.write_str("max_rounds must be greater than zero"),
            Self::InvalidMatchRange { min, max } => write!(
                formatter,
                "max_matches ({max}) must be at least min_matches ({min})"
            ),
            Self::IncompleteConvergenceSettings => formatter.write_str(
                "min_matches and win_ci_width must be supplied together; max_matches is optional",
            ),
            Self::InvalidConfidenceWidth { width } => write!(
                formatter,
                "win_ci_width must be finite and in the range (0, 1], got {width}"
            ),
            Self::HeadToHeadRequiresTwoPlayers => {
                formatter.write_str("head-to-head mode requires player_count = 2")
            }
            Self::HeadToHeadRequiresTwoStrategies => {
                formatter.write_str("head-to-head mode requires at least two distinct strategies")
            }
        }
    }
}

impl std::error::Error for SimulationError {}

pub(super) fn validate_config(config: &SimulationConfig) -> Result<(), SimulationError> {
    if config.rollout_count == 0 {
        return Err(SimulationError::ZeroRollouts);
    }
    if config.decision_rollouts == 0 {
        return Err(SimulationError::ZeroDecisionRollouts);
    }
    if !(1..=18).contains(&config.player_count) {
        return Err(SimulationError::InvalidPlayerCount {
            player_count: config.player_count,
        });
    }
    if config.strategies.is_empty() {
        return Err(SimulationError::EmptyStrategies);
    }
    if config.target_score == 0 {
        return Err(SimulationError::ZeroTargetScore);
    }
    if config.max_rounds == 0 {
        return Err(SimulationError::ZeroMaxRounds);
    }
    match (config.min_matches, config.max_matches, config.win_ci_width) {
        (None, None, None) => {}
        (Some(min), max, Some(width)) => {
            if min == 0 {
                return Err(SimulationError::ZeroRollouts);
            }
            if let Some(max) = max
                && max < min
            {
                return Err(SimulationError::InvalidMatchRange { min, max });
            }
            if !(width.is_finite() && 0.0 < width && width <= 1.0) {
                return Err(SimulationError::InvalidConfidenceWidth { width });
            }
        }
        _ => return Err(SimulationError::IncompleteConvergenceSettings),
    }
    if config.comparison_mode == ComparisonMode::HeadToHead {
        if config.player_count != 2 {
            return Err(SimulationError::HeadToHeadRequiresTwoPlayers);
        }
        if unique_strategies(&config.strategies).len() < 2 {
            return Err(SimulationError::HeadToHeadRequiresTwoStrategies);
        }
    }
    Ok(())
}
