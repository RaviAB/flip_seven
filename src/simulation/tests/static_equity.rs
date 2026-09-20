use super::{game_with_draw_order, settings};
use crate::model::{Card, Deck, GameState, PlayerId};
use crate::simulation::decision::{AiDecision, DecisionContext, recommend_decision};
use crate::simulation::static_equity::{static_win_evaluation, static_win_features};
use crate::simulation::{SimulationConfig, StrategyKind};

fn decision_context_for(game: &GameState, player_id: PlayerId) -> DecisionContext {
    DecisionContext {
        player_id,
        pending_action: game.pending_action(),
        legal_targets: game.legal_pending_targets(),
        draw_odds: game.draw_odds_for_player(player_id),
    }
}

#[test]
fn static_features_use_all_opponents_and_player_count() {
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let mut two_player = game_with_draw_order(2, vec![Card::Number(8)]);
    two_player.deal_selected_card(Card::Number(8)).unwrap();
    two_player.set_total_score_for_test(PlayerId::new(0), 20);
    two_player.set_total_score_for_test(PlayerId::new(1), 10);
    two_player.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

    let mut four_player = game_with_draw_order(4, vec![Card::Number(8)]);
    four_player.deal_selected_card(Card::Number(8)).unwrap();
    four_player.set_total_score_for_test(PlayerId::new(0), 20);
    four_player.set_total_score_for_test(PlayerId::new(1), 10);
    four_player.set_total_score_for_test(PlayerId::new(2), 80);
    four_player.set_total_score_for_test(PlayerId::new(3), 90);
    four_player.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

    let two = static_win_features(&two_player, PlayerId::new(0), &settings).unwrap();
    let four = static_win_features(&four_player, PlayerId::new(0), &settings).unwrap();

    assert_eq!(two.player_count, 2);
    assert_eq!(four.player_count, 4);
    assert!(two.relative_rank_after_stay > four.relative_rank_after_stay);
    assert!(four.field_pressure > two.field_pressure);
    assert!(four.worst_margin_after_stay < two.worst_margin_after_stay);
}

#[test]
fn static_endgame_urgency_increases_as_any_player_approaches_target() {
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let mut early = game_with_draw_order(3, vec![Card::Number(8)]);
    early.deal_selected_card(Card::Number(8)).unwrap();
    early.set_total_score_for_test(PlayerId::new(1), 40);
    early.set_total_score_for_test(PlayerId::new(2), 50);
    early.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

    let mut late = game_with_draw_order(3, vec![Card::Number(8)]);
    late.deal_selected_card(Card::Number(8)).unwrap();
    late.set_total_score_for_test(PlayerId::new(1), 190);
    late.set_total_score_for_test(PlayerId::new(2), 50);
    late.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

    let early_features = static_win_features(&early, PlayerId::new(0), &settings).unwrap();
    let late_features = static_win_features(&late, PlayerId::new(0), &settings).unwrap();

    assert!(late_features.endgame_urgency > early_features.endgame_urgency);
}

#[test]
fn static_second_chance_increases_draw_equity() {
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let mut unsafe_game = game_with_draw_order(1, vec![Card::Number(5)]);
    unsafe_game.deal_selected_card(Card::Number(5)).unwrap();
    unsafe_game.replace_deck_for_test(Deck::from_draw_order([Card::Number(5), Card::Number(6)]));

    let mut protected_game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);
    protected_game.deal_selected_card(Card::Number(5)).unwrap();
    protected_game
        .deal_selected_card(Card::SecondChance)
        .unwrap();
    protected_game.replace_deck_for_test(Deck::from_draw_order([Card::Number(5), Card::Number(6)]));

    let unsafe_eval = static_win_evaluation(
        &unsafe_game,
        &decision_context_for(&unsafe_game, PlayerId::new(0)),
        &settings,
    )
    .unwrap();
    let protected_eval = static_win_evaluation(
        &protected_game,
        &decision_context_for(&protected_game, PlayerId::new(0)),
        &settings,
    )
    .unwrap();

    assert!(!unsafe_eval.features.has_second_chance);
    assert!(protected_eval.features.has_second_chance);
    assert!(
        protected_eval.draw_equity - protected_eval.stay_equity
            > unsafe_eval.draw_equity - unsafe_eval.stay_equity
    );
}

#[test]
fn static_flip_seven_upside_increases_draw_equity_when_close() {
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let dealt = [
        Card::Number(0),
        Card::Number(1),
        Card::Number(2),
        Card::Number(3),
        Card::Number(4),
        Card::Number(5),
    ];
    let mut flip_upside = game_with_draw_order(1, dealt.to_vec());
    for card in dealt {
        flip_upside.deal_selected_card(card).unwrap();
    }
    flip_upside.replace_deck_for_test(Deck::from_draw_order([Card::Number(6)]));

    let mut bust_only = game_with_draw_order(1, dealt.to_vec());
    for card in dealt {
        bust_only.deal_selected_card(card).unwrap();
    }
    bust_only.replace_deck_for_test(Deck::from_draw_order([Card::Number(0)]));

    let upside = static_win_evaluation(
        &flip_upside,
        &decision_context_for(&flip_upside, PlayerId::new(0)),
        &settings,
    )
    .unwrap();
    let bust = static_win_evaluation(
        &bust_only,
        &decision_context_for(&bust_only, PlayerId::new(0)),
        &settings,
    )
    .unwrap();

    assert_eq!(upside.features.flip_seven_distance, 1);
    assert!(upside.features.flip_seven_probability > bust.features.flip_seven_probability);
    assert!(upside.draw_equity > bust.draw_equity);
}

#[test]
fn static_strategy_hard_rules_stay_at_target_and_draw_at_zero() {
    let mut winning = game_with_draw_order(1, vec![Card::Number(1)]);
    winning.deal_selected_card(Card::Number(1)).unwrap();
    winning.set_total_score_for_test(PlayerId::new(0), 49);
    winning.replace_deck_for_test(Deck::from_draw_order([Card::Number(12)]));
    let target_settings = SimulationConfig {
        target_score: 50,
        ..settings()
    };

    let recommendation =
        recommend_decision(&winning, StrategyKind::MaxWinStatic, &target_settings).unwrap();
    assert_eq!(recommendation.decision, Some(AiDecision::Stay));
    assert!(recommendation.rationale.contains("reaches"));

    let zero = game_with_draw_order(1, vec![Card::Number(9)]);
    let recommendation =
        recommend_decision(&zero, StrategyKind::MaxWinStatic, &settings()).unwrap();
    assert_eq!(recommendation.decision, Some(AiDecision::Draw));
    assert!(recommendation.rationale.contains("no points"));
}

#[test]
fn static_draw_stay_decision_does_not_use_decision_rollouts() {
    let mut game = game_with_draw_order(2, vec![Card::Number(8)]);
    game.deal_selected_card(Card::Number(8)).unwrap();
    game.set_total_score_for_test(PlayerId::new(0), 50);
    game.set_total_score_for_test(PlayerId::new(1), 120);
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(9),
        Card::Number(10),
        Card::Number(8),
    ]));
    let low_rollout_settings = SimulationConfig {
        target_score: 200,
        decision_rollouts: 1,
        ..settings()
    };
    let high_rollout_settings = SimulationConfig {
        decision_rollouts: 999,
        ..low_rollout_settings.clone()
    };

    let low = recommend_decision(&game, StrategyKind::MaxWinStatic, &low_rollout_settings).unwrap();
    let high =
        recommend_decision(&game, StrategyKind::MaxWinStatic, &high_rollout_settings).unwrap();

    assert_eq!(low.decision, high.decision);
    assert_eq!(low.rationale, high.rationale);
    assert!(!low.rationale.contains("Monte Carlo"));
}
