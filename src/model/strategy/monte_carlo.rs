use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::model::{GameState, PlayerId, SpecialAction};

use super::decision::AiDecision;
use super::decision::projected_player_value;
use super::report::SimulationSettings;
use super::simulator::{
    SimulatedMatch, apply_simulated_decision, finish_match_with_basic_policy, simulation_clone,
};

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
pub(super) struct MonteCarloActionEvaluation {
    pub(super) decision: AiDecision,
    pub(super) win_rate: f64,
    pub(super) average_utility: f64,
    pub(super) samples: usize,
}

pub(super) fn evaluate_monte_carlo_actions(
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

pub(super) fn mix_seed(seed: u64) -> u64 {
    let mut value = seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
