mod config;
mod decision;
mod execution;
mod kind;
mod monte_carlo;
mod report;
mod static_equity;
mod targets;
mod thresholds;

use crate::model::{Card, Deck, GameState};
use crate::simulation::{ComparisonMode, SimulationConfig, StrategyKind};

fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
    GameState::with_deck(player_count, Deck::from_draw_order(cards))
}

fn settings() -> SimulationConfig {
    SimulationConfig {
        comparison_mode: ComparisonMode::MixedPool,
        rollout_count: 8,
        decision_rollouts: 2,
        rng_seed: 42,
        target_score: 50,
        max_rounds: 10,
        player_count: 2,
        strategies: vec![StrategyKind::Conservative, StrategyKind::Balanced],
        mirrored_seating: true,
        parallel: false,
        min_matches: None,
        max_matches: None,
        win_ci_width: None,
    }
}
