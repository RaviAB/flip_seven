use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use flip_seven::simulation::{
    ComparisonMode, SimulationConfig, SimulationReport, SimulationSummary, StrategyKind,
    run_simulation,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(about = "Run offline Flip Seven strategy simulations")]
struct Cli {
    #[arg(long = "matches", alias = "rollouts")]
    rollout_count: Option<usize>,
    #[arg(long = "players")]
    player_count: Option<usize>,
    #[arg(long, value_parser = parse_strategies)]
    strategies: Option<Vec<StrategyKind>>,
    #[arg(long, value_enum)]
    preset: Option<Preset>,
    #[arg(long)]
    decision_rollouts: Option<usize>,
    #[arg(long = "seed")]
    rng_seed: Option<u64>,
    #[arg(long)]
    target_score: Option<u32>,
    #[arg(long)]
    max_rounds: Option<u32>,
    #[arg(long)]
    min_matches: Option<usize>,
    #[arg(long)]
    max_matches: Option<usize>,
    #[arg(long)]
    win_ci_width: Option<f64>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
    #[arg(long)]
    no_mirrored_seating: bool,
    #[arg(long)]
    single_thread: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Table,
    Csv,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Preset {
    HumanSweep,
    MaxWinStaticCalibration,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct Benchmark {
    elapsed_seconds: f64,
    matches_per_second: f64,
    rounds_per_second: f64,
    decisions_per_second: f64,
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    settings: &'a SimulationConfig,
    benchmark: Benchmark,
    total_match_runs: usize,
    completed_match_runs: usize,
    round_limited_match_runs: usize,
    total_rounds: u64,
    total_decisions: u64,
    summaries: &'a [SimulationSummary],
}

#[derive(Serialize)]
struct CsvRow {
    strategy: String,
    matches: usize,
    completed_matches: usize,
    round_limited_matches: usize,
    wins: usize,
    win_rate: f64,
    win_ci_lower: f64,
    win_ci_upper: f64,
    average_final_score: f64,
    final_score_standard_error: f64,
    average_rounds: f64,
    bust_rate: f64,
    average_pre_draw_bust_risk: f64,
    flip_seven_rate: f64,
    points_per_round: f64,
    total_match_runs: usize,
    completed_match_runs: usize,
    round_limited_match_runs: usize,
    total_rounds: u64,
    total_decisions: u64,
    elapsed_seconds: f64,
    matches_per_second: f64,
    rounds_per_second: f64,
    decisions_per_second: f64,
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn execute(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = SimulationConfig::default();
    if let Some(value) = cli.rollout_count {
        config.rollout_count = value;
    }
    if let Some(value) = cli.player_count {
        config.player_count = value;
    }
    if let Some(value) = cli.strategies {
        config.strategies = value;
    }
    if let Some(value) = cli.decision_rollouts {
        config.decision_rollouts = value;
    }
    if let Some(value) = cli.rng_seed {
        config.rng_seed = value;
    }
    if let Some(value) = cli.target_score {
        config.target_score = value;
    }
    if let Some(value) = cli.max_rounds {
        config.max_rounds = value;
    }
    config.min_matches = cli.min_matches;
    config.max_matches = cli.max_matches;
    config.win_ci_width = cli.win_ci_width;
    config.mirrored_seating = !cli.no_mirrored_seating;
    config.parallel = !cli.single_thread;

    match cli.preset {
        Some(Preset::HumanSweep) => {
            config.comparison_mode = ComparisonMode::HeadToHead;
            config.player_count = 2;
            config.strategies = StrategyKind::human_sweep_catalog();
            config.mirrored_seating = true;
        }
        Some(Preset::MaxWinStaticCalibration) => {
            config.player_count = 4;
            config.strategies = vec![
                StrategyKind::MaxWinStatic,
                StrategyKind::MaxRoundEv,
                StrategyKind::Balanced,
                StrategyKind::Aggressive,
                StrategyKind::MaxWinProbability,
                StrategyKind::StayAtScore(27),
                StrategyKind::StayAtScore(28),
                StrategyKind::StayAtScore(29),
                StrategyKind::StayAtScore(25),
            ];
            config.mirrored_seating = true;
        }
        None => {}
    }

    let started = Instant::now();
    let report = run_simulation(config)?;
    let elapsed_seconds = started.elapsed().as_secs_f64().max(f64::EPSILON);
    let benchmark = Benchmark {
        elapsed_seconds,
        matches_per_second: report.total_match_runs as f64 / elapsed_seconds,
        rounds_per_second: report.total_rounds as f64 / elapsed_seconds,
        decisions_per_second: report.total_decisions as f64 / elapsed_seconds,
    };
    match cli.format {
        OutputFormat::Table => print_table(&report, benchmark),
        OutputFormat::Csv => print_csv(&report, benchmark)?,
        OutputFormat::Json => serde_json::to_writer_pretty(
            std::io::stdout(),
            &JsonOutput {
                settings: &report.settings,
                benchmark,
                total_match_runs: report.total_match_runs,
                completed_match_runs: report.completed_match_runs,
                round_limited_match_runs: report.round_limited_match_runs,
                total_rounds: report.total_rounds,
                total_decisions: report.total_decisions,
                summaries: &report.summaries,
            },
        )?,
    }
    Ok(())
}

fn parse_strategies(value: &str) -> Result<Vec<StrategyKind>, String> {
    let strategies = value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()?;
    if strategies.is_empty() {
        Err("at least one strategy is required".to_owned())
    } else {
        Ok(strategies)
    }
}

fn print_table(report: &SimulationReport, benchmark: Benchmark) {
    println!("Offline Flip Seven strategy comparison");
    println!(
        "match runs: {} | completed: {} | round limit: {} | players: {} | seed: {}",
        report.total_match_runs,
        report.completed_match_runs,
        report.round_limited_match_runs,
        report.settings.player_count,
        report.settings.rng_seed
    );
    println!(
        "elapsed: {:.3}s | matches/sec: {:.0}",
        benchmark.elapsed_seconds, benchmark.matches_per_second
    );
    println!(
        "{:<16} {:>8} {:>8} {:>8} {:>8} {:>8} {:>12} {:>9} {:>9}",
        "Strategy", "Matches", "Done", "Limited", "Wins", "Win%", "Final", "Bust", "Pts/R"
    );
    for summary in &report.summaries {
        print_table_row(summary);
    }
}

fn print_table_row(summary: &SimulationSummary) {
    let win_percent = if summary.completed_matches == 0 {
        "-".to_owned()
    } else {
        format!("{:.1}%", summary.win_rate * 100.0)
    };
    println!(
        "{:<16} {:>8} {:>8} {:>8} {:>8} {:>8} {:>12.1} {:>8.1}% {:>9.1}",
        summary.strategy.label(),
        summary.matches,
        summary.completed_matches,
        summary.round_limited_matches,
        summary.wins,
        win_percent,
        summary.average_final_score,
        summary.bust_rate * 100.0,
        summary.average_points_per_round
    );
}

fn print_csv(report: &SimulationReport, benchmark: Benchmark) -> csv::Result<()> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    for summary in &report.summaries {
        writer.serialize(CsvRow {
            strategy: summary.strategy.slug(),
            matches: summary.matches,
            completed_matches: summary.completed_matches,
            round_limited_matches: summary.round_limited_matches,
            wins: summary.wins,
            win_rate: summary.win_rate,
            win_ci_lower: summary.win_ci_lower,
            win_ci_upper: summary.win_ci_upper,
            average_final_score: summary.average_final_score,
            final_score_standard_error: summary.final_score_standard_error,
            average_rounds: summary.average_rounds_to_finish,
            bust_rate: summary.bust_rate,
            average_pre_draw_bust_risk: summary.average_pre_draw_bust_risk,
            flip_seven_rate: summary.flip_seven_rate,
            points_per_round: summary.average_points_per_round,
            total_match_runs: report.total_match_runs,
            completed_match_runs: report.completed_match_runs,
            round_limited_match_runs: report.round_limited_match_runs,
            total_rounds: report.total_rounds,
            total_decisions: report.total_decisions,
            elapsed_seconds: benchmark.elapsed_seconds,
            matches_per_second: benchmark.matches_per_second,
            rounds_per_second: benchmark.rounds_per_second,
            decisions_per_second: benchmark.decisions_per_second,
        })?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, OutputFormat};

    #[test]
    fn parses_existing_flags() {
        let cli = Cli::try_parse_from([
            "offline",
            "--matches",
            "2",
            "--players",
            "3",
            "--format",
            "json",
        ])
        .unwrap();
        assert_eq!(cli.rollout_count, Some(2));
        assert_eq!(cli.player_count, Some(3));
        assert_eq!(cli.format, OutputFormat::Json);
    }

    #[test]
    fn rejects_unknown_strategy() {
        assert!(Cli::try_parse_from(["offline", "--strategies", "nope"]).is_err());
    }
}
