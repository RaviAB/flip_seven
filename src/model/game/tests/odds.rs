use super::game_with_draw_order;
use crate::model::{BonusCard, Card, Deck, GameState, PlayerId};

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn odds_weight_duplicate_counts_and_expected_scores() {
    let mut game = game_with_draw_order(
        1,
        [
            Card::Number(5),
            Card::Number(5),
            Card::Number(5),
            Card::Number(7),
        ],
    );
    game.deal_selected_card(Card::Number(5)).unwrap();
    let before = game.clone();
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.next_pool_size, 3);
    assert_eq!(odds.current_score, 5);
    close(odds.bust_probability, 2.0 / 3.0);
    close(odds.expected_score_after_draw, 4.0);
    close(odds.expected_score_delta, -1.0);
    assert_eq!(odds.bust_cards[0].count, 2);
    close(
        odds.expected_value_details
            .iter()
            .map(|d| d.probability)
            .sum(),
        1.0,
    );
    close(
        odds.expected_value_details
            .iter()
            .map(|d| d.weighted_score)
            .sum(),
        4.0,
    );
    assert_eq!(game, before);
}

#[test]
fn second_chance_removes_bust_risk_without_losing_banked_value() {
    let mut game = game_with_draw_order(
        1,
        [
            Card::Number(5),
            Card::SecondChance,
            Card::Number(5),
            Card::Number(7),
        ],
    );
    game.deal_selected_card(Card::Number(5)).unwrap();
    game.deal_selected_card(Card::SecondChance).unwrap();
    let before = game.clone();
    let odds = game.current_player_odds().unwrap();
    close(odds.bust_probability, 0.0);
    close(odds.expected_score_after_draw, 8.5);
    close(odds.expected_score_delta, 3.5);
    close(odds.bust_cards[0].probability, 0.5);
    assert!(odds.bust_cards[0].protected_by_second_chance);
    assert_eq!(game, before);
}

#[test]
fn flip_seven_odds_include_multiplier_additive_and_completion_bonuses() {
    let hand = (0..6)
        .map(Card::Number)
        .chain([
            Card::Bonus(BonusCard::Double),
            Card::Bonus(BonusCard::Plus(10)),
        ])
        .collect::<Vec<_>>();
    let mut game = game_with_draw_order(
        1,
        hand.iter()
            .copied()
            .chain([Card::Number(6), Card::Number(5)]),
    );
    for card in hand {
        game.deal_selected_card(card).unwrap();
    }
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.current_score, 40);
    close(odds.flip_seven_probability, 0.5);
    close(odds.bust_probability, 0.5);
    close(odds.expected_score_after_draw, 33.5);
    close(odds.expected_score_delta, -6.5);
    assert_eq!(
        odds.expected_value_details
            .iter()
            .find(|d| d.card == Card::Number(6))
            .unwrap()
            .score_after_draw,
        67
    );
}

#[test]
fn odds_use_discard_only_after_draw_pile_is_empty_without_reshuffling() {
    let mut game = GameState::with_deck(
        1,
        Deck::with_draw_and_discard([Card::Number(5), Card::Number(7)], [Card::Number(5)]),
    );
    game.deal_selected_card(Card::Number(5)).unwrap();
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.next_pool_size, 1);
    close(odds.bust_probability, 0.0);
    close(odds.expected_score_after_draw, 12.0);
    game.deal_selected_card(Card::Number(7)).unwrap();
    let before = game.clone();
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.next_pool_size, 1);
    close(odds.bust_probability, 1.0);
    close(odds.expected_score_after_draw, 0.0);
    assert_eq!(game, before);
    assert_eq!(game.draw_pile_count(), 0);
    assert_eq!(game.discard_pile_count(), 1);
}

#[test]
fn empty_pool_has_finite_odds_and_round_end_has_no_odds() {
    let mut game = game_with_draw_order(1, [Card::Number(5)]);
    game.deal_selected_card(Card::Number(5)).unwrap();
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.next_pool_size, 0);
    close(odds.expected_score_after_draw, 5.0);
    close(odds.expected_score_delta, 0.0);
    close(odds.bust_probability, 0.0);
    game.stay_current_player().unwrap();
    assert!(game.current_player_odds().is_none());
}

#[test]
fn flip_three_odds_use_the_target_hand() {
    let mut game = game_with_draw_order(
        2,
        [
            Card::Number(5),
            Card::Number(9),
            Card::FlipThree,
            Card::Number(9),
            Card::Number(1),
        ],
    );
    for card in [Card::Number(5), Card::Number(9), Card::FlipThree] {
        game.deal_selected_card(card).unwrap();
    }
    game.choose_target(PlayerId::new(1)).unwrap();
    let odds = game.current_player_odds().unwrap();
    assert_eq!(odds.current_score, 9);
    close(odds.bust_probability, 0.5);
    close(odds.expected_score_after_draw, 5.0);
}
