use rand::{SeedableRng, rngs::StdRng};

use super::{game_with_draw_order, settings};
use crate::model::{Card, Deck, GameError, PlayerId};
use crate::simulation::StrategyKind;
use crate::simulation::decision::AiDecision;
use crate::simulation::simulator::{
    MatchOutcome, SimulationFailure, apply_simulated_decision, finish_match_with_strategies,
    play_round_with_strategies, simulation_clone,
};

#[test]
fn invalid_action_propagates_its_error_without_mutation() {
    let mut game = game_with_draw_order(1, vec![Card::Number(5)]);
    let before = game.clone();
    let error = apply_simulated_decision(
        &mut game,
        AiDecision::ChooseTarget(PlayerId::new(9)),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SimulationFailure::CommandRejected(GameError::InvalidTarget)
    );
    assert_eq!(game, before);
}

#[test]
fn exhausted_forced_draw_fails_instead_of_retrying_a_rejected_stay() {
    let mut game = game_with_draw_order(1, vec![Card::FlipThree]);
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(0)).unwrap();
    game.replace_deck_for_test(Deck::from_draw_order([]));
    let error = finish_match_with_strategies(
        simulation_clone(&game),
        &[StrategyKind::Balanced],
        &settings(),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SimulationFailure::CommandRejected(GameError::PendingTarget { .. })
    ));
}

#[test]
fn stalled_round_reports_decision_limit() {
    // This synthetic deck keeps re-dealing Flip Three without making scoring progress.
    let game = game_with_draw_order(1, vec![Card::FlipThree]);
    let mut replay = simulation_clone(&game);
    let error = play_round_with_strategies(
        &mut replay,
        &[StrategyKind::Balanced],
        &settings(),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap_err();
    assert_eq!(error, SimulationFailure::DecisionLimitExceeded);
}

#[test]
fn leader_below_target_at_round_limit_has_no_winner() {
    let mut game = game_with_draw_order(2, vec![Card::Number(5)]);
    game.deal_selected_card(Card::Number(5)).unwrap();
    game.stay_current_player().unwrap();
    game.stay_current_player().unwrap();
    let config = crate::simulation::SimulationConfig {
        max_rounds: 1,
        ..settings()
    };
    let result = finish_match_with_strategies(
        simulation_clone(&game),
        &[StrategyKind::Balanced; 2],
        &config,
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap();
    assert_eq!(result.outcome, MatchOutcome::RoundLimit);
    assert_eq!(result.winner(), None);
    assert_eq!(result.rounds_played, 1);
}

#[test]
fn unfinished_round_without_a_legal_actor_reports_an_error() {
    let mut game = game_with_draw_order(1, vec![]);
    game.force_bust_for_test(PlayerId::new(0));
    let error = finish_match_with_strategies(
        game,
        &[StrategyKind::Balanced],
        &settings(),
        &mut StdRng::seed_from_u64(1),
    )
    .unwrap_err();
    assert_eq!(error, SimulationFailure::NoLegalDecision);
}
