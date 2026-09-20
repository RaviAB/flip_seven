use super::{game_with_draw_order, settings};
use crate::model::{BonusCard, Card, Deck, GameState, PlayerId, PlayerStatus};
use crate::simulation::decision::{
    AiDecision, legal_ai_decisions, recommend_decision, win_probability_profile,
};
use crate::simulation::{SimulationConfig, StrategyKind};

#[test]
fn ev_ai_stays_when_current_score_beats_draw_ev() {
    let mut game = game_with_draw_order(1, vec![Card::Number(12)]);
    game.deal_selected_card(Card::Number(12)).unwrap();
    game.replace_deck_for_test(Deck::from_draw_order([Card::Number(12), Card::Number(1)]));

    let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings()).unwrap();

    assert_eq!(recommendation.decision, Some(AiDecision::Stay));
}

#[test]
fn ev_ai_draws_when_draw_ev_beats_current_score() {
    let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
    game.deal_selected_card(Card::Number(1)).unwrap();
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(10),
        Card::Bonus(BonusCard::Plus(10)),
        Card::SecondChance,
    ]));

    let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings()).unwrap();

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
    game.deal_selected_card(Card::Number(5)).unwrap();

    for strategy in StrategyKind::catalog() {
        let recommendation = recommend_decision(&game, strategy, &settings()).unwrap();
        if let Some(decision) = recommendation.decision {
            assert!(legal_ai_decisions(&game).contains(&decision));
        }
    }
}

#[test]
fn conservative_stays_where_aggressive_draws() {
    let mut game = game_with_draw_order(1, vec![Card::Number(10)]);
    game.deal_selected_card(Card::Number(10)).unwrap();
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(10),
        Card::Number(11),
        Card::Number(12),
    ]));

    let conservative = recommend_decision(&game, StrategyKind::Conservative, &settings()).unwrap();
    let aggressive = recommend_decision(&game, StrategyKind::Aggressive, &settings()).unwrap();

    assert_eq!(conservative.decision, Some(AiDecision::Stay));
    assert_eq!(aggressive.decision, Some(AiDecision::Draw));
}

#[test]
fn win_probability_pressure_profiles_are_monotonic() {
    let settings = SimulationConfig {
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
    game.deal_selected_card(Card::Number(8)).unwrap();
    game.stay_current_player().unwrap();
    game.deal_selected_card(Card::Number(12)).unwrap();
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
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let recommendation =
        recommend_decision(&game, StrategyKind::MaxWinProbability, &settings).unwrap();

    assert_eq!(recommendation.decision, Some(AiDecision::Draw));
    assert!(recommendation.rationale.contains("pressure"));
    assert!(recommendation.rationale.contains("leading opponent 120"));
}

#[test]
fn win_probability_stays_more_conservatively_when_ahead() {
    let game = two_player_game_with_player_zero_score_pressure(120, 50);
    let settings = SimulationConfig {
        target_score: 200,
        ..settings()
    };
    let recommendation =
        recommend_decision(&game, StrategyKind::MaxWinProbability, &settings).unwrap();

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
        game.deal_selected_card(card).unwrap();
    }
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(6),
        Card::Number(7),
        Card::Number(8),
        Card::Number(0),
    ]));

    let recommendation = recommend_decision(&game, StrategyKind::Aggressive, &settings()).unwrap();

    assert_eq!(recommendation.decision, Some(AiDecision::Draw));
}

#[test]
fn win_probability_ai_can_choose_different_action_than_ev_ai() {
    let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
    game.deal_selected_card(Card::Number(1)).unwrap();
    game.set_total_score_for_test(PlayerId::new(0), 49);
    game.replace_deck_for_test(Deck::from_draw_order([
        Card::Number(10),
        Card::Number(11),
        Card::Number(12),
    ]));
    let settings = SimulationConfig {
        target_score: 50,
        decision_rollouts: 1,
        max_rounds: 6,
        ..settings()
    };

    let ev = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings).unwrap();
    let win = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings).unwrap();

    assert_eq!(ev.decision, Some(AiDecision::Draw));
    assert_eq!(win.decision, Some(AiDecision::Stay));
    assert!(win.rationale.contains("reaches"));
    assert_eq!(game.players()[0].status(), PlayerStatus::Active);
}
