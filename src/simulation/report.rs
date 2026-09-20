use serde::{Deserialize, Serialize};

use crate::model::PlayerId;

use super::config::{ComparisonMode, SimulationConfig};
use super::kind::{StrategyKind, strategy_sort_key, unique_strategies};
use super::simulator::SimulatedMatch;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub strategy: StrategyKind,
    /// All attempted matches, including those stopped at the round limit.
    pub matches: usize,
    pub completed_matches: usize,
    pub round_limited_matches: usize,
    pub wins: usize,
    /// Wins divided by completed matches; zero when there are no completed matches.
    pub win_rate: f64,
    pub win_ci_lower: f64,
    pub win_ci_upper: f64,
    pub average_final_score: f64,
    pub final_score_standard_error: f64,
    /// Completed matches only; zero when no match finished.
    pub average_rounds_to_finish: f64,
    pub bust_rate: f64,
    pub average_pre_draw_bust_risk: f64,
    pub flip_seven_rate: f64,
    pub average_points_per_round: f64,
    pub seat_exposure: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationReport {
    pub settings: SimulationConfig,
    pub total_match_runs: usize,
    pub completed_match_runs: usize,
    pub round_limited_match_runs: usize,
    pub total_rounds: u64,
    pub total_decisions: u64,
    pub summaries: Vec<SimulationSummary>,
}

#[derive(Debug, Clone)]
struct StrategyStats {
    strategy: StrategyKind,
    matches: usize,
    completed_matches: usize,
    wins: usize,
    final_score_sum: u64,
    final_score_square_sum: f64,
    rounds_sum: u64,
    busted_sum: u64,
    completed_rounds_sum: u64,
    bust_risk_basis_points_sum: u64,
    bust_risk_samples: u64,
    flip_seven_sum: u64,
    seat_exposure: Vec<usize>,
}

impl StrategyStats {
    fn new(strategy: StrategyKind, player_count: usize) -> Self {
        Self {
            strategy,
            matches: 0,
            completed_matches: 0,
            wins: 0,
            final_score_sum: 0,
            final_score_square_sum: 0.0,
            rounds_sum: 0,
            busted_sum: 0,
            completed_rounds_sum: 0,
            bust_risk_basis_points_sum: 0,
            bust_risk_samples: 0,
            flip_seven_sum: 0,
            seat_exposure: vec![0; player_count],
        }
    }

    fn record(&mut self, result: &SimulatedMatch, player_id: PlayerId) {
        let Some(score) = result.game.player_score(player_id) else {
            return;
        };

        self.matches += 1;
        self.wins += usize::from(result.winner() == Some(player_id));
        if result.winner().is_some() {
            self.completed_matches += 1;
            self.rounds_sum += result.rounds_played as u64;
        }
        self.final_score_sum += score.total_score as u64;
        self.final_score_square_sum += (score.total_score as f64).powi(2);
        self.busted_sum += score.busted_count as u64;
        self.completed_rounds_sum += score.rounds_completed as u64;
        self.bust_risk_basis_points_sum += score.telemetry.bust_risk_sum_basis_points;
        self.bust_risk_samples += score.telemetry.bust_risk_samples as u64;
        self.flip_seven_sum += score.flip_seven_count as u64;
        if let Some(exposure) = self.seat_exposure.get_mut(player_id.index()) {
            *exposure += 1;
        }
    }

    fn summary(self) -> SimulationSummary {
        let matches = self.matches.max(1) as f64;
        let completed_rounds = self.completed_rounds_sum.max(1) as f64;
        let bust_risk_samples = self.bust_risk_samples.max(1) as f64;
        let mean = self.final_score_sum as f64 / matches;
        let variance = if self.matches > 1 {
            (self.final_score_square_sum - matches * mean.powi(2)).max(0.0)
                / (self.matches - 1) as f64
        } else {
            0.0
        };
        let (win_ci_lower, win_ci_upper) = wilson_interval(self.wins, self.completed_matches);

        SimulationSummary {
            strategy: self.strategy,
            matches: self.matches,
            completed_matches: self.completed_matches,
            round_limited_matches: self.matches - self.completed_matches,
            wins: self.wins,
            win_rate: self.wins as f64 / self.completed_matches.max(1) as f64,
            win_ci_lower,
            win_ci_upper,
            average_final_score: mean,
            final_score_standard_error: (variance / matches).sqrt(),
            average_rounds_to_finish: self.rounds_sum as f64 / self.completed_matches.max(1) as f64,
            bust_rate: self.busted_sum as f64 / completed_rounds,
            average_pre_draw_bust_risk: self.bust_risk_basis_points_sum as f64
                / bust_risk_samples
                / 10_000.0,
            flip_seven_rate: self.flip_seven_sum as f64 / completed_rounds,
            average_points_per_round: self.final_score_sum as f64 / completed_rounds,
            seat_exposure: self.seat_exposure,
        }
    }
}

/// Stores aggregate statistics only, independent of the number of match runs.
pub(super) struct ReportAccumulator {
    stats: Vec<StrategyStats>,
    total_match_runs: usize,
    completed_match_runs: usize,
    total_rounds: u64,
    total_decisions: u64,
}

impl ReportAccumulator {
    pub(super) fn new(settings: &SimulationConfig) -> Self {
        Self {
            stats: unique_strategies(&settings.strategies)
                .into_iter()
                .map(|strategy| StrategyStats::new(strategy, settings.player_count))
                .collect(),
            total_match_runs: 0,
            completed_match_runs: 0,
            total_rounds: 0,
            total_decisions: 0,
        }
    }

    pub(super) fn record(&mut self, seating: &[StrategyKind], result: &SimulatedMatch) {
        self.total_match_runs += 1;
        self.completed_match_runs += usize::from(result.winner().is_some());
        self.total_rounds += result.rounds_played as u64;
        self.total_decisions += result.decisions_made as u64;
        for (seat, strategy) in seating.iter().enumerate() {
            if let Some(stats) = self
                .stats
                .iter_mut()
                .find(|stats| stats.strategy == *strategy)
            {
                stats.record(result, PlayerId::new(seat));
            }
        }
    }

    pub(super) fn report(&self, settings: SimulationConfig) -> SimulationReport {
        let mut summaries = self
            .stats
            .iter()
            .cloned()
            .map(StrategyStats::summary)
            .collect::<Vec<_>>();
        if settings.comparison_mode == ComparisonMode::HeadToHead {
            summaries.sort_by(|left, right| {
                right.win_rate.total_cmp(&left.win_rate).then_with(|| {
                    strategy_sort_key(left.strategy).cmp(&strategy_sort_key(right.strategy))
                })
            });
        } else {
            summaries.sort_by_key(|summary| strategy_sort_key(summary.strategy));
        }
        SimulationReport {
            settings,
            total_match_runs: self.total_match_runs,
            completed_match_runs: self.completed_match_runs,
            round_limited_match_runs: self.total_match_runs - self.completed_match_runs,
            total_rounds: self.total_rounds,
            total_decisions: self.total_decisions,
            summaries,
        }
    }
}

fn wilson_interval(wins: usize, matches: usize) -> (f64, f64) {
    if matches == 0 {
        return (0.0, 1.0);
    }

    let n = matches as f64;
    let p = wins as f64 / n;
    let z = 1.959_963_984_540_054;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half_width = z * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt() / denominator;
    (
        (center - half_width).max(0.0),
        (center + half_width).min(1.0),
    )
}
