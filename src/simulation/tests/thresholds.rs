use super::{game_with_draw_order, settings};
use crate::model::{BonusCard, Card, Deck, PlayerId};
use crate::simulation::decision::{AiDecision, recommend_decision};
use crate::simulation::simulator::simulation_clone;
use crate::simulation::{SimulationConfig, StrategyKind};

#[test]
fn stay_at_number_count_uses_number_card_threshold() {
    let mut below = game_with_draw_order(1, vec![Card::Number(1), Card::Number(2)]);
    below.deal_selected_card(Card::Number(1)).unwrap();
    below.deal_selected_card(Card::Number(2)).unwrap();
    below.replace_deck_for_test(Deck::from_draw_order([Card::Number(3)]));

    let mut at_three =
        game_with_draw_order(1, vec![Card::Number(1), Card::Number(2), Card::Number(3)]);
    for card in [Card::Number(1), Card::Number(2), Card::Number(3)] {
        at_three.deal_selected_card(card).unwrap();
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
        at_four.deal_selected_card(card).unwrap();
    }
    at_four.replace_deck_for_test(Deck::from_draw_order([Card::Number(5)]));

    assert_eq!(
        recommend_decision(&below, StrategyKind::StayAtNumberCount(3), &settings())
            .unwrap()
            .decision,
        Some(AiDecision::Draw)
    );
    assert_eq!(
        recommend_decision(&at_three, StrategyKind::StayAtNumberCount(3), &settings())
            .unwrap()
            .decision,
        Some(AiDecision::Stay)
    );
    assert_eq!(
        recommend_decision(&at_three, StrategyKind::StayAtNumberCount(4), &settings())
            .unwrap()
            .decision,
        Some(AiDecision::Draw)
    );
    assert_eq!(
        recommend_decision(&at_four, StrategyKind::StayAtNumberCount(4), &settings())
            .unwrap()
            .decision,
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
        below.deal_selected_card(Card::Number(12)).unwrap();
        below
            .deal_selected_card(Card::Bonus(BonusCard::Plus(below_bonus)))
            .unwrap();
        below.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

        let mut at_threshold = game_with_draw_order(
            1,
            vec![Card::Number(12), Card::Bonus(BonusCard::Plus(at_bonus))],
        );
        at_threshold.deal_selected_card(Card::Number(12)).unwrap();
        at_threshold
            .deal_selected_card(Card::Bonus(BonusCard::Plus(at_bonus)))
            .unwrap();
        at_threshold.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

        assert_eq!(
            recommend_decision(
                &below,
                StrategyKind::StayAtScore(threshold as u32),
                &settings()
            )
            .unwrap()
            .decision,
            Some(AiDecision::Draw)
        );
        assert_eq!(
            recommend_decision(
                &at_threshold,
                StrategyKind::StayAtScore(threshold as u32),
                &settings()
            )
            .unwrap()
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
        protected.deal_selected_card(card).unwrap();
    }
    protected.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

    let recommendation =
        recommend_decision(&protected, StrategyKind::StayAtNumberCount(3), &settings()).unwrap();
    assert_eq!(recommendation.decision, Some(AiDecision::Draw));

    let mut winning = simulation_clone(&protected);
    winning.set_total_score_for_test(PlayerId::new(0), 20);
    let target_settings = SimulationConfig {
        target_score: 50,
        ..settings()
    };
    let recommendation = recommend_decision(
        &winning,
        StrategyKind::StayAtNumberCount(3),
        &target_settings,
    )
    .unwrap();
    assert_eq!(recommendation.decision, Some(AiDecision::Stay));
}
