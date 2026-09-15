use rand::rngs::StdRng;

use crate::model::{ActionChoice, Card, DealOutcome, Deck, GameState, PendingAction, PlayerId};

use super::decision::{AiDecision, current_strategy_player_id, recommend_decision};
use super::kind::StrategyKind;
use super::report::SimulationSettings;

#[derive(Debug, Clone)]
pub(super) struct SimulatedMatch {
    pub(super) game: GameState,
    pub(super) winner: Option<PlayerId>,
    pub(super) rounds_played: u32,
    pub(super) decisions_made: usize,
}

pub(super) fn simulate_match(
    player_count: usize,
    strategies: &[StrategyKind],
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let game = simulation_clone(&GameState::with_deck(
        player_count,
        Deck::new_full_ordered(),
    ));
    finish_match_with_strategies(game, strategies, settings, rng)
}

pub(super) fn finish_match_with_strategies(
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

pub(super) fn play_round_with_strategies(
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

pub(super) fn finish_match_with_basic_policy(
    game: GameState,
    settings: &SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let strategies = vec![StrategyKind::Balanced; game.players().len()];
    finish_match_with_strategies(game, &strategies, settings, rng)
}

pub(super) fn apply_simulated_decision(
    game: &mut GameState,
    decision: AiDecision,
    rng: &mut StdRng,
) {
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

pub(super) fn simulation_clone(game: &GameState) -> GameState {
    let mut clone = game.clone();
    clone.disable_undo_tracking();
    clone
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
