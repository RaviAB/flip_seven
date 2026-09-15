use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::prelude::*;

use crate::model::PlayerId;

use super::kind::{StrategyKind, strategy_sort_key, unique_strategies};
use super::monte_carlo::mix_seed;
use super::simulator::{SimulatedMatch, simulate_match};

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSettings {
    pub rollout_count: usize,
    pub decision_rollouts: usize,
    pub rng_seed: u64,
    pub target_score: u32,
    pub max_rounds: u32,
    pub player_count: usize,
    pub strategies: Vec<StrategyKind>,
    pub mirrored_seating: bool,
    pub parallel: bool,
    pub min_matches: Option<usize>,
    pub max_matches: Option<usize>,
    pub win_ci_width: Option<f64>,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
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
pub struct SimulationSummary {
    pub strategy: StrategyKind,
    pub matches: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub win_ci_lower: f64,
    pub win_ci_upper: f64,
    pub average_final_score: f64,
    pub final_score_standard_error: f64,
    pub average_rounds_to_finish: f64,
    pub bust_rate: f64,
    pub average_pre_draw_bust_risk: f64,
    pub flip_seven_rate: f64,
    pub average_points_per_round: f64,
    pub seat_exposure: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyReport {
    pub settings: SimulationSettings,
    pub total_match_runs: usize,
    pub total_rounds: u64,
    pub total_decisions: u64,
    pub summaries: Vec<SimulationSummary>,
}

#[derive(Debug, Clone)]
struct StrategyStats {
    strategy: StrategyKind,
    matches: usize,
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
        self.wins += usize::from(result.winner == Some(player_id));
        self.final_score_sum += score.total_score as u64;
        self.final_score_square_sum += (score.total_score as f64).powi(2);
        self.rounds_sum += result.rounds_played as u64;
        self.busted_sum += score.busted_count as u64;
        self.completed_rounds_sum += score.rounds_completed as u64;
        self.bust_risk_basis_points_sum += score.bust_risk_sum_basis_points;
        self.bust_risk_samples += score.bust_risk_sample_count as u64;
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
        let (win_ci_lower, win_ci_upper) = wilson_interval(self.wins, self.matches);

        SimulationSummary {
            strategy: self.strategy,
            matches: self.matches,
            wins: self.wins,
            win_rate: self.wins as f64 / matches,
            win_ci_lower,
            win_ci_upper,
            average_final_score: mean,
            final_score_standard_error: (variance / matches).sqrt(),
            average_rounds_to_finish: self.rounds_sum as f64 / matches,
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

pub fn compare_strategies(settings: SimulationSettings) -> StrategyReport {
    let mut settings = normalize_settings(settings);

    if let (Some(min_matches), Some(width)) = (settings.min_matches, settings.win_ci_width) {
        let max_matches = settings
            .max_matches
            .unwrap_or(settings.rollout_count.max(min_matches));
        let mut matches = min_matches.max(1);

        loop {
            settings.rollout_count = matches;
            let report = run_strategy_report(settings.clone());
            let widest = report
                .summaries
                .iter()
                .map(|summary| summary.win_ci_upper - summary.win_ci_lower)
                .fold(0.0, f64::max);

            if matches >= max_matches || widest <= width {
                return report;
            }

            matches = (matches.saturating_mul(2))
                .min(max_matches)
                .max(matches + 1);
        }
    }

    run_strategy_report(settings)
}

pub fn compare_head_to_head_strategies(settings: SimulationSettings) -> StrategyReport {
    let mut settings = normalize_settings(settings);
    settings.player_count = 2;
    run_head_to_head_strategy_report(settings)
}

fn run_strategy_report(settings: SimulationSettings) -> StrategyReport {
    let seatings = seating_rotations(&settings);
    let jobs = seatings
        .iter()
        .enumerate()
        .flat_map(|(rotation_index, seating)| {
            (0..settings.rollout_count).map(move |match_index| MatchJob {
                rotation_index,
                match_index,
                seating: seating.clone(),
            })
        })
        .collect::<Vec<_>>();

    let records = if settings.parallel && jobs.len() > 1 {
        jobs.par_iter()
            .map(|job| run_match_job(job, &settings))
            .collect::<Vec<_>>()
    } else {
        jobs.iter()
            .map(|job| run_match_job(job, &settings))
            .collect::<Vec<_>>()
    };

    let mut stats = unique_strategies(&settings.strategies)
        .into_iter()
        .map(|strategy| StrategyStats::new(strategy, settings.player_count))
        .collect::<Vec<_>>();
    let mut total_rounds = 0u64;
    let mut total_decisions = 0u64;

    for record in &records {
        total_rounds += record.result.rounds_played as u64;
        total_decisions += record.result.decisions_made as u64;

        for (seat_index, strategy) in record.seating.iter().enumerate() {
            let player_id = PlayerId::new(seat_index);
            if let Some(strategy_stats) = stats
                .iter_mut()
                .find(|strategy_stats| strategy_stats.strategy == *strategy)
            {
                strategy_stats.record(&record.result, player_id);
            }
        }
    }

    stats.sort_by_key(|stat| strategy_sort_key(stat.strategy));
    let summaries = stats.into_iter().map(StrategyStats::summary).collect();

    StrategyReport {
        settings,
        total_match_runs: records.len(),
        total_rounds,
        total_decisions,
        summaries,
    }
}

fn run_head_to_head_strategy_report(settings: SimulationSettings) -> StrategyReport {
    let strategies = unique_strategies(&settings.strategies);
    let jobs = strategies
        .iter()
        .enumerate()
        .flat_map(|(left_index, left)| {
            strategies.iter().enumerate().skip(left_index + 1).flat_map(
                move |(right_index, right)| {
                    [
                        HeadToHeadPairing {
                            matchup_id: matchup_id(left_index, right_index),
                            seating: [*left, *right],
                        },
                        HeadToHeadPairing {
                            matchup_id: matchup_id(left_index, right_index),
                            seating: [*right, *left],
                        },
                    ]
                },
            )
        })
        .flat_map(|pairing| {
            (0..settings.rollout_count).map(move |match_index| HeadToHeadMatchJob {
                matchup_id: pairing.matchup_id,
                match_index,
                seating: pairing.seating,
            })
        })
        .collect::<Vec<_>>();

    let records = if settings.parallel && jobs.len() > 1 {
        jobs.par_iter()
            .map(|job| run_head_to_head_match_job(job, &settings))
            .collect::<Vec<_>>()
    } else {
        jobs.iter()
            .map(|job| run_head_to_head_match_job(job, &settings))
            .collect::<Vec<_>>()
    };

    let mut stats = strategies
        .into_iter()
        .map(|strategy| StrategyStats::new(strategy, settings.player_count))
        .collect::<Vec<_>>();
    let mut total_rounds = 0u64;
    let mut total_decisions = 0u64;

    for record in &records {
        total_rounds += record.result.rounds_played as u64;
        total_decisions += record.result.decisions_made as u64;

        for (seat_index, strategy) in record.seating.iter().enumerate() {
            let player_id = PlayerId::new(seat_index);
            if let Some(strategy_stats) = stats
                .iter_mut()
                .find(|strategy_stats| strategy_stats.strategy == *strategy)
            {
                strategy_stats.record(&record.result, player_id);
            }
        }
    }

    let mut summaries = stats
        .into_iter()
        .map(StrategyStats::summary)
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .win_rate
            .total_cmp(&left.win_rate)
            .then_with(|| strategy_sort_key(left.strategy).cmp(&strategy_sort_key(right.strategy)))
    });

    StrategyReport {
        settings,
        total_match_runs: records.len(),
        total_rounds,
        total_decisions,
        summaries,
    }
}

#[derive(Debug, Clone, Copy)]
struct HeadToHeadPairing {
    matchup_id: u64,
    seating: [StrategyKind; 2],
}

#[derive(Debug, Clone)]
struct HeadToHeadMatchJob {
    matchup_id: u64,
    match_index: usize,
    seating: [StrategyKind; 2],
}

fn run_head_to_head_match_job(
    job: &HeadToHeadMatchJob,
    settings: &SimulationSettings,
) -> MatchRecord {
    let seed = match_seed(settings.rng_seed, job.matchup_id, 0, job.match_index as u64);
    let mut rng = StdRng::seed_from_u64(seed);
    let seating = job.seating.to_vec();
    let result = simulate_match(settings.player_count, &seating, settings, &mut rng);

    MatchRecord { seating, result }
}

fn matchup_id(left_index: usize, right_index: usize) -> u64 {
    ((left_index as u64) << 32) | right_index as u64
}

#[derive(Debug, Clone)]
struct MatchJob {
    rotation_index: usize,
    match_index: usize,
    seating: Vec<StrategyKind>,
}

#[derive(Debug, Clone)]
struct MatchRecord {
    seating: Vec<StrategyKind>,
    result: SimulatedMatch,
}

fn run_match_job(job: &MatchJob, settings: &SimulationSettings) -> MatchRecord {
    let seed = match_seed(
        settings.rng_seed,
        0,
        job.rotation_index as u64,
        job.match_index as u64,
    );
    let mut rng = StdRng::seed_from_u64(seed);
    let result = simulate_match(settings.player_count, &job.seating, settings, &mut rng);

    MatchRecord {
        seating: job.seating.clone(),
        result,
    }
}

fn normalize_settings(mut settings: SimulationSettings) -> SimulationSettings {
    settings.rollout_count = settings.rollout_count.max(1);
    settings.decision_rollouts = settings.decision_rollouts.max(1);
    settings.target_score = settings.target_score.max(1);
    settings.max_rounds = settings.max_rounds.max(1);
    settings.player_count = settings.player_count.max(1);
    if settings.strategies.is_empty() {
        settings.strategies.push(StrategyKind::Balanced);
    }
    if let Some(max_matches) = settings.max_matches
        && let Some(min_matches) = settings.min_matches
        && max_matches < min_matches
    {
        settings.max_matches = Some(min_matches);
    }
    settings
}

fn seating_rotations(settings: &SimulationSettings) -> Vec<Vec<StrategyKind>> {
    let bases = seating_bases(settings);
    let rotation_count = if settings.mirrored_seating {
        settings.player_count
    } else {
        1
    };

    bases
        .into_iter()
        .flat_map(|base| {
            (0..rotation_count).map(move |rotation| {
                (0..settings.player_count)
                    .map(|seat| base[(seat + rotation) % settings.player_count])
                    .collect()
            })
        })
        .collect()
}

fn seating_bases(settings: &SimulationSettings) -> Vec<Vec<StrategyKind>> {
    if settings.strategies.len() <= settings.player_count {
        return vec![
            (0..settings.player_count)
                .map(|index| settings.strategies[index % settings.strategies.len()])
                .collect(),
        ];
    }

    strategy_combinations(&settings.strategies, settings.player_count)
}

fn strategy_combinations(strategies: &[StrategyKind], size: usize) -> Vec<Vec<StrategyKind>> {
    fn collect(
        strategies: &[StrategyKind],
        size: usize,
        start: usize,
        current: &mut Vec<StrategyKind>,
        combinations: &mut Vec<Vec<StrategyKind>>,
    ) {
        if current.len() == size {
            combinations.push(current.clone());
            return;
        }

        let remaining_slots = size - current.len();
        let last_start = strategies.len().saturating_sub(remaining_slots);
        for index in start..=last_start {
            current.push(strategies[index]);
            collect(strategies, size, index + 1, current, combinations);
            current.pop();
        }
    }

    let mut combinations = Vec::new();
    collect(strategies, size, 0, &mut Vec::new(), &mut combinations);
    combinations
}

fn match_seed(base_seed: u64, matchup_id: u64, rotation_id: u64, match_index: u64) -> u64 {
    mix_seed(
        base_seed
            ^ matchup_id.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ rotation_id.wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ match_index.wrapping_mul(0x94D0_49BB_1331_11EB),
    )
}

fn wilson_interval(wins: usize, matches: usize) -> (f64, f64) {
    if matches == 0 {
        return (0.0, 0.0);
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
