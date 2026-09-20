use crate::model::{Card, GameError, GameEvent, PlayerId};

use super::game_with_draw_order;

#[test]
fn first_deal_advances_turn() {
    let mut game = game_with_draw_order(2, [Card::Number(7)]);
    assert!(matches!(
        game.deal_next_card(),
        Ok(GameEvent::CardDealt { player_id, .. }) if player_id == PlayerId::new(0)
    ));
    assert_eq!(game.current_player_index(), Some(1));
}

#[test]
fn selected_deal_is_undoable() {
    let mut game = game_with_draw_order(1, [Card::Number(5)]);
    game.deal_selected_card(Card::Number(5)).unwrap();
    assert!(game.players()[0].has_number(5));
    assert_eq!(game.undo(), Ok(GameEvent::UndoApplied));
    assert!(game.players()[0].hand().is_empty());
    assert_eq!(game.draw_pile_count(), 1);
}

#[test]
fn rejected_commands_leave_all_state_unchanged() {
    let mut game = game_with_draw_order(1, [Card::Number(3)]);
    let before = game.clone();
    let depth = game.undo_depth();
    assert_eq!(
        game.deal_selected_card(Card::Number(12)),
        Err(GameError::CardUnavailable {
            card: Card::Number(12)
        })
    );
    assert_eq!(game, before);
    assert_eq!(game.undo_depth(), depth);
}

#[test]
fn reshuffle_is_typed_and_undoable() {
    let mut game = game_with_draw_order(1, [Card::Number(5)]);
    game.deal_next_card().unwrap();
    game.stay_current_player().unwrap();
    assert_eq!(
        game.reshuffle_discard(),
        Ok(GameEvent::Reshuffled { cards_moved: 1 })
    );
    assert_eq!(game.undo(), Ok(GameEvent::UndoApplied));
    assert_eq!(game.discard_pile_count(), 1);
}

#[test]
fn start_next_round_rejects_an_active_round_without_history() {
    let mut game = game_with_draw_order(1, []);
    assert_eq!(game.start_next_round(), Err(GameError::RoundInProgress));
    assert!(!game.can_undo());
}
