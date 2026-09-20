use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::prelude::*;

use super::config::{ComparisonMode, SimulationConfig};
use super::kind::{StrategyKind, unique_strategies};
use super::monte_carlo::mix_seed;
use super::report::{ReportAccumulator, SimulationReport};
use super::simulator::{SimulatedMatch, SimulationFailure, simulate_match};

// Bound retained match states even when a sweep requests millions of runs.
const MATCH_BATCH_SIZE: usize = 64;

pub(super) fn compare(
    mut settings: SimulationConfig,
) -> Result<SimulationReport, SimulationFailure> {
    let seatings = match_seatings(&settings);
    let mut accumulator = ReportAccumulator::new(&settings);
    let convergence = settings.min_matches.zip(settings.win_ci_width);
    let maximum = convergence.map_or(settings.rollout_count, |(minimum, _)| {
        settings
            .max_matches
            .unwrap_or(settings.rollout_count.max(minimum))
    });
    let mut requested = convergence.map_or(settings.rollout_count, |(minimum, _)| minimum);
    let mut completed = 0;
    loop {
        // Extend each seating's seeded sequence without replaying earlier matches.
        for seating in &seatings {
            for start in (completed..requested).step_by(MATCH_BATCH_SIZE) {
                let end = start.saturating_add(MATCH_BATCH_SIZE).min(requested);
                if settings.parallel {
                    let results = (start..end)
                        .into_par_iter()
                        .map(|index| run_match(seating, index, &settings))
                        .collect::<Vec<_>>();
                    // Consume in seed order, including errors, for deterministic reports.
                    for result in results {
                        accumulator.record(&seating.strategies, &result?);
                    }
                } else {
                    for index in start..end {
                        accumulator
                            .record(&seating.strategies, &run_match(seating, index, &settings)?);
                    }
                }
            }
        }
        completed = requested;
        settings.rollout_count = completed;
        let report = accumulator.report(settings.clone());
        let converged = convergence.is_some_and(|(_, width)| {
            report.summaries.iter().all(|summary| {
                summary.completed_matches > 0
                    && summary.win_ci_upper - summary.win_ci_lower <= width
            })
        });
        if convergence.is_none() || completed >= maximum || converged {
            return Ok(report);
        }
        requested = completed.saturating_mul(2).min(maximum);
    }
}

struct MatchSeating {
    matchup_id: u64,
    rotation_id: u64,
    strategies: Vec<StrategyKind>,
}

fn match_seatings(settings: &SimulationConfig) -> Vec<MatchSeating> {
    match settings.comparison_mode {
        ComparisonMode::MixedPool => seating_rotations(settings)
            .into_iter()
            .enumerate()
            .map(|(rotation, strategies)| MatchSeating {
                matchup_id: 0,
                rotation_id: rotation as u64,
                strategies,
            })
            .collect(),
        ComparisonMode::HeadToHead => {
            let strategies = unique_strategies(&settings.strategies);
            let mut seatings = Vec::new();
            for (left_index, left) in strategies.iter().enumerate() {
                for (right_index, right) in strategies.iter().enumerate().skip(left_index + 1) {
                    let matchup_id = ((left_index as u64) << 32) | right_index as u64;
                    seatings.push(MatchSeating {
                        matchup_id,
                        rotation_id: 0,
                        strategies: vec![*left, *right],
                    });
                    if settings.mirrored_seating {
                        seatings.push(MatchSeating {
                            matchup_id,
                            rotation_id: 0,
                            strategies: vec![*right, *left],
                        });
                    }
                }
            }
            seatings
        }
    }
}

fn run_match(
    seating: &MatchSeating,
    index: usize,
    settings: &SimulationConfig,
) -> Result<SimulatedMatch, SimulationFailure> {
    let seed = match_seed(
        settings.rng_seed,
        seating.matchup_id,
        seating.rotation_id,
        index as u64,
    );
    let mut rng = StdRng::seed_from_u64(seed);
    simulate_match(
        settings.player_count,
        &seating.strategies,
        settings,
        &mut rng,
    )
}

fn seating_rotations(settings: &SimulationConfig) -> Vec<Vec<StrategyKind>> {
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

fn seating_bases(settings: &SimulationConfig) -> Vec<Vec<StrategyKind>> {
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
