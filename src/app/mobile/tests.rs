use crate::model::{Card, GameState};
use eframe::egui;

use super::super::FlipSevenApp;
use super::MobileView;

fn frame(
    app: &mut FlipSevenApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(390.0, 844.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.render_mobile(ctx),
    )
}

fn text_center(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Pos2 {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("Missing control: {label}"))
}

fn tap(app: &mut FlipSevenApp, ctx: &egui::Context, label: &str) {
    let _ = frame(app, ctx, vec![]);
    let output = frame(app, ctx, vec![]);
    let pos = text_center(&output.shapes, label);
    for pressed in [true, false] {
        let _ = frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn mobile_deal_and_undo_restore_the_live_game() {
    let ctx = egui::Context::default();
    let mut app = FlipSevenApp::default();
    let before = app.game.clone();
    tap(&mut app, &ctx, "7\nx7");
    assert_eq!(app.game.players()[0].hand(), &[Card::Number(7)]);
    tap(&mut app, &ctx, "Undo");
    assert_eq!(app.game, before);
}

#[test]
fn mobile_navigation_preserves_game_and_next_round_scores() {
    let ctx = egui::Context::default();
    let mut app = FlipSevenApp {
        game: GameState::new(1),
        ..Default::default()
    };
    tap(&mut app, &ctx, "7\nx7");
    tap(&mut app, &ctx, "Stay");
    assert!(app.game.round_over());
    assert_eq!(app.game.score_board()[0].total_score, 7);
    let before_navigation = app.game.clone();
    tap(&mut app, &ctx, "Scores");
    assert!(app.mobile_view == MobileView::Scores);
    tap(&mut app, &ctx, "Settings");
    tap(&mut app, &ctx, "Play");
    assert_eq!(app.game, before_navigation);
    tap(&mut app, &ctx, "Next round");
    assert_eq!(app.game.round_number(), 2);
    assert_eq!(app.game.score_board()[0].total_score, 7);
}
