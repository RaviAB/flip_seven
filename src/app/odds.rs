use eframe::egui;

use crate::model::{BustCardDetail, Card, DrawOdds, ExpectedValueDetail};

use super::components::render_card;
use super::presentation::{
    bust_probability_label, format_percent, protected_duplicate_probability,
};

pub(super) fn render_odds(ui: &mut egui::Ui, odds: &DrawOdds) {
    let next_pool_tooltip = "Cards currently available for the next draw. If the draw pile is empty, this uses the discard pile because it will be reshuffled.";
    let pool_count_tooltip = "The denominator used for the probability calculations below.";
    let current_score_tooltip = "Score the current player would bank by staying right now.";
    let expected_delta_tooltip = "Expected score after drawing minus the score from staying now.";

    egui::Grid::new("odds")
        .num_columns(2)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            let label = ui.label("Next-card pool");
            let value = ui.label(odds.next_pool_size.to_string());
            if label.hovered() {
                show_immediate_text_hover(ui.ctx(), "next_pool_label_hover", next_pool_tooltip);
            }
            if value.hovered() {
                show_immediate_text_hover(ui.ctx(), "next_pool_count_hover", pool_count_tooltip);
            }
            ui.end_row();

            let label = ui.label("Current round score");
            let value = ui.label(odds.current_score.to_string());
            if label.hovered() || value.hovered() {
                show_immediate_text_hover(ui.ctx(), "current_score_hover", current_score_tooltip);
            }
            ui.end_row();

            let label = ui.label("Expected score after draw");
            let value = ui.label(format!("{:.2}", odds.expected_score_after_draw));
            if label.hovered() || value.hovered() {
                show_immediate_hover(ui.ctx(), "expected_value_hover", |ui| {
                    render_expected_value_hover(ui, &odds.expected_value_details)
                });
            }
            ui.end_row();

            let label = ui.label("Expected score delta");
            let value = ui.label(format!("{:+.2}", odds.expected_score_delta));
            if label.hovered() || value.hovered() {
                show_immediate_text_hover(ui.ctx(), "expected_delta_hover", expected_delta_tooltip);
            }
            ui.end_row();

            let label = ui.label("Bust probability");
            let value = ui.label(bust_probability_label(odds));
            if label.hovered() || value.hovered() {
                show_immediate_hover(ui.ctx(), "bust_probability_hover", |ui| {
                    render_bust_hover(ui, &odds.bust_cards)
                });
            }
            ui.end_row();
        });
}

fn show_immediate_text_hover(ctx: &egui::Context, id_source: &'static str, text: &str) {
    show_immediate_hover(ctx, id_source, |ui| {
        ui.label(text);
    });
}

fn show_immediate_hover(
    ctx: &egui::Context,
    id_source: &'static str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let Some(pointer_pos) = ctx.input(|input| input.pointer.hover_pos()) else {
        return;
    };

    egui::Area::new(egui::Id::new(id_source))
        .fixed_pos(pointer_pos + egui::vec2(14.0, 18.0))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgb(36, 39, 43))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(104, 116, 128),
                ))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_max_width(520.0);
                    ui.visuals_mut().override_text_color =
                        Some(egui::Color32::from_rgb(238, 240, 242));
                    add_contents(ui);
                });
        });
}

pub(super) fn render_bust_hover(ui: &mut egui::Ui, details: &[BustCardDetail]) {
    if details.is_empty() {
        ui.label("No duplicate number cards are currently in the next-card pool.");
        return;
    }

    ui.label("Duplicate number cards");
    let protected = protected_duplicate_probability(details);
    if protected > 0.0 {
        ui.label(format!(
            "Second Chance protects {} of duplicate draws.",
            format_percent(protected)
        ));
    }

    egui::Grid::new(ui.next_auto_id())
        .num_columns(4)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Card");
            ui.strong("Count");
            ui.strong("Chance");
            ui.strong("Result");
            ui.end_row();

            for detail in details {
                render_card(ui, Card::Number(detail.number));
                ui.label(detail.count.to_string());
                ui.label(format_percent(detail.probability));
                ui.label(if detail.protected_by_second_chance {
                    "SC"
                } else {
                    "Bust"
                });
                ui.end_row();
            }
        });
}

fn render_expected_value_hover(ui: &mut egui::Ui, details: &[ExpectedValueDetail]) {
    if details.is_empty() {
        ui.label("No cards are currently available to draw.");
        return;
    }

    ui.label("Expected value contributions");
    egui::Grid::new(ui.next_auto_id())
        .num_columns(5)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Card");
            ui.strong("Count");
            ui.strong("Chance");
            ui.strong("Score");
            ui.strong("Weighted");
            ui.end_row();

            for detail in details {
                render_card(ui, detail.card);
                ui.label(detail.count.to_string());
                ui.label(format_percent(detail.probability));
                ui.label(detail.score_after_draw.to_string());
                ui.label(format!("{:.2}", detail.weighted_score));
                ui.end_row();
            }
        });
}
