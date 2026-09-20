use crate::model::{Card, CardEffect, GameEvent, PlayerId, SpecialAction};

use super::game_with_draw_order;

#[test]
fn duplicate_second_chance_is_assigned_immediately() {
    let mut game =
        game_with_draw_order(2, [Card::SecondChance, Card::Number(1), Card::SecondChance]);
    game.deal_selected_card(Card::SecondChance).unwrap();
    game.deal_selected_card(Card::Number(1)).unwrap();
    assert!(matches!(
        game.deal_selected_card(Card::SecondChance),
        Ok(GameEvent::TargetRequired {
            action: SpecialAction::SecondChance,
            ..
        })
    ));
    assert_eq!(game.legal_pending_targets(), [PlayerId::new(1)]);
    game.choose_target(PlayerId::new(1)).unwrap();
    assert!(game.players()[1].has_second_chance());
}

#[test]
fn duplicate_second_chance_is_discarded_without_a_recipient() {
    let mut game = game_with_draw_order(1, [Card::SecondChance, Card::SecondChance]);
    game.deal_selected_card(Card::SecondChance).unwrap();
    assert!(matches!(
        game.deal_selected_card(Card::SecondChance),
        Ok(GameEvent::CardDealt {
            effect: CardEffect::SecondChanceDiscarded,
            ..
        })
    ));
    assert_eq!(game.discard_pile_count(), 1);
}

#[test]
fn flip_three_defers_specials_in_fifo_order() {
    let mut game = game_with_draw_order(
        3,
        [
            Card::FlipThree,
            Card::Freeze,
            Card::FlipThree,
            Card::Number(2),
        ],
    );
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(1)).unwrap();
    game.deal_selected_card(Card::Freeze).unwrap();
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.deal_selected_card(Card::Number(2)).unwrap();
    assert_eq!(
        game.pending_action().unwrap().action(),
        SpecialAction::Freeze
    );
    game.choose_target(PlayerId::new(2)).unwrap();
    assert_eq!(
        game.pending_action().unwrap().action(),
        SpecialAction::FlipThree
    );
}

#[test]
fn flip_three_bust_cancels_scoped_deferred_actions() {
    let mut game = game_with_draw_order(
        2,
        [
            Card::Number(5),
            Card::FlipThree,
            Card::Freeze,
            Card::Number(5),
        ],
    );
    game.deal_selected_card(Card::Number(5)).unwrap();
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(0)).unwrap();
    game.deal_selected_card(Card::Freeze).unwrap();
    game.deal_selected_card(Card::Number(5)).unwrap();
    assert!(game.pending_action().is_none());
}

#[test]
fn undo_restores_each_pending_stage() {
    let mut game = game_with_draw_order(2, [Card::FlipThree, Card::Number(4)]);
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(1)).unwrap();
    game.deal_selected_card(Card::Number(4)).unwrap();
    game.undo().unwrap();
    assert_eq!(game.pending_action().unwrap().remaining_draws(), 3);
    game.undo().unwrap();
    assert!(game.pending_action().unwrap().needs_target());
}

#[test]
fn flip_three_resumes_after_original_source_player() {
    let mut game = game_with_draw_order(
        3,
        [
            Card::FlipThree,
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
        ],
    );
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(2)).unwrap();
    for card in [Card::Number(1), Card::Number(2), Card::Number(3)] {
        game.deal_selected_card(card).unwrap();
    }
    assert!(game.pending_action().is_none());
    assert_eq!(game.current_player_index(), Some(1));
}

#[test]
fn flip_three_pauses_for_duplicate_second_chance_transfer() {
    let mut game = game_with_draw_order(
        3,
        [
            Card::SecondChance,
            Card::Number(1),
            Card::FlipThree,
            Card::SecondChance,
            Card::Number(2),
            Card::Number(3),
        ],
    );
    game.deal_selected_card(Card::SecondChance).unwrap();
    game.deal_selected_card(Card::Number(1)).unwrap();
    game.deal_selected_card(Card::FlipThree).unwrap();
    game.choose_target(PlayerId::new(0)).unwrap();
    game.deal_selected_card(Card::SecondChance).unwrap();
    assert_eq!(
        game.pending_action().unwrap().action(),
        SpecialAction::SecondChance
    );
    game.choose_target(PlayerId::new(1)).unwrap();
    assert_eq!(game.pending_action().unwrap().remaining_draws(), 2);
}
