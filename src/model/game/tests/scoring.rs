use crate::model::{BonusCard, Card, GameEvent, PlayerStatus};

use super::game_with_draw_order;

#[test]
fn duplicate_number_busts_and_scores_zero() {
    let mut game = game_with_draw_order(1, [Card::Number(5), Card::Number(5)]);
    game.deal_next_card().unwrap();
    assert!(matches!(
        game.deal_next_card(),
        Ok(GameEvent::RoundEnded { .. })
    ));
    assert_eq!(game.players()[0].status(), PlayerStatus::Busted);
    assert_eq!(game.score_board()[0].last_round_score, 0);
}

#[test]
fn second_chance_prevents_duplicate_bust() {
    let mut game = game_with_draw_order(1, [Card::Number(5), Card::SecondChance, Card::Number(5)]);
    game.deal_next_card().unwrap();
    game.deal_next_card().unwrap();
    game.deal_next_card().unwrap();
    assert_eq!(game.players()[0].status(), PlayerStatus::Active);
    assert_eq!(game.players()[0].hand(), [Card::Number(5)]);
}

#[test]
fn flip_seven_bonus_and_telemetry_are_recorded() {
    let cards = (0..7).map(Card::Number).collect::<Vec<_>>();
    let mut game = game_with_draw_order(1, cards);
    for _ in 0..7 {
        game.deal_next_card().unwrap();
    }
    let score = &game.score_board()[0];
    assert_eq!(score.last_round_score, 36);
    assert_eq!(score.flip_seven_count, 1);
    assert_eq!(score.telemetry.cards_dealt, 7);
}

#[test]
fn multiplier_applies_before_additive_bonus() {
    let mut game = game_with_draw_order(
        1,
        [
            Card::Number(5),
            Card::Bonus(BonusCard::Double),
            Card::Bonus(BonusCard::Plus(10)),
        ],
    );
    for _ in 0..3 {
        game.deal_next_card().unwrap();
    }
    game.stay_current_player().unwrap();
    assert_eq!(game.score_board()[0].last_round_score, 20);
}

#[test]
fn next_round_rotates_starting_player() {
    let mut game = game_with_draw_order(2, []);
    game.stay_current_player().unwrap();
    game.stay_current_player().unwrap();
    game.start_next_round().unwrap();
    assert_eq!(game.current_player_index(), Some(1));
}
