use super::settings;
use crate::simulation::{ComparisonMode, SimulationConfig, StrategyKind, run_simulation};

#[test]
fn simulations_are_reproducible_with_fixed_seed() {
    let first = run_simulation(settings()).unwrap();
    let second = run_simulation(settings()).unwrap();

    assert_eq!(first, second);
}

#[test]
fn mirrored_seating_gives_equal_seat_exposure() {
    let report = run_simulation(SimulationConfig {
        rollout_count: 3,
        player_count: 4,
        strategies: vec![
            StrategyKind::Conservative,
            StrategyKind::Balanced,
            StrategyKind::Aggressive,
            StrategyKind::MaxRoundEv,
        ],
        ..settings()
    })
    .unwrap();

    for summary in report.summaries {
        assert_eq!(summary.seat_exposure, vec![3, 3, 3, 3]);
    }
}

#[test]
fn mixed_pool_comparison_seats_every_strategy() {
    let report = run_simulation(SimulationConfig {
        rollout_count: 1,
        player_count: 4,
        strategies: vec![
            StrategyKind::MaxWinStatic,
            StrategyKind::MaxRoundEv,
            StrategyKind::Balanced,
            StrategyKind::Aggressive,
            StrategyKind::MaxWinProbability,
        ],
        decision_rollouts: 1,
        max_rounds: 4,
        parallel: false,
        ..settings()
    })
    .unwrap();

    assert_eq!(report.total_match_runs, 20);
    for summary in report.summaries {
        assert!(summary.matches > 0, "{:?}", summary.strategy);
    }
}

#[test]
fn human_sweep_round_robin_gives_equal_mirrored_exposure() {
    let strategies = vec![
        StrategyKind::Balanced,
        StrategyKind::StayAtNumberCount(3),
        StrategyKind::StayAtScore(20),
    ];
    let report = run_simulation(SimulationConfig {
        comparison_mode: ComparisonMode::HeadToHead,
        rollout_count: 3,
        player_count: 2,
        strategies,
        parallel: false,
        ..settings()
    })
    .unwrap();

    assert_eq!(report.total_match_runs, 18);
    for summary in report.summaries {
        assert_eq!(summary.matches, 12);
        assert_eq!(summary.seat_exposure, vec![6, 6]);
    }
}

#[test]
fn parallel_and_single_thread_reports_match_for_same_seed() {
    let serial = run_simulation(SimulationConfig {
        rollout_count: 12,
        parallel: false,
        ..settings()
    })
    .unwrap();
    let parallel = run_simulation(SimulationConfig {
        rollout_count: 12,
        parallel: true,
        ..settings()
    })
    .unwrap();

    assert_eq!(serial.total_match_runs, parallel.total_match_runs);
    assert_eq!(serial.total_rounds, parallel.total_rounds);
    assert_eq!(serial.total_decisions, parallel.total_decisions);
    assert_eq!(serial.summaries, parallel.summaries);
}

#[test]
fn simulation_summary_counts_matches_and_labels() {
    let report = run_simulation(SimulationConfig {
        rollout_count: 4,
        strategies: vec![StrategyKind::MaxRoundEv, StrategyKind::MaxWinProbability],
        ..settings()
    })
    .unwrap();

    assert_eq!(report.summaries.len(), 2);
    assert_eq!(report.summaries[0].strategy, StrategyKind::MaxRoundEv);
    assert_eq!(
        report.summaries[1].strategy,
        StrategyKind::MaxWinProbability
    );
    assert_eq!(report.summaries[0].matches, 8);
    assert_eq!(report.summaries[1].matches, 8);
    assert!(report.summaries[0].average_rounds_to_finish > 0.0);
    assert!(report.total_decisions > 0);
}

#[test]
fn round_limited_matches_do_not_count_as_wins_or_completed_samples() {
    let report = run_simulation(SimulationConfig {
        rollout_count: 3,
        target_score: 1_000,
        max_rounds: 1,
        ..settings()
    })
    .unwrap();
    assert_eq!(report.total_match_runs, 6);
    assert_eq!(report.completed_match_runs, 0);
    assert_eq!(report.round_limited_match_runs, 6);
    for summary in report.summaries {
        assert_eq!(summary.wins, 0);
        assert_eq!(summary.completed_matches, 0);
        assert_eq!(summary.round_limited_matches, 6);
        assert_eq!(summary.win_rate, 0.0);
        assert_eq!(summary.average_rounds_to_finish, 0.0);
        assert_eq!((summary.win_ci_lower, summary.win_ci_upper), (0.0, 1.0));
    }
}

#[test]
fn win_rates_exclude_limited_runs_when_only_some_matches_finish() {
    let report = run_simulation(SimulationConfig {
        rollout_count: 32,
        rng_seed: 7,
        target_score: 25,
        max_rounds: 1,
        ..settings()
    })
    .unwrap();
    assert!(report.completed_match_runs > 0);
    assert!(report.round_limited_match_runs > 0);
    assert_eq!(
        report.completed_match_runs + report.round_limited_match_runs,
        report.total_match_runs
    );
    assert_eq!(
        report
            .summaries
            .iter()
            .map(|summary| summary.wins)
            .sum::<usize>(),
        report.completed_match_runs
    );
    for summary in report.summaries {
        assert_eq!(summary.completed_matches, report.completed_match_runs);
        assert_eq!(
            summary.round_limited_matches,
            report.round_limited_match_runs
        );
        assert_eq!(
            summary.win_rate,
            summary.wins as f64 / summary.completed_matches as f64
        );
        assert_eq!(summary.average_rounds_to_finish, 1.0);
    }
}

#[test]
fn both_modes_stop_at_minimum_when_completed_samples_converge() {
    for comparison_mode in [ComparisonMode::MixedPool, ComparisonMode::HeadToHead] {
        let report = run_simulation(SimulationConfig {
            comparison_mode,
            rollout_count: 1,
            min_matches: Some(2),
            max_matches: Some(5),
            win_ci_width: Some(1.0),
            target_score: 1,
            ..settings()
        })
        .unwrap();
        assert_eq!(report.settings.rollout_count, 2);
        assert_eq!(report.total_match_runs, 4);
        assert_eq!(report.completed_match_runs, 4);
    }
}

#[test]
fn both_modes_run_to_maximum_when_no_match_finishes() {
    for comparison_mode in [ComparisonMode::MixedPool, ComparisonMode::HeadToHead] {
        let report = run_simulation(SimulationConfig {
            comparison_mode,
            rollout_count: 1,
            min_matches: Some(2),
            max_matches: Some(5),
            win_ci_width: Some(1.0),
            target_score: 1_000,
            max_rounds: 1,
            ..settings()
        })
        .unwrap();
        assert_eq!(report.settings.rollout_count, 5);
        assert_eq!(report.total_match_runs, 10);
        assert_eq!(report.round_limited_match_runs, 10);
    }
}

#[test]
fn incremental_and_batched_reports_match_fixed_runs_in_both_modes() {
    for comparison_mode in [ComparisonMode::MixedPool, ComparisonMode::HeadToHead] {
        let fixed_settings = SimulationConfig {
            comparison_mode,
            rollout_count: 70,
            ..settings()
        };
        let fixed = run_simulation(fixed_settings.clone()).unwrap();
        for parallel in [false, true] {
            let incremental = run_simulation(SimulationConfig {
                parallel,
                min_matches: Some(2),
                max_matches: Some(70),
                win_ci_width: Some(0.000_001),
                ..fixed_settings.clone()
            })
            .unwrap();
            assert_eq!(incremental.total_match_runs, fixed.total_match_runs);
            assert_eq!(incremental.total_rounds, fixed.total_rounds);
            assert_eq!(incremental.total_decisions, fixed.total_decisions);
            assert_eq!(incremental.summaries, fixed.summaries);
        }
        let parallel = run_simulation(SimulationConfig {
            parallel: true,
            ..fixed_settings
        })
        .unwrap();
        assert_eq!(parallel.summaries, fixed.summaries);
    }
}

#[test]
fn head_to_head_can_disable_mirrored_seating() {
    let report = run_simulation(SimulationConfig {
        comparison_mode: ComparisonMode::HeadToHead,
        rollout_count: 3,
        mirrored_seating: false,
        ..settings()
    })
    .unwrap();
    assert_eq!(report.total_match_runs, 3);
    for summary in report.summaries {
        assert_eq!(summary.matches, 3);
        assert!(summary.seat_exposure == [3, 0] || summary.seat_exposure == [0, 3]);
    }
}
