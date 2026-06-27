use std::env;
use std::process::ExitCode;

use flip_seven::model::{
    SimulationSettings, SimulationSummary, StrategyReport, compare_strategies, strategy_label,
};

fn main() -> ExitCode {
    let mut settings = SimulationSettings::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            "--rollouts" => {
                settings.rollout_count = parse_next(&arg, &mut args);
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
            unknown => {
                eprintln!("Unknown argument: {unknown}");
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    let report = compare_strategies(settings);
    print_report(&report);
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

fn print_usage() {
    println!(
        "\
Usage: cargo run --bin offline -- [options]

Runs offline Flip 7 strategy simulations against the shared model.

Options:
  --rollouts <n>           Matches to simulate
  --decision-rollouts <n>  Rollouts used for each rollout-backed decision
  --seed <n>               RNG seed
  --target-score <n>       Score needed to win
  --max-rounds <n>         Round cap per match
  -h, --help               Show this help"
    );
}

fn print_report(report: &StrategyReport) {
    println!("Offline Flip 7 strategy comparison");
    println!("matches: {}", report.settings.rollout_count);
    println!("decision rollouts: {}", report.settings.decision_rollouts);
    println!("seed: {}", report.settings.rng_seed);
    println!("target score: {}", report.settings.target_score);
    println!("max rounds: {}", report.settings.max_rounds);
    println!();
    println!(
        "{:<10} {:>12} {:>12} {:>10} {:>9} {:>9} {:>9} {:>10}",
        "Strategy", "Win", "Final", "Rounds", "Bust", "Risk", "Flip 7", "Pts/R"
    );
    println!("{}", "-".repeat(91));

    for summary in &report.summaries {
        print_summary(summary);
    }
}

fn print_summary(summary: &SimulationSummary) {
    println!(
        "{:<10} {:>5}/{:<5} {:>12.1} {:>10.1} {:>8.1}% {:>8.1}% {:>8.1}% {:>10.1}",
        strategy_label(summary.strategy),
        summary.wins,
        summary.matches,
        summary.average_final_score,
        summary.average_rounds_to_finish,
        summary.bust_rate * 100.0,
        summary.average_pre_draw_bust_risk * 100.0,
        summary.flip_seven_rate * 100.0,
        summary.average_points_per_round
    );
}
