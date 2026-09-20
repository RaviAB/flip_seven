mod commands;
mod odds;
mod pending;
mod scoring;

use crate::model::{Card, Deck};

use super::GameState;

fn game_with_draw_order(player_count: usize, cards: impl IntoIterator<Item = Card>) -> GameState {
    GameState::with_deck(player_count, Deck::from_draw_order(cards))
}
