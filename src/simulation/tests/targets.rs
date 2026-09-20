use super::{game_with_draw_order, settings};
use crate::model::{Card, GameEvent, PendingStep, PlayerId};
use crate::simulation::StrategyKind;
use crate::simulation::decision::{AiDecision, recommend_decision};

#[test]
fn target_selection_uses_only_active_legal_targets() {
    let mut game = game_with_draw_order(3, vec![Card::Freeze]);
    game.deal_selected_card(Card::Freeze).unwrap();
    game.force_bust_for_test(PlayerId::new(2));

    let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings()).unwrap();

    let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
        panic!("expected target decision");
    };
    assert_ne!(target_id, PlayerId::new(2));
    assert!(game.legal_active_targets().contains(&target_id));
}

#[test]
fn win_probability_target_selection_reports_monte_carlo_search() {
    let mut game = game_with_draw_order(3, vec![Card::Freeze]);
    game.deal_selected_card(Card::Freeze).unwrap();

    let recommendation =
        recommend_decision(&game, StrategyKind::MaxWinProbability, &settings()).unwrap();

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
    game.deal_selected_card(Card::FlipThree).unwrap();

    let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings()).unwrap();
    let Some(decision @ AiDecision::ChooseTarget(_)) = recommendation.decision else {
        panic!("expected target decision");
    };
    let outcome = match decision {
        AiDecision::ChooseTarget(player_id) => game.choose_target(player_id),
        AiDecision::Stay => game.stay_current_player(),
        AiDecision::Draw => panic!("target selection should not recommend draw"),
    };

    assert!(matches!(outcome, Ok(GameEvent::FlipThreeStarted { .. })));
    assert!(
        game.pending_action()
            .is_some_and(PendingStep::awaiting_selected_draws)
    );
    assert_eq!(game.players()[1].hand().len(), 0);
}
