use rand::rngs::StdRng;

use crate::model::{Card, Deck, GameError, GameState, PendingStep, PlayerId};

use super::config::SimulationConfig;
use super::decision::{AiDecision, current_strategy_player_id, recommend_decision};
use super::kind::StrategyKind;

const MAX_ROUND_STEPS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MatchOutcome {
    Winner(PlayerId),
    RoundLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SimulationFailure {
    CommandRejected(GameError),
    NoLegalDecision,
    MissingStrategy(PlayerId),
    DecisionLimitExceeded,
}

impl std::fmt::Display for SimulationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandRejected(error) => write!(f, "game command rejected: {error:?}"),
            Self::NoLegalDecision => f.write_str("unfinished round has no legal decision"),
            Self::MissingStrategy(id) => {
                write!(f, "no strategy for player {}", id.display_number())
            }
            Self::DecisionLimitExceeded => write!(f, "round exceeded {MAX_ROUND_STEPS} steps"),
        }
    }
}

impl From<GameError> for SimulationFailure {
    fn from(error: GameError) -> Self {
        Self::CommandRejected(error)
    }
}

#[derive(Debug, Clone)]
pub(super) struct SimulatedMatch {
    pub(super) game: GameState,
    pub(super) outcome: MatchOutcome,
    pub(super) rounds_played: u32,
    pub(super) decisions_made: usize,
}

impl SimulatedMatch {
    pub(super) fn winner(&self) -> Option<PlayerId> {
        match self.outcome {
            MatchOutcome::Winner(id) => Some(id),
            MatchOutcome::RoundLimit => None,
        }
    }
}

pub(super) fn simulate_match(
    player_count: usize,
    strategies: &[StrategyKind],
    settings: &SimulationConfig,
    rng: &mut StdRng,
) -> Result<SimulatedMatch, SimulationFailure> {
    let game = simulation_clone(&GameState::with_deck(
        player_count,
        Deck::new_full_ordered(),
    ));
    finish_match_with_strategies(game, strategies, settings, rng)
}

pub(super) fn finish_match_with_strategies(
    mut game: GameState,
    strategies: &[StrategyKind],
    settings: &SimulationConfig,
    rng: &mut StdRng,
) -> Result<SimulatedMatch, SimulationFailure> {
    let mut decisions_made = 0;
    loop {
        let rounds_played = game
            .score_board()
            .iter()
            .map(|score| score.rounds_completed)
            .max()
            .unwrap_or(0) as u32;
        let outcome = if let Some(player_id) = winner(&game, settings.target_score) {
            Some(MatchOutcome::Winner(player_id))
        } else if rounds_played >= settings.max_rounds {
            Some(MatchOutcome::RoundLimit)
        } else {
            None
        };
        if let Some(outcome) = outcome {
            return Ok(SimulatedMatch {
                game,
                outcome,
                rounds_played,
                decisions_made,
            });
        }
        if game.round_over() {
            game.start_next_round()?;
        }
        decisions_made += play_round_with_strategies(&mut game, strategies, settings, rng)?;
    }
}

pub(super) fn play_round_with_strategies(
    game: &mut GameState,
    strategies: &[StrategyKind],
    settings: &SimulationConfig,
    rng: &mut StdRng,
) -> Result<usize, SimulationFailure> {
    let mut decisions_made = 0;
    for _ in 0..MAX_ROUND_STEPS {
        if game.round_over() {
            return Ok(decisions_made);
        }
        if game
            .pending_action()
            .is_some_and(PendingStep::awaiting_selected_draws)
        {
            deal_random_available_card(game, rng)?;
            continue;
        }
        let player_id =
            current_strategy_player_id(game).ok_or(SimulationFailure::NoLegalDecision)?;
        let strategy = strategies
            .get(player_id.index())
            .copied()
            .ok_or(SimulationFailure::MissingStrategy(player_id))?;
        let decision = recommend_decision(game, strategy, settings)?
            .decision
            .ok_or(SimulationFailure::NoLegalDecision)?;
        apply_simulated_decision(game, decision, rng)?;
        decisions_made += 1;
    }
    if game.round_over() {
        Ok(decisions_made)
    } else {
        Err(SimulationFailure::DecisionLimitExceeded)
    }
}

pub(super) fn finish_match_with_basic_policy(
    game: GameState,
    settings: &SimulationConfig,
    rng: &mut StdRng,
) -> Result<SimulatedMatch, SimulationFailure> {
    let strategies = vec![StrategyKind::Balanced; game.players().len()];
    finish_match_with_strategies(game, &strategies, settings, rng)
}

pub(super) fn apply_simulated_decision(
    game: &mut GameState,
    decision: AiDecision,
    rng: &mut StdRng,
) -> Result<(), SimulationFailure> {
    match decision {
        AiDecision::Stay => {
            game.stay_current_player()?;
        }
        AiDecision::Draw => return deal_random_available_card(game, rng),
        AiDecision::ChooseTarget(player_id) => {
            game.choose_target(player_id)?;
        }
    }
    Ok(())
}

fn deal_random_available_card(
    game: &mut GameState,
    rng: &mut StdRng,
) -> Result<(), SimulationFailure> {
    if let Some(card) = choose_random_available_card(game, rng) {
        game.deal_selected_card(card)?;
    } else {
        // An ordinary turn can bank when the pool is empty. A forced draw cannot:
        // propagate the rejected stay instead of retrying the same pending action.
        game.stay_current_player()?;
    }
    Ok(())
}

pub(super) fn simulation_clone(game: &GameState) -> GameState {
    game.clone_without_undo()
}

fn choose_random_available_card(game: &GameState, rng: &mut StdRng) -> Option<Card> {
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

fn winner(game: &GameState, target_score: u32) -> Option<PlayerId> {
    unique_highest_score_player(game, target_score)
}

fn unique_highest_score_player(game: &GameState, minimum_score: u32) -> Option<PlayerId> {
    let highest_score = game
        .score_board()
        .iter()
        .map(|score| score.total_score)
        .max()?;
    if highest_score < minimum_score {
        return None;
    }

    let mut leaders = game
        .score_board()
        .iter()
        .filter(|score| score.total_score == highest_score);
    let leader = leaders.next()?;
    leaders.next().is_none().then_some(leader.player_id)
}

#[cfg(test)]
mod tests {
    use super::winner;
    use crate::model::{Deck, GameState, PlayerId};

    #[test]
    fn winning_score_tie_requires_another_round() {
        let mut game = GameState::with_deck(3, Deck::from_draw_order([]));
        game.set_total_score_for_test(PlayerId::new(0), 200);
        game.set_total_score_for_test(PlayerId::new(1), 200);
        game.set_total_score_for_test(PlayerId::new(2), 150);

        assert_eq!(winner(&game, 200), None);

        game.set_total_score_for_test(PlayerId::new(1), 201);
        assert_eq!(winner(&game, 200), Some(PlayerId::new(1)));
    }
}
