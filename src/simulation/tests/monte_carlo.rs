use super::{game_with_draw_order, settings};
use crate::model::{Card, Deck, GameError, PlayerId};
use crate::simulation::SimulationConfig;
use crate::simulation::StrategyKind;
use crate::simulation::decision::{AiDecision, legal_ai_decisions, recommend_decision};
use crate::simulation::monte_carlo::evaluate_monte_carlo_actions;
use crate::simulation::simulator::SimulationFailure;

#[test]
fn monte_carlo_search_returns_stable_metadata() {
    let mut game = game_with_draw_order(2, vec![Card::Number(5)]);
    game.deal_selected_card(Card::Number(5)).unwrap();
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(6),
        Card::Number(7),
        Card::Number(5),
    ]));
    let settings = SimulationConfig {
        decision_rollouts: 3,
        ..settings()
    };
    let legal = legal_ai_decisions(&game);

    let first = evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings)
        .unwrap()
        .unwrap();
    let second = evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings)
        .unwrap()
        .unwrap();

    assert_eq!(first.decision, second.decision);
    assert_eq!(first.samples, 3);
    assert!((0.0..=1.0).contains(&first.win_rate));
    assert!(legal.contains(&first.decision));
}

#[test]
fn capped_rollouts_do_not_award_wins() {
    let mut game = game_with_draw_order(1, vec![Card::Number(5)]);
    game.deal_selected_card(Card::Number(5)).unwrap();
    let config = SimulationConfig {
        target_score: 1_000,
        max_rounds: 1,
        ..settings()
    };
    let evaluation =
        evaluate_monte_carlo_actions(&game, PlayerId::new(0), &[AiDecision::Stay], &config)
            .unwrap()
            .unwrap();
    assert_eq!(evaluation.samples, config.decision_rollouts);
    assert_eq!(evaluation.completed_samples, 0);
    assert_eq!(evaluation.win_rate, 0.0);
    assert!(evaluation.average_utility < 1.0);
}

#[test]
fn rejected_rollout_decision_propagates_to_the_search_caller() {
    let game = game_with_draw_order(1, vec![Card::Number(5)]);
    let result = evaluate_monte_carlo_actions(
        &game,
        PlayerId::new(0),
        &[AiDecision::ChooseTarget(PlayerId::new(9))],
        &settings(),
    );
    assert_eq!(
        result.unwrap_err(),
        SimulationFailure::CommandRejected(GameError::InvalidTarget)
    );
}

#[test]
fn stalled_rollout_propagates_through_target_recommendation() {
    let mut game = game_with_draw_order(1, vec![Card::FlipThree]);
    game.deal_selected_card(Card::FlipThree).unwrap();
    assert_eq!(
        recommend_decision(&game, StrategyKind::MaxWinProbability, &settings()).unwrap_err(),
        SimulationFailure::DecisionLimitExceeded,
    );
}
