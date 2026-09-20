use eframe::egui;
use std::time::Duration;
use web_time::Instant;

mod components;
mod desktop;
mod mobile;
mod odds;
mod presentation;
mod scoreboard;
mod theme;

use presentation::status_for_result;

use crate::model::{Card, GameEvent, GameState, PlayerId};

const MIN_PLAYERS: usize = 1;
const MAX_PLAYERS: usize = 18;

pub struct FlipSevenApp {
    selected_player_count: usize,
    player_count_input: String,
    game: GameState,
    status: String,
    show_deck_details: bool,
    selected_card: Option<Card>,
    show_diagnostics: bool,
    last_frame_at: Option<Instant>,
    smoothed_frame_ms: f32,
    frame_count: u64,
    mobile_view: mobile::MobileView,
}

impl Default for FlipSevenApp {
    fn default() -> Self {
        let selected_player_count = 4;

        Self {
            selected_player_count,
            player_count_input: selected_player_count.to_string(),
            game: GameState::new(selected_player_count),
            status: "Ready to deal.".to_owned(),
            show_deck_details: false,
            selected_card: None,
            show_diagnostics: false,
            last_frame_at: None,
            smoothed_frame_ms: 0.0,
            frame_count: 0,
            mobile_view: mobile::MobileView::Play,
        }
    }
}

impl eframe::App for FlipSevenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_frame_stats();
        if self.show_diagnostics {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        let size = ctx.screen_rect().size();
        let compact = size.x < 700.0 || (size.x < 1100.0 && size.y < 500.0);
        if compact {
            self.render_mobile(ctx);
        } else {
            self.render_desktop(ctx);
        }
        if !compact {
            self.render_deck_details(ctx);
        }
        self.render_diagnostics(ctx);
    }
}

impl FlipSevenApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        Self::default()
    }

    fn update_frame_stats(&mut self) {
        self.frame_count += 1;
        let now = Instant::now();
        let Some(last_frame_at) = self.last_frame_at.replace(now) else {
            return;
        };

        let frame_ms = now.duration_since(last_frame_at).as_secs_f32() * 1000.0;
        if self.smoothed_frame_ms == 0.0 {
            self.smoothed_frame_ms = frame_ms;
        } else {
            self.smoothed_frame_ms = self.smoothed_frame_ms * 0.9 + frame_ms * 0.1;
        }
    }

    fn render_diagnostics(&self, ctx: &egui::Context) {
        if !self.show_diagnostics {
            return;
        }

        let fps = if self.smoothed_frame_ms > 0.0 {
            1000.0 / self.smoothed_frame_ms
        } else {
            0.0
        };
        let pointer = ctx.input(|input| input.pointer.hover_pos());

        egui::Area::new(egui::Id::new("diagnostics_overlay"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_premultiplied(18, 20, 22, 232))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(104, 116, 128),
                    ))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.visuals_mut().override_text_color =
                            Some(egui::Color32::from_rgb(238, 240, 242));
                        ui.label(egui::RichText::new("Diagnostics").strong());
                        ui.label(format!("FPS: {:.1}", fps));
                        ui.label(format!("Frame: {:.1} ms", self.smoothed_frame_ms));
                        ui.label(format!("Frames: {}", self.frame_count));
                        if let Some(pointer) = pointer {
                            ui.label(format!("Pointer: {:.0}, {:.0}", pointer.x, pointer.y));
                        } else {
                            ui.label("Pointer: -");
                        }
                    });
            });
    }

    fn deal_selected_card(&mut self) {
        let Some(card) = self.selected_card else {
            self.status = "Choose a card to deal.".to_owned();
            return;
        };

        let outcome = self.game.deal_selected_card(card);
        self.status = status_for_result(outcome, &self.game);
    }

    fn stay(&mut self) {
        let outcome = self.game.stay_current_player();
        self.status = status_for_result(outcome, &self.game);
    }

    fn next_round(&mut self) {
        let outcome = self.game.start_next_round();
        self.status = status_for_result(outcome, &self.game);
    }

    fn reset_game(&mut self) {
        self.normalize_player_count_input();
        let outcome = self.game.reset(self.selected_player_count);
        self.status = status_for_result(outcome, &self.game);
    }

    fn undo(&mut self) {
        let outcome = self.game.undo();
        if matches!(outcome, Ok(GameEvent::UndoApplied)) {
            self.selected_player_count = self.game.players().len();
            self.player_count_input = self.selected_player_count.to_string();
        }
        self.status = status_for_result(outcome, &self.game);
    }

    fn reshuffle_discard(&mut self) {
        let outcome = self.game.reshuffle_discard();
        self.status = status_for_result(outcome, &self.game);
    }

    fn choose_target(&mut self, player_id: PlayerId) {
        let outcome = self.game.choose_target(player_id);
        self.status = status_for_result(outcome, &self.game);
    }

    fn normalize_player_count_input(&mut self) {
        if let Ok(value) = self.player_count_input.parse::<usize>() {
            self.selected_player_count = value.clamp(MIN_PLAYERS, MAX_PLAYERS);
        }
        self.player_count_input = self.selected_player_count.to_string();
    }
}
