use eframe::egui;

use super::FlipSevenApp;
use crate::model::{Card, Deck, GameState};

fn assert_panel_score(app: &FlipSevenApp, score: u32) {
    let ctx = egui::Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.render_current_player(ui));
    });
    let label = format!("Score if round ended: {score}");
    fn contains(shape: &egui::Shape, label: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text() == label,
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, label)),
            _ => false,
        }
    }
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| contains(&shape.shape, &label)),
        "missing {label}"
    );
}

#[test]
fn current_panel_preserves_banked_score_and_tracks_undo() {
    let mut app = FlipSevenApp {
        game: GameState::with_deck(1, Deck::from_draw_order([Card::Number(7)])),
        ..Default::default()
    };
    app.game.deal_selected_card(Card::Number(7)).unwrap();
    assert_panel_score(&app, 7);
    app.stay();
    assert!(app.game.players()[0].hand().is_empty());
    assert_panel_score(&app, 7);
    app.undo();
    assert_panel_score(&app, 7);
    app.undo();
    assert_panel_score(&app, 0);
}

#[test]
fn current_panel_includes_banked_flip_seven_bonus() {
    let mut app = FlipSevenApp {
        game: GameState::with_deck(1, Deck::from_draw_order((0..7).map(Card::Number))),
        ..Default::default()
    };
    for number in 0..7 {
        app.game.deal_selected_card(Card::Number(number)).unwrap();
    }
    assert!(app.game.round_over());
    assert_panel_score(&app, 36);
}
