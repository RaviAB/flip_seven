use std::str::FromStr;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::prelude::*;

use super::{
    ActionChoice, DealOutcome, DrawOdds, GameState, PendingAction, PlayerId, SpecialAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StrategyKind {
    Conservative,
    Balanced,
    Aggressive,
    MaxRoundEv,
    MaxWinProbability,
    StayAtNumberCount(u8),
    StayAtScore(u32),
}

impl FromStr for StrategyKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = normalize_strategy_name(value);
        match normalized.as_str() {
            "conservative" => Ok(Self::Conservative),
            "balanced" => Ok(Self::Balanced),
            "aggressive" => Ok(Self::Aggressive),
            "maxroundev" | "roundev" | "ev" | "maxev" => Ok(Self::MaxRoundEv),
            "maxwinprobability" | "winprobability" | "winprob" | "win" => {
                Ok(Self::MaxWinProbability)
            }
            _ => parse_simple_strategy(&normalized)
                .ok_or_else(|| format!("unknown strategy '{value}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerController {
    Human,
    Ai(StrategyKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDecision {
    Stay,
    Draw,
    ChooseTarget(PlayerId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionContext {
    pub player_id: PlayerId,
    pub pending_action: Option<PendingAction>,
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
struct DecisionWithRationale {
    decision: Option<AiDecision>,
    rationale: String,
}

#[derive(Debug, Clone, Copy)]
struct MonteCarloActionStats {
    decision: AiDecision,
    wins: usize,
    utility_sum: f64,
    prior: f64,
}

impl MonteCarloActionStats {
    fn new(decision: AiDecision, prior: f64) -> Self {
        Self {
            decision,
            wins: 0,
            utility_sum: 0.0,
            prior,
        }
    }

    fn average_utility(self, samples: usize) -> f64 {
        self.utility_sum / samples.max(1) as f64
    }

    fn win_rate(self, samples: usize) -> f64 {
        self.wins as f64 / samples.max(1) as f64
    }

    fn selection_score(self, samples: usize) -> f64 {
        self.average_utility(samples) + 0.03 * self.prior
    }
}

#[derive(Debug, Clone, Copy)]
struct MonteCarloActionEvaluation {
    decision: AiDecision,
    win_rate: f64,
    average_utility: f64,
    samples: usize,
}

#[derive(Debug, Clone, Copy)]
struct WinProbabilityProfile {
    pressure: f64,
    bust_risk_cap: f64,
    building_bust_risk_cap: f64,
    flip_seven_risk_cap: f64,
    flip_seven_chase_probability: f64,
    minimum_bank_score: u32,
    required_ev_edge: f64,
}

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

pub fn recommend_decision(
    game: &GameState,
    strategy: StrategyKind,
    settings: &SimulationSettings,
) -> StrategyRecommendation {
    let Some(context) = decision_context(game) else {
        return StrategyRecommendation {
            strategy,
            decision: None,
            rationale: "No legal AI decision is pending.".to_owned(),
        };
    };

    let decision = match strategy {
        StrategyKind::Conservative | StrategyKind::Balanced | StrategyKind::Aggressive => {
            risk_profile_decision(game, &context, strategy, settings)
        }
        StrategyKind::MaxRoundEv => max_round_ev_decision(game, &context, settings),
        StrategyKind::MaxWinProbability => max_win_probability_decision(game, &context, settings),
        StrategyKind::StayAtNumberCount(_) | StrategyKind::StayAtScore(_) => {
            simple_threshold_decision(game, &context, strategy, settings)
        }
    };

    StrategyRecommendation {
        strategy,
        decision: decision.decision,
        rationale: decision.rationale,
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
    let base = (0..settings.player_count)
        .map(|index| settings.strategies[index % settings.strategies.len()])
        .collect::<Vec<_>>();
    let rotation_count = if settings.mirrored_seating {
        settings.player_count
    } else {
        1
    };

    (0..rotation_count)
        .map(|rotation| {
            (0..settings.player_count)
                .map(|seat| base[(seat + rotation) % settings.player_count])
                .collect()
        })
        .collect()
}

fn risk_profile_decision(
    game: &GameState,
    context: &DecisionContext,
    strategy: StrategyKind,
    settings: &SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_heuristic(game, context, strategy);
    }

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
                strategy_label(strategy),
                odds.bust_probability * 100.0,
                odds.expected_score_after_draw
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: {} banks {} with {:.1}% bust risk on a draw.",
                strategy_label(strategy),
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
    settings: &SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_heuristic(game, context, strategy);
    }

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
                strategy_label(strategy)
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
                strategy_label(strategy),
                odds.current_score
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: {} threshold has not been reached.",
                strategy_label(strategy)
            ),
        }
    }
}

fn max_round_ev_decision(
    game: &GameState,
    context: &DecisionContext,
    _settings: &SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_heuristic(game, context, StrategyKind::MaxRoundEv);
    }

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
    settings: &SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_rollout_value(game, context, StrategyKind::MaxWinProbability, settings);
    }

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
    settings: &SimulationSettings,
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

fn leading_opponent_score(game: &GameState, player_id: PlayerId) -> u32 {
    game.score_board()
        .iter()
        .filter(|score| score.player_id != player_id)
        .map(|score| score.total_score)
        .max()
        .unwrap_or(0)
}

fn win_probability_profile(
    leader_total: u32,
    stay_total: u32,
    settings: &SimulationSettings,
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
            special_action_label(pending_action.action()),
            strategy_label(strategy)
        ),
    }
}

fn target_by_rollout_value(
    game: &GameState,
    context: &DecisionContext,
    _strategy: StrategyKind,
    settings: &SimulationSettings,
) -> DecisionWithRationale {
    let Some(pending_action) = context.pending_action.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No target decision is pending.".to_owned(),
        };
    };

    let decisions = context
        .legal_targets
        .iter()
        .copied()
        .map(AiDecision::ChooseTarget)
        .collect::<Vec<_>>();

    let Some(evaluation) =
        evaluate_monte_carlo_actions(game, context.player_id, &decisions, settings)
    else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No active legal targets are available.".to_owned(),
        };
    };

    let target_name = match evaluation.decision {
        AiDecision::ChooseTarget(player_id) => player_name(game, player_id),
        AiDecision::Stay | AiDecision::Draw => "-".to_owned(),
    };
    let basis = format!(
        "Monte Carlo win {:.1}%, utility {:.3} over {} samples",
        evaluation.win_rate * 100.0,
        evaluation.average_utility,
        evaluation.samples
    );

    DecisionWithRationale {
        decision: Some(evaluation.decision),
        rationale: format!(
            "Choose {target_name} for {}: {basis}.",
            special_action_label(pending_action.action())
        ),
    }
}

fn evaluate_monte_carlo_actions(
    game: &GameState,
    player_id: PlayerId,
    decisions: &[AiDecision],
    settings: &SimulationSettings,
) -> Option<MonteCarloActionEvaluation> {
    if decisions.is_empty() {
        return None;
    }

    let samples = settings.decision_rollouts.max(1);
    let mut stats = decisions
        .iter()
        .copied()
        .map(|decision| {
            MonteCarloActionStats::new(
                decision,
                tactical_decision_prior(game, player_id, decision, settings),
            )
        })
        .collect::<Vec<_>>();

    for sample_index in 0..samples {
        let sample_seed = search_sample_seed(settings.rng_seed, player_id, sample_index);

        for (decision_index, decision) in decisions.iter().copied().enumerate() {
            let mut rng = StdRng::seed_from_u64(search_action_seed(sample_seed, decision_index));
            let mut candidate = simulation_clone(game);
            apply_simulated_decision(&mut candidate, decision, &mut rng);
            let result = finish_match_with_basic_policy(candidate, settings, &mut rng);
            let utility = terminal_search_utility(&result, player_id, settings.target_score);
            if result.winner == Some(player_id) {
                stats[decision_index].wins += 1;
            }
            stats[decision_index].utility_sum += utility;
        }
    }

    let best = stats.into_iter().max_by(|left, right| {
        left.selection_score(samples)
            .total_cmp(&right.selection_score(samples))
            .then_with(|| {
                left.average_utility(samples)
                    .total_cmp(&right.average_utility(samples))
            })
            .then_with(|| left.prior.total_cmp(&right.prior))
    })?;

    Some(MonteCarloActionEvaluation {
        decision: best.decision,
        win_rate: best.win_rate(samples),
        average_utility: best.average_utility(samples),
        samples,
    })
}

fn tactical_decision_prior(
    game: &GameState,
    player_id: PlayerId,
    decision: AiDecision,
    settings: &SimulationSettings,
) -> f64 {
    match decision {
        AiDecision::Stay => game
            .draw_odds_for_player(player_id)
            .map(|odds| {
                let score_to_win = game
                    .score_to_win_target(player_id, settings.target_score)
                    .unwrap_or(u32::MAX);
                let target_lock = if odds.current_score > 0 && odds.current_score >= score_to_win {
                    2.0
                } else {
                    0.0
                };
                target_lock + odds.current_score as f64 / settings.target_score.max(1) as f64
            })
            .unwrap_or(0.0),
        AiDecision::Draw => game
            .draw_odds_for_player(player_id)
            .map(|odds| {
                ((odds.expected_score_delta / 25.0) - odds.bust_probability
                    + odds.flip_seven_probability * 1.5
                    + f64::from(odds.current_score == 0) * 0.1)
                    .clamp(-1.0, 1.0)
            })
            .unwrap_or(0.0),
        AiDecision::ChooseTarget(target_id) => {
            let own_target_penalty = if game.pending_action().is_some_and(|pending| {
                pending.action() == SpecialAction::Freeze && target_id == player_id
            }) {
                -1.0
            } else {
                0.0
            };
            own_target_penalty
                + projected_player_value(game, target_id) as f64
                    / settings.target_score.max(1) as f64
        }
    }
}

fn terminal_search_utility(result: &SimulatedMatch, player_id: PlayerId, target_score: u32) -> f64 {
    let own_score = result
        .game
        .player_score(player_id)
        .map(|score| score.total_score)
        .unwrap_or(0);
    let best_other_score = result
        .game
        .score_board()
        .iter()
        .filter(|score| score.player_id != player_id)
        .map(|score| score.total_score)
        .max()
        .unwrap_or(0);
    let win_value = f64::from(result.winner == Some(player_id));
    let score_margin = (own_score as f64 - best_other_score as f64) / target_score.max(1) as f64;

    win_value + 0.12 * score_margin.clamp(-1.0, 1.0)
}

fn search_sample_seed(base_seed: u64, player_id: PlayerId, sample_index: usize) -> u64 {
    mix_seed(
        base_seed
            ^ ((player_id.index() as u64) << 32)
            ^ (sample_index as u64 + 1).wrapping_mul(0xD6E8_FD93_3135_7A9D),
    )
}

fn search_action_seed(sample_seed: u64, decision_index: usize) -> u64 {
    mix_seed(sample_seed ^ (decision_index as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407))
}

#[derive(Debug, Clone)]
struct SimulatedMatch {
    game: GameState,
    winner: Option<PlayerId>,
    rounds_played: u32,
    decisions_made: usize,
}

fn simulate_match(
    player_count: usize,
    strategies: &[StrategyKind],
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let game = simulation_clone(&GameState::with_deck(
        player_count,
        super::Deck::new_full_ordered(),
    ));
    finish_match_with_strategies(game, strategies, settings, rng)
}

fn finish_match_with_strategies(
    mut game: GameState,
    strategies: &[StrategyKind],
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let mut rounds_played = game
        .score_board()
        .iter()
        .map(|score| score.rounds_completed)
        .max()
        .unwrap_or(0) as u32;
    let mut decisions_made = 0usize;

    while winner(&game, settings.target_score).is_none() && rounds_played < settings.max_rounds {
        decisions_made += play_round_with_strategies(&mut game, strategies, settings, rng);
        if !game.round_over() {
            break;
        }

        if game.round_over() {
            rounds_played = game
                .score_board()
                .iter()
                .map(|score| score.rounds_completed)
                .max()
                .unwrap_or(rounds_played as usize) as u32;
        }

        if winner(&game, settings.target_score).is_some() || rounds_played >= settings.max_rounds {
            break;
        }

        let _ = game.start_next_round();
    }

    let winner = winner(&game, settings.target_score).or_else(|| highest_score_player(&game));
    SimulatedMatch {
        game,
        winner,
        rounds_played,
        decisions_made,
    }
}

fn play_round_with_strategies(
    game: &mut GameState,
    strategies: &[StrategyKind],
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> usize {
    let mut guard = 0usize;
    let mut decisions_made = 0usize;
    while !game.round_over() && guard < 1_000 {
        guard += 1;

        if game
            .pending_action()
            .is_some_and(PendingAction::awaiting_selected_draws)
        {
            deal_random_available_card(game, rng);
            continue;
        }

        let Some(player_id) = current_strategy_player_id(game) else {
            break;
        };
        let strategy = strategies
            .get(player_id.index())
            .copied()
            .unwrap_or(StrategyKind::Balanced);
        let recommendation = recommend_decision(game, strategy, settings);

        match recommendation.decision {
            Some(AiDecision::Stay) => {
                decisions_made += 1;
                let _ = game.stay_current_player();
            }
            Some(AiDecision::Draw) => {
                decisions_made += 1;
                deal_random_available_card(game, rng);
            }
            Some(AiDecision::ChooseTarget(target_id)) => {
                decisions_made += 1;
                let _ = game.resolve_pending_action(ActionChoice::Player(target_id));
            }
            None => break,
        }
    }
    decisions_made
}

fn finish_match_with_basic_policy(
    game: GameState,
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let strategies = vec![StrategyKind::Balanced; game.players().len()];
    finish_match_with_strategies(game, &strategies, settings, rng)
}

fn apply_simulated_decision(game: &mut GameState, decision: AiDecision, rng: &mut StdRng) {
    match decision {
        AiDecision::Stay => {
            let _ = game.stay_current_player();
        }
        AiDecision::Draw => deal_random_available_card(game, rng),
        AiDecision::ChooseTarget(player_id) => {
            let _ = game.resolve_pending_action(ActionChoice::Player(player_id));
        }
    }
}

fn deal_random_available_card(game: &mut GameState, rng: &mut StdRng) {
    let Some(card) = choose_random_available_card(game, rng) else {
        let _ = game.stay_current_player();
        return;
    };
    let outcome = game.deal_selected_card(card);
    if matches!(
        outcome,
        DealOutcome::DeckEmpty | DealOutcome::SelectedCardUnavailable
    ) {
        let _ = game.stay_current_player();
    }
}

fn simulation_clone(game: &GameState) -> GameState {
    let mut clone = game.clone();
    clone.disable_undo_tracking_for_replay();
    clone
}

fn current_strategy_player_id(game: &GameState) -> Option<PlayerId> {
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
        pending_action: game.pending_action().cloned(),
        legal_targets: game.legal_active_targets(),
        draw_odds,
    })
}

fn legal_ai_decisions(game: &GameState) -> Vec<AiDecision> {
    if game.round_over() {
        return Vec::new();
    }

    if let Some(pending_action) = game.pending_action() {
        if pending_action.needs_target() {
            return game
                .legal_active_targets()
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

fn choose_random_available_card(game: &GameState, rng: &mut StdRng) -> Option<super::Card> {
    use rand::Rng;

    let counts = game.next_draw_pool_counts();
    let total = counts.iter().map(|(_, count)| *count).sum::<usize>();
    if total == 0 {
        return None;
    }

    let mut choice = rng.random_range(0..total);
    for (card, count) in counts {
        if choice < count {
            return Some(card);
        }
        choice -= count;
    }

    None
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
        StrategyKind::MaxRoundEv | StrategyKind::MaxWinProbability => RiskProfile {
            bust_risk_cap: 1.0,
            flip_seven_risk_cap: 1.0,
            flip_seven_chase_probability: 1.0,
            minimum_bank_score: 0,
            required_ev_edge: 0.0,
        },
        StrategyKind::StayAtNumberCount(_) | StrategyKind::StayAtScore(_) => RiskProfile {
            bust_risk_cap: 1.0,
            flip_seven_risk_cap: 1.0,
            flip_seven_chase_probability: 1.0,
            minimum_bank_score: 0,
            required_ev_edge: 0.0,
        },
    }
}

fn winner(game: &GameState, target_score: u32) -> Option<PlayerId> {
    game.score_board()
        .iter()
        .filter(|score| score.total_score >= target_score)
        .max_by_key(|score| score.total_score)
        .map(|score| score.player_id)
}

fn highest_score_player(game: &GameState) -> Option<PlayerId> {
    game.score_board()
        .iter()
        .max_by_key(|score| score.total_score)
        .map(|score| score.player_id)
}

fn projected_player_value(game: &GameState, player_id: PlayerId) -> u32 {
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

fn special_action_label(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::FlipThree => "Flip Three",
        SpecialAction::Freeze => "Freeze",
    }
}

pub fn strategy_label(strategy: StrategyKind) -> String {
    match strategy {
        StrategyKind::Conservative => "Conservative".to_owned(),
        StrategyKind::Balanced => "Balanced".to_owned(),
        StrategyKind::Aggressive => "Aggressive".to_owned(),
        StrategyKind::MaxRoundEv => "MaxRoundEV".to_owned(),
        StrategyKind::MaxWinProbability => "MaxWin%".to_owned(),
        StrategyKind::StayAtNumberCount(threshold) => format!("Stay{threshold}Cards"),
        StrategyKind::StayAtScore(threshold) => format!("Stay{threshold}"),
    }
}

pub fn strategy_slug(strategy: StrategyKind) -> String {
    match strategy {
        StrategyKind::Conservative => "conservative".to_owned(),
        StrategyKind::Balanced => "balanced".to_owned(),
        StrategyKind::Aggressive => "aggressive".to_owned(),
        StrategyKind::MaxRoundEv => "max-round-ev".to_owned(),
        StrategyKind::MaxWinProbability => "max-win-probability".to_owned(),
        StrategyKind::StayAtNumberCount(threshold) => format!("stay-at-{threshold}-cards"),
        StrategyKind::StayAtScore(threshold) => format!("stay-at-{threshold}"),
    }
}

pub fn all_strategy_kinds() -> Vec<StrategyKind> {
    let mut strategies = vec![
        StrategyKind::Conservative,
        StrategyKind::Balanced,
        StrategyKind::Aggressive,
        StrategyKind::MaxRoundEv,
        StrategyKind::MaxWinProbability,
        StrategyKind::StayAtNumberCount(3),
        StrategyKind::StayAtNumberCount(4),
    ];
    strategies.extend((15..=35).map(StrategyKind::StayAtScore));
    strategies
}

pub fn human_sweep_strategy_kinds() -> Vec<StrategyKind> {
    all_strategy_kinds()
}

fn normalize_strategy_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_simple_strategy(normalized: &str) -> Option<StrategyKind> {
    if let Some(number) = normalized
        .strip_prefix("stayat")
        .and_then(|suffix| suffix.strip_suffix("cards"))
        .and_then(|number| number.parse::<u8>().ok())
        && matches!(number, 3 | 4)
    {
        return Some(StrategyKind::StayAtNumberCount(number));
    }

    if let Some(score) = normalized
        .strip_prefix("stayat")
        .and_then(|number| number.parse::<u32>().ok())
        && (15..=35).contains(&score)
    {
        return Some(StrategyKind::StayAtScore(score));
    }

    None
}

fn unique_strategies(strategies: &[StrategyKind]) -> Vec<StrategyKind> {
    let mut unique = Vec::new();
    for strategy in strategies {
        if !unique.contains(strategy) {
            unique.push(*strategy);
        }
    }
    unique
}

fn strategy_sort_key(strategy: StrategyKind) -> usize {
    all_strategy_kinds()
        .iter()
        .position(|candidate| *candidate == strategy)
        .unwrap_or(usize::MAX)
}

fn match_seed(base_seed: u64, matchup_id: u64, rotation_id: u64, match_index: u64) -> u64 {
    mix_seed(
        base_seed
            ^ matchup_id.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ rotation_id.wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ match_index.wrapping_mul(0x94D0_49BB_1331_11EB),
    )
}

fn mix_seed(seed: u64) -> u64 {
    let mut value = seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BonusCard, Card, Deck, PlayerStatus};

    fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
        GameState::with_deck(player_count, Deck::from_draw_order(cards))
    }

    fn settings() -> SimulationSettings {
        SimulationSettings {
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

    #[test]
    fn ev_ai_stays_when_current_score_beats_draw_ev() {
        let mut game = game_with_draw_order(1, vec![Card::Number(12)]);
        game.deal_selected_card(Card::Number(12));
        game.replace_deck_for_test(Deck::from_draw_order([Card::Number(12), Card::Number(1)]));

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

        assert_eq!(recommendation.decision, Some(AiDecision::Stay));
    }

    #[test]
    fn ev_ai_draws_when_draw_ev_beats_current_score() {
        let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
        game.deal_selected_card(Card::Number(1));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Bonus(BonusCard::Plus(10)),
            Card::SecondChance,
        ]));

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

        assert_eq!(
            recommendation.decision,
            Some(AiDecision::Draw),
            "{}",
            recommendation.rationale
        );
    }

    #[test]
    fn every_strategy_produces_only_legal_decisions() {
        let mut game = game_with_draw_order(3, vec![Card::Number(5)]);
        game.deal_selected_card(Card::Number(5));

        for strategy in all_strategy_kinds() {
            let recommendation = recommend_decision(&game, strategy, &settings());
            if let Some(decision) = recommendation.decision {
                assert!(legal_ai_decisions(&game).contains(&decision));
            }
        }
    }

    #[test]
    fn conservative_stays_where_aggressive_draws() {
        let mut game = game_with_draw_order(1, vec![Card::Number(10)]);
        game.deal_selected_card(Card::Number(10));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
        ]));

        let conservative = recommend_decision(&game, StrategyKind::Conservative, &settings());
        let aggressive = recommend_decision(&game, StrategyKind::Aggressive, &settings());

        assert_eq!(conservative.decision, Some(AiDecision::Stay));
        assert_eq!(aggressive.decision, Some(AiDecision::Draw));
    }

    #[test]
    fn stay_at_number_count_uses_number_card_threshold() {
        let mut below = game_with_draw_order(1, vec![Card::Number(1), Card::Number(2)]);
        below.deal_selected_card(Card::Number(1));
        below.deal_selected_card(Card::Number(2));
        below.replace_deck_for_test(Deck::from_draw_order([Card::Number(3)]));

        let mut at_three =
            game_with_draw_order(1, vec![Card::Number(1), Card::Number(2), Card::Number(3)]);
        for card in [Card::Number(1), Card::Number(2), Card::Number(3)] {
            at_three.deal_selected_card(card);
        }
        at_three.replace_deck_for_test(Deck::from_draw_order([Card::Number(4)]));

        let mut at_four = game_with_draw_order(
            1,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
            ],
        );
        for card in [
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
            Card::Number(4),
        ] {
            at_four.deal_selected_card(card);
        }
        at_four.replace_deck_for_test(Deck::from_draw_order([Card::Number(5)]));

        assert_eq!(
            recommend_decision(&below, StrategyKind::StayAtNumberCount(3), &settings()).decision,
            Some(AiDecision::Draw)
        );
        assert_eq!(
            recommend_decision(&at_three, StrategyKind::StayAtNumberCount(3), &settings()).decision,
            Some(AiDecision::Stay)
        );
        assert_eq!(
            recommend_decision(&at_three, StrategyKind::StayAtNumberCount(4), &settings()).decision,
            Some(AiDecision::Draw)
        );
        assert_eq!(
            recommend_decision(&at_four, StrategyKind::StayAtNumberCount(4), &settings()).decision,
            Some(AiDecision::Stay)
        );
    }

    #[test]
    fn stay_at_score_uses_score_threshold() {
        for threshold in [15, 25, 35] {
            let below_bonus = (threshold - 13) as u8;
            let at_bonus = (threshold - 12) as u8;

            let mut below = game_with_draw_order(
                1,
                vec![Card::Number(12), Card::Bonus(BonusCard::Plus(below_bonus))],
            );
            below.deal_selected_card(Card::Number(12));
            below.deal_selected_card(Card::Bonus(BonusCard::Plus(below_bonus)));
            below.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

            let mut at_threshold = game_with_draw_order(
                1,
                vec![Card::Number(12), Card::Bonus(BonusCard::Plus(at_bonus))],
            );
            at_threshold.deal_selected_card(Card::Number(12));
            at_threshold.deal_selected_card(Card::Bonus(BonusCard::Plus(at_bonus)));
            at_threshold.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

            assert_eq!(
                recommend_decision(
                    &below,
                    StrategyKind::StayAtScore(threshold as u32),
                    &settings()
                )
                .decision,
                Some(AiDecision::Draw)
            );
            assert_eq!(
                recommend_decision(
                    &at_threshold,
                    StrategyKind::StayAtScore(threshold as u32),
                    &settings()
                )
                .decision,
                Some(AiDecision::Stay)
            );
        }
    }

    #[test]
    fn second_chance_overrides_simple_thresholds_unless_banking_wins() {
        let mut protected = game_with_draw_order(
            1,
            vec![
                Card::Number(10),
                Card::Number(11),
                Card::Number(12),
                Card::SecondChance,
            ],
        );
        for card in [
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
            Card::SecondChance,
        ] {
            protected.deal_selected_card(card);
        }
        protected.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

        let recommendation =
            recommend_decision(&protected, StrategyKind::StayAtNumberCount(3), &settings());
        assert_eq!(recommendation.decision, Some(AiDecision::Draw));

        let mut winning = simulation_clone(&protected);
        winning.set_total_score_for_test(PlayerId::new(0), 20);
        let target_settings = SimulationSettings {
            target_score: 50,
            ..settings()
        };
        let recommendation = recommend_decision(
            &winning,
            StrategyKind::StayAtNumberCount(3),
            &target_settings,
        );
        assert_eq!(recommendation.decision, Some(AiDecision::Stay));
    }

    #[test]
    fn strategy_parsing_accepts_human_simple_slugs() {
        assert_eq!(
            "stay-at-3-cards".parse::<StrategyKind>(),
            Ok(StrategyKind::StayAtNumberCount(3))
        );
        assert_eq!(
            "stay-at-4-cards".parse::<StrategyKind>(),
            Ok(StrategyKind::StayAtNumberCount(4))
        );

        for threshold in 15..=35 {
            let slug = format!("stay-at-{threshold}");
            assert_eq!(
                slug.parse::<StrategyKind>(),
                Ok(StrategyKind::StayAtScore(threshold))
            );
        }
    }

    #[test]
    fn win_probability_pressure_profiles_are_monotonic() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let behind = win_probability_profile(120, 40, &settings);
        let close = win_probability_profile(100, 100, &settings);
        let ahead = win_probability_profile(50, 120, &settings);

        assert!(behind.pressure > close.pressure);
        assert!(close.pressure > ahead.pressure);
        assert!(behind.bust_risk_cap > close.bust_risk_cap);
        assert!(close.bust_risk_cap > ahead.bust_risk_cap);
        assert!(behind.building_bust_risk_cap > close.building_bust_risk_cap);
        assert!(close.building_bust_risk_cap > ahead.building_bust_risk_cap);
        assert!(behind.flip_seven_risk_cap > close.flip_seven_risk_cap);
        assert!(close.flip_seven_risk_cap > ahead.flip_seven_risk_cap);
        assert!(ahead.required_ev_edge > close.required_ev_edge);
        assert!(close.required_ev_edge > behind.required_ev_edge);
    }

    fn two_player_game_with_player_zero_score_pressure(
        own_total: u32,
        opponent_total: u32,
    ) -> GameState {
        let mut game = game_with_draw_order(2, vec![Card::Number(8), Card::Number(12)]);
        game.deal_selected_card(Card::Number(8));
        game.stay_current_player();
        game.deal_selected_card(Card::Number(12));
        game.set_total_score_for_test(PlayerId::new(0), own_total);
        game.set_total_score_for_test(PlayerId::new(1), opponent_total);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(8),
            Card::Number(10),
            Card::Number(11),
        ]));
        game
    }

    #[test]
    fn win_probability_draws_more_aggressively_when_behind() {
        let game = two_player_game_with_player_zero_score_pressure(50, 120);
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let recommendation = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
        assert!(recommendation.rationale.contains("pressure"));
        assert!(recommendation.rationale.contains("leading opponent 120"));
    }

    #[test]
    fn win_probability_stays_more_conservatively_when_ahead() {
        let game = two_player_game_with_player_zero_score_pressure(120, 50);
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let recommendation = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(
            recommendation.decision,
            Some(AiDecision::Stay),
            "{}",
            recommendation.rationale
        );
        assert!(recommendation.rationale.contains("pressure -"));
        assert!(recommendation.rationale.contains("leading opponent 50"));
    }

    #[test]
    fn aggressive_draws_in_high_upside_flip_seven_state() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
            ],
        );
        for card in [
            Card::Number(0),
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
            Card::Number(4),
            Card::Number(5),
        ] {
            game.deal_selected_card(card);
        }
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(6),
            Card::Number(7),
            Card::Number(8),
            Card::Number(0),
        ]));

        let recommendation = recommend_decision(&game, StrategyKind::Aggressive, &settings());

        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
    }

    #[test]
    fn target_selection_uses_only_active_legal_targets() {
        let mut game = game_with_draw_order(3, vec![Card::Freeze]);
        game.deal_selected_card(Card::Freeze);
        game.force_bust_for_test(PlayerId::new(2));

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

        let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
            panic!("expected target decision");
        };
        assert_ne!(target_id, PlayerId::new(2));
        assert!(game.legal_active_targets().contains(&target_id));
    }

    #[test]
    fn win_probability_target_selection_reports_monte_carlo_search() {
        let mut game = game_with_draw_order(3, vec![Card::Freeze]);
        game.deal_selected_card(Card::Freeze);

        let recommendation =
            recommend_decision(&game, StrategyKind::MaxWinProbability, &settings());

        let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
            panic!("expected target decision");
        };
        assert!(game.legal_active_targets().contains(&target_id));
        assert!(recommendation.rationale.contains("Monte Carlo win"));
        assert!(recommendation.rationale.contains("samples"));
    }

    #[test]
    fn flip_three_ai_target_selection_keeps_card_selection_manual_in_live_mode() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
            ],
        );
        game.deal_selected_card(Card::FlipThree);

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());
        let Some(decision @ AiDecision::ChooseTarget(_)) = recommendation.decision else {
            panic!("expected target decision");
        };
        let outcome = match decision {
            AiDecision::ChooseTarget(player_id) => {
                game.resolve_pending_action(ActionChoice::Player(player_id))
            }
            AiDecision::Stay => game.stay_current_player(),
            AiDecision::Draw => panic!("target selection should not recommend draw"),
        };

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        assert!(
            game.pending_action()
                .is_some_and(PendingAction::awaiting_selected_draws)
        );
        assert_eq!(game.players()[1].hand().len(), 0);
    }

    #[test]
    fn simulations_are_reproducible_with_fixed_seed() {
        let first = compare_strategies(settings());
        let second = compare_strategies(settings());

        assert_eq!(first, second);
    }

    #[test]
    fn mirrored_seating_gives_equal_seat_exposure() {
        let report = compare_strategies(SimulationSettings {
            rollout_count: 3,
            player_count: 4,
            strategies: vec![
                StrategyKind::Conservative,
                StrategyKind::Balanced,
                StrategyKind::Aggressive,
                StrategyKind::MaxRoundEv,
            ],
            ..settings()
        });

        for summary in report.summaries {
            assert_eq!(summary.seat_exposure, vec![3, 3, 3, 3]);
        }
    }

    #[test]
    fn human_sweep_round_robin_gives_equal_mirrored_exposure() {
        let strategies = vec![
            StrategyKind::Balanced,
            StrategyKind::StayAtNumberCount(3),
            StrategyKind::StayAtScore(20),
        ];
        let report = compare_head_to_head_strategies(SimulationSettings {
            rollout_count: 3,
            player_count: 2,
            strategies,
            parallel: false,
            ..settings()
        });

        assert_eq!(report.total_match_runs, 18);
        for summary in report.summaries {
            assert_eq!(summary.matches, 12);
            assert_eq!(summary.seat_exposure, vec![6, 6]);
        }
    }

    #[test]
    fn parallel_and_single_thread_reports_match_for_same_seed() {
        let serial = compare_strategies(SimulationSettings {
            rollout_count: 12,
            parallel: false,
            ..settings()
        });
        let parallel = compare_strategies(SimulationSettings {
            rollout_count: 12,
            parallel: true,
            ..settings()
        });

        assert_eq!(serial.total_match_runs, parallel.total_match_runs);
        assert_eq!(serial.total_rounds, parallel.total_rounds);
        assert_eq!(serial.total_decisions, parallel.total_decisions);
        assert_eq!(serial.summaries, parallel.summaries);
    }

    #[test]
    fn simulation_summary_counts_matches_and_labels() {
        let report = compare_strategies(SimulationSettings {
            rollout_count: 4,
            strategies: vec![StrategyKind::MaxRoundEv, StrategyKind::MaxWinProbability],
            ..settings()
        });

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
    fn win_probability_ai_can_choose_different_action_than_ev_ai() {
        let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
        game.deal_selected_card(Card::Number(1));
        game.set_total_score_for_test(PlayerId::new(0), 49);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
        ]));
        let settings = SimulationSettings {
            target_score: 50,
            decision_rollouts: 1,
            max_rounds: 6,
            ..settings()
        };

        let ev = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings);
        let win = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(ev.decision, Some(AiDecision::Draw));
        assert_eq!(win.decision, Some(AiDecision::Stay));
        assert!(win.rationale.contains("reaches"));
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
    }

    #[test]
    fn monte_carlo_search_returns_stable_metadata() {
        let mut game = game_with_draw_order(2, vec![Card::Number(5)]);
        game.deal_selected_card(Card::Number(5));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(6),
            Card::Number(7),
            Card::Number(5),
        ]));
        let settings = SimulationSettings {
            decision_rollouts: 3,
            ..settings()
        };
        let legal = legal_ai_decisions(&game);

        let first =
            evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings).unwrap();
        let second =
            evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings).unwrap();

        assert_eq!(first.decision, second.decision);
        assert_eq!(first.samples, 3);
        assert!((0.0..=1.0).contains(&first.win_rate));
        assert!(legal.contains(&first.decision));
    }
}
