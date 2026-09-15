use std::env;
use std::process::ExitCode;
use std::time::Instant;

use flip_seven::model::{
    SimulationSettings, SimulationSummary, StrategyKind, StrategyReport, all_strategy_kinds,
    compare_head_to_head_strategies, compare_strategies, human_sweep_strategy_kinds,
    strategy_label, strategy_slug,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Table,
    Csv,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    HumanSweep,
    MaxWinStaticCalibration,
}

#[derive(Debug, Clone, Copy)]
struct Benchmark {
    elapsed_seconds: f64,
    matches_per_second: f64,
    rounds_per_second: f64,
    decisions_per_second: f64,
}

fn main() -> ExitCode {
    let mut settings = SimulationSettings::default();
    let mut format = OutputFormat::Table;
    let mut preset = None;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            "--matches" | "--rollouts" => {
                settings.rollout_count = parse_next(&arg, &mut args);
            }
            "--players" => {
                settings.player_count = parse_next(&arg, &mut args);
            }
            "--strategies" => {
                let value: String = parse_next(&arg, &mut args);
                settings.strategies = parse_strategies(&value).unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2);
                });
            }
            "--preset" => {
                let value: String = parse_next(&arg, &mut args);
                preset = Some(parse_preset(&value).unwrap_or_else(|| {
                    eprintln!("Invalid preset: {value}");
                    std::process::exit(2);
                }));
            }
            "--decision-rollouts" => {
                settings.decision_rollouts = parse_next(&arg, &mut args);
            }
            "--seed" => {
                settings.rng_seed = parse_next(&arg, &mut args);
            }
            "--target-score" => {
                settings.target_score = parse_next(&arg, &mut args);
            }
            "--max-rounds" => {
                settings.max_rounds = parse_next(&arg, &mut args);
            }
            "--min-matches" => {
                settings.min_matches = Some(parse_next(&arg, &mut args));
            }
            "--max-matches" => {
                settings.max_matches = Some(parse_next(&arg, &mut args));
            }
            "--win-ci-width" => {
                settings.win_ci_width = Some(parse_next(&arg, &mut args));
            }
            "--format" => {
                let value: String = parse_next(&arg, &mut args);
                format = parse_format(&value).unwrap_or_else(|| {
                    eprintln!("Invalid format: {value}");
                    std::process::exit(2);
                });
            }
            "--no-mirrored-seating" => {
                settings.mirrored_seating = false;
            }
            "--single-thread" => {
                settings.parallel = false;
            }
            unknown => {
                eprintln!("Unknown argument: {unknown}");
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    if preset == Some(Preset::HumanSweep) {
        settings.player_count = 2;
        settings.strategies = human_sweep_strategy_kinds();
        settings.mirrored_seating = true;
    } else if preset == Some(Preset::MaxWinStaticCalibration) {
        settings.player_count = 4;
        settings.strategies = vec![
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
        settings.mirrored_seating = true;
    }

    let started = Instant::now();
    let report = if preset == Some(Preset::HumanSweep) {
        compare_head_to_head_strategies(settings)
    } else {
        compare_strategies(settings)
    };
    let elapsed_seconds = started.elapsed().as_secs_f64().max(f64::EPSILON);
    let benchmark = Benchmark {
        elapsed_seconds,
        matches_per_second: report.total_match_runs as f64 / elapsed_seconds,
        rounds_per_second: report.total_rounds as f64 / elapsed_seconds,
        decisions_per_second: report.total_decisions as f64 / elapsed_seconds,
    };

    match format {
        OutputFormat::Table => print_table_report(&report, benchmark),
        OutputFormat::Csv => print_csv_report(&report, benchmark),
        OutputFormat::Json => print_json_report(&report, benchmark),
    }
    ExitCode::SUCCESS
}

fn parse_next<T>(flag: &str, args: &mut impl Iterator<Item = String>) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = args.next().unwrap_or_else(|| {
        eprintln!("Missing value for {flag}");
        std::process::exit(2);
    });

    value.parse::<T>().unwrap_or_else(|error| {
        eprintln!("Invalid value for {flag}: {value} ({error})");
        std::process::exit(2);
    })
}

fn parse_strategies(value: &str) -> Result<Vec<StrategyKind>, String> {
    let strategies = value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(str::parse::<StrategyKind>)
        .collect::<Result<Vec<_>, _>>()?;

    if strategies.is_empty() {
        return Err("--strategies must include at least one strategy".to_owned());
    }

    Ok(strategies)
}

fn parse_format(value: &str) -> Option<OutputFormat> {
    match value {
        "table" => Some(OutputFormat::Table),
        "csv" => Some(OutputFormat::Csv),
        "json" => Some(OutputFormat::Json),
        _ => None,
    }
}

fn parse_preset(value: &str) -> Option<Preset> {
    match value {
        "human-sweep" => Some(Preset::HumanSweep),
        "max-win-static-calibration" => Some(Preset::MaxWinStaticCalibration),
        _ => None,
    }
}

fn print_usage() {
    let strategies = all_strategy_kinds()
        .iter()
        .map(|strategy| strategy_slug(*strategy))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "\
Usage: cargo run --release --bin offline -- [options]

Runs offline Flip 7 strategy simulations against the shared model.

Options:
  --matches <n>            Base matches to simulate per seating rotation
  --rollouts <n>           Alias for --matches
  --players <n>            Player count
  --strategies <list>      Comma-separated strategies. Available: {strategies}
  --preset <name>          Run a named benchmark preset: human-sweep, max-win-static-calibration
  --decision-rollouts <n>  Rollouts used for each rollout-backed decision
  --seed <n>               RNG seed
  --target-score <n>       Score needed to win
  --max-rounds <n>         Round cap per match
  --min-matches <n>        Convergence mode starting match count
  --max-matches <n>        Convergence mode maximum match count
  --win-ci-width <n>       Stop convergence when widest 95% win CI is at or below n
  --format <table|csv|json>
  --no-mirrored-seating    Disable default seat rotations
  --single-thread          Disable parallel match execution
  -h, --help               Show this help"
    );
}

fn print_table_report(report: &StrategyReport, benchmark: Benchmark) {
    println!("Offline Flip 7 strategy comparison");
    println!("base matches: {}", report.settings.rollout_count);
    println!("match runs: {}", report.total_match_runs);
    println!("players: {}", report.settings.player_count);
    println!(
        "strategies: {}",
        report
            .settings
            .strategies
            .iter()
            .map(|strategy| strategy_slug(*strategy))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("mirrored seating: {}", report.settings.mirrored_seating);
    println!("decision rollouts: {}", report.settings.decision_rollouts);
    println!("seed: {}", report.settings.rng_seed);
    println!("target score: {}", report.settings.target_score);
    println!("max rounds: {}", report.settings.max_rounds);
    println!(
        "elapsed: {:.3}s | matches/sec: {:.0} | rounds/sec: {:.0} | decisions/sec: {:.0}",
        benchmark.elapsed_seconds,
        benchmark.matches_per_second,
        benchmark.rounds_per_second,
        benchmark.decisions_per_second
    );
    println!();
    println!(
        "{:<14} {:>8} {:>8} {:>8} {:>16} {:>12} {:>10} {:>9} {:>9} {:>9} {:>10}",
        "Strategy",
        "Matches",
        "Wins",
        "Win%",
        "95% CI",
        "Final",
        "SE",
        "Rounds",
        "Bust",
        "Flip 7",
        "Pts/R"
    );
    println!("{}", "-".repeat(130));

    for summary in &report.summaries {
        print_table_summary(summary);
    }
}

fn print_table_summary(summary: &SimulationSummary) {
    println!(
        "{:<14} {:>8} {:>8} {:>7.1}% {:>6.1}-{:<6.1} {:>12.1} {:>10.2} {:>9.1} {:>8.1}% {:>8.1}% {:>10.1}",
        strategy_label(summary.strategy),
        summary.matches,
        summary.wins,
        summary.win_rate * 100.0,
        summary.win_ci_lower * 100.0,
        summary.win_ci_upper * 100.0,
        summary.average_final_score,
        summary.final_score_standard_error,
        summary.average_rounds_to_finish,
        summary.bust_rate * 100.0,
        summary.flip_seven_rate * 100.0,
        summary.average_points_per_round
    );
}

fn print_csv_report(report: &StrategyReport, benchmark: Benchmark) {
    println!(
        "strategy,matches,wins,win_rate,win_ci_lower,win_ci_upper,average_final_score,final_score_standard_error,average_rounds,bust_rate,average_pre_draw_bust_risk,flip_seven_rate,points_per_round,total_match_runs,total_rounds,total_decisions,elapsed_seconds,matches_per_second,rounds_per_second,decisions_per_second"
    );
    for summary in &report.summaries {
        println!(
            "{},{},{},{:.8},{:.8},{:.8},{:.4},{:.4},{:.4},{:.8},{:.8},{:.8},{:.4},{},{},{},{:.6},{:.4},{:.4},{:.4}",
            strategy_slug(summary.strategy),
            summary.matches,
            summary.wins,
            summary.win_rate,
            summary.win_ci_lower,
            summary.win_ci_upper,
            summary.average_final_score,
            summary.final_score_standard_error,
            summary.average_rounds_to_finish,
            summary.bust_rate,
            summary.average_pre_draw_bust_risk,
            summary.flip_seven_rate,
            summary.average_points_per_round,
            report.total_match_runs,
            report.total_rounds,
            report.total_decisions,
            benchmark.elapsed_seconds,
            benchmark.matches_per_second,
            benchmark.rounds_per_second,
            benchmark.decisions_per_second
        );
    }
}

fn print_json_report(report: &StrategyReport, benchmark: Benchmark) {
    println!("{{");
    println!("  \"settings\": {{");
    println!("    \"matches\": {},", report.settings.rollout_count);
    println!("    \"players\": {},", report.settings.player_count);
    println!(
        "    \"strategies\": [{}],",
        json_strategy_list(&report.settings.strategies)
    );
    println!(
        "    \"mirrored_seating\": {},",
        report.settings.mirrored_seating
    );
    println!(
        "    \"decision_rollouts\": {},",
        report.settings.decision_rollouts
    );
    println!("    \"seed\": {},", report.settings.rng_seed);
    println!("    \"target_score\": {},", report.settings.target_score);
    println!("    \"max_rounds\": {}", report.settings.max_rounds);
    println!("  }},");
    println!("  \"benchmark\": {{");
    println!("    \"elapsed_seconds\": {:.6},", benchmark.elapsed_seconds);
    println!(
        "    \"matches_per_second\": {:.4},",
        benchmark.matches_per_second
    );
    println!(
        "    \"rounds_per_second\": {:.4},",
        benchmark.rounds_per_second
    );
    println!(
        "    \"decisions_per_second\": {:.4},",
        benchmark.decisions_per_second
    );
    println!("    \"total_match_runs\": {},", report.total_match_runs);
    println!("    \"total_rounds\": {},", report.total_rounds);
    println!("    \"total_decisions\": {}", report.total_decisions);
    println!("  }},");
    println!("  \"summaries\": [");
    for (index, summary) in report.summaries.iter().enumerate() {
        let comma = if index + 1 == report.summaries.len() {
            ""
        } else {
            ","
        };
        println!(
            "    {{\"strategy\":\"{}\",\"matches\":{},\"wins\":{},\"win_rate\":{:.8},\"win_ci_lower\":{:.8},\"win_ci_upper\":{:.8},\"average_final_score\":{:.4},\"final_score_standard_error\":{:.4},\"average_rounds\":{:.4},\"bust_rate\":{:.8},\"average_pre_draw_bust_risk\":{:.8},\"flip_seven_rate\":{:.8},\"points_per_round\":{:.4},\"seat_exposure\":[{}]}}{}",
            strategy_slug(summary.strategy),
            summary.matches,
            summary.wins,
            summary.win_rate,
            summary.win_ci_lower,
            summary.win_ci_upper,
            summary.average_final_score,
            summary.final_score_standard_error,
            summary.average_rounds_to_finish,
            summary.bust_rate,
            summary.average_pre_draw_bust_risk,
            summary.flip_seven_rate,
            summary.average_points_per_round,
            summary
                .seat_exposure
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn json_strategy_list(strategies: &[StrategyKind]) -> String {
    strategies
        .iter()
        .map(|strategy| format!("\"{}\"", strategy_slug(*strategy)))
        .collect::<Vec<_>>()
        .join(",")
}
