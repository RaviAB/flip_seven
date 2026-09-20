use eframe::egui;

use crate::model::Card;

use super::presentation::card_tooltip;
use super::theme::card_colors;
use super::{FlipSevenApp, MAX_PLAYERS, MIN_PLAYERS};

pub(super) fn render_hand(ui: &mut egui::Ui, hand: &[Card]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        let mut cards = hand.to_vec();
        cards.sort_by_key(|card| card.sort_key());
        if cards.is_empty() {
            ui.label("-");
        } else {
            for card in cards {
                render_card(ui, card);
            }
        }
    });
}

pub(super) fn render_card(ui: &mut egui::Ui, card: Card) {
    render_card_chip(ui, card, false).on_hover_text(card_tooltip(card));
}

pub(super) fn render_selectable_card_group(
    ui: &mut egui::Ui,
    title: &str,
    counts: &[(Card, usize)],
    selected_card: &mut Option<Card>,
    can_deal: bool,
    clicked_card: &mut Option<Card>,
    predicate: impl Fn(Card) -> bool,
) {
    let filtered = counts
        .iter()
        .copied()
        .filter(|(card, _)| predicate(*card))
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return;
    }
    ui.label(egui::RichText::new(title).small().strong());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        for (card, count) in filtered {
            if ui
                .add_enabled_ui(can_deal, |ui| {
                    render_card_chip(ui, card, *selected_card == Some(card))
                })
                .inner
                .on_hover_text(card_tooltip(card))
                .clicked()
            {
                *clicked_card = Some(card);
            }
            ui.small(format!("x{count}"));
        }
    });
}

pub(super) fn render_card_count_column(
    ui: &mut egui::Ui,
    counts: &[(Card, usize)],
    predicate: impl Fn(Card) -> bool,
) {
    let mut rendered_any = false;
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for (card, count) in counts.iter().copied().filter(|(card, _)| predicate(*card)) {
                rendered_any = true;
                render_card(ui, card);
                ui.small(format!("x{count}"))
                    .on_hover_text(format!("{count} copies of {card} in this pile."));
                ui.end_row();
            }
        });
    if !rendered_any {
        ui.label("-");
    }
}

pub(super) fn render_card_chip(ui: &mut egui::Ui, card: Card, selected: bool) -> egui::Response {
    let (fill, stroke, text_color) = card_colors(card);
    let stroke = if selected {
        egui::Stroke::new(2.0, egui::Color32::from_rgb(245, 226, 130))
    } else {
        egui::Stroke::new(1.0, stroke)
    };
    let text = egui::RichText::new(card.short_label())
        .strong()
        .size(12.0)
        .color(text_color);
    ui.add_sized(
        egui::vec2(38.0, 24.0),
        egui::Button::new(text)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(5)),
    )
}

impl FlipSevenApp {
    pub(super) fn render_player_count_control(&mut self, ui: &mut egui::Ui) {
        ui.label("Players");
        if ui
            .add_enabled(
                self.selected_player_count > MIN_PLAYERS,
                egui::Button::new("-"),
            )
            .on_hover_text("Decrease player count")
            .clicked()
        {
            self.selected_player_count -= 1;
            self.player_count_input = self.selected_player_count.to_string();
        }

        let count_response = ui.add(
            egui::TextEdit::singleline(&mut self.player_count_input)
                .desired_width(44.0)
                .char_limit(MAX_PLAYERS.to_string().len()),
        );
        if count_response.changed() {
            let digits = self
                .player_count_input
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>();
            if digits != self.player_count_input {
                self.player_count_input = digits;
            }
            if let Ok(value) = self.player_count_input.parse::<usize>() {
                self.selected_player_count = value.clamp(MIN_PLAYERS, MAX_PLAYERS);
            }
        }
        if count_response.lost_focus() {
            self.normalize_player_count_input();
        }
        count_response.on_hover_text("Type a player count");

        if ui
            .add_enabled(
                self.selected_player_count < MAX_PLAYERS,
                egui::Button::new("+"),
            )
            .on_hover_text("Increase player count")
            .clicked()
        {
            self.selected_player_count += 1;
            self.player_count_input = self.selected_player_count.to_string();
        }

        let active_player_count = self.game.players().len();
        let player_count_changed = self.selected_player_count != active_player_count;
        if ui
            .add_enabled(player_count_changed, egui::Button::new("Apply"))
            .on_hover_text("Reset the game with this player count")
            .clicked()
        {
            self.normalize_player_count_input();
            self.reset_game();
        }
        if player_count_changed {
            ui.colored_label(
                egui::Color32::from_rgb(245, 184, 86),
                format!("{active_player_count} active"),
            );
        }
    }
}
