use eframe::egui;

use crate::model::{Card, PendingStep};

use super::FlipSevenApp;
use super::components::{render_card, render_card_count_column, render_hand};
use super::odds::render_bust_hover;
use super::presentation::{
    card_tooltip, display_round_score, format_percent, player_score_tooltip, queued_action_suffix,
    round_bust_risk_tooltip, round_outcome_label, special_action_label,
};
use super::theme::{
    card_colors, current_player_frame, manual_deal_frame, neutral_panel_frame, player_frame,
    status_chip,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MobileView {
    Play,
    Scores,
    Settings,
}

impl FlipSevenApp {
    pub(super) fn render_mobile(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("mobile_header")
            .frame(neutral_panel_frame())
            .show(ctx, |ui| {
                touch_spacing(ui);
                ui.spacing_mut().interact_size.y = 24.0;
                ui.horizontal(|ui| {
                    ui.heading("Flip Seven");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Round {}", self.game.round_number()));
                    });
                });
            });

        egui::TopBottomPanel::bottom("mobile_navigation")
            .frame(neutral_panel_frame())
            .show(ctx, |ui| {
                touch_spacing(ui);
                if self.mobile_view == MobileView::Play {
                    ui.columns(2, |columns| {
                        let round_over = self.game.round_over();
                        let enabled = round_over || self.game.pending_action().is_none();
                        if columns[0]
                            .add_enabled(
                                enabled,
                                egui::Button::new(if round_over { "Next round" } else { "Stay" })
                                    .min_size(egui::vec2(columns[0].available_width(), 48.0)),
                            )
                            .clicked()
                        {
                            if round_over {
                                self.next_round();
                            } else {
                                self.stay();
                            }
                        }
                        if columns[1]
                            .add_enabled(
                                self.game.can_undo(),
                                egui::Button::new("Undo")
                                    .min_size(egui::vec2(columns[1].available_width(), 48.0)),
                            )
                            .clicked()
                        {
                            self.undo();
                        }
                    });
                    ui.separator();
                }
                ui.columns(3, |columns| {
                    navigation_tab(
                        &mut columns[0],
                        &mut self.mobile_view,
                        MobileView::Play,
                        "Play",
                    );
                    navigation_tab(
                        &mut columns[1],
                        &mut self.mobile_view,
                        MobileView::Scores,
                        "Scores",
                    );
                    navigation_tab(
                        &mut columns[2],
                        &mut self.mobile_view,
                        MobileView::Settings,
                        "Settings",
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(31, 35, 39))
                    .inner_margin(10),
            )
            .show(ctx, |ui| {
                touch_spacing(ui);
                egui::ScrollArea::vertical()
                    .id_salt(("mobile_content", self.mobile_view as u8))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match self.mobile_view {
                            MobileView::Play => self.render_mobile_play(ui),
                            MobileView::Scores => self.render_mobile_scores(ui),
                            MobileView::Settings => self.render_mobile_settings(ui),
                        }
                    });
            });
    }

    fn render_mobile_play(&mut self, ui: &mut egui::Ui) {
        let pending = self.game.pending_action();
        current_player_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().interact_size.y = 24.0;
            let player = pending
                .as_ref()
                .and_then(|pending| pending.target_player_id())
                .and_then(|id| self.game.players().iter().find(|player| player.id() == id))
                .or_else(|| self.game.current_player());
            if let Some(player) = player {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(player.name());
                    status_chip(ui, player.status());
                    ui.label(format!(
                        "Score {}",
                        display_round_score(player, self.game.player_score(player.id()))
                    ));
                });
                if !player.hand().is_empty() {
                    ui.add_space(6.0);
                    render_hand(ui, player.hand());
                }
            }
            if pending.is_none()
                && !self.game.round_over()
                && let Some(odds) = self.game.current_player_odds()
            {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Bust {}", format_percent(odds.bust_probability)));
                    ui.label(format!("Draw EV {:.1}", odds.expected_score_after_draw));
                    ui.label(format!("Delta {:+.1}", odds.expected_score_delta));
                });
                ui.spacing_mut().interact_size.y = 44.0;
                egui::CollapsingHeader::new("Draw details").show(ui, |ui| {
                    ui.spacing_mut().interact_size.y = 24.0;
                    if let Some(player) = player {
                        ui.label(player_score_tooltip(
                            player,
                            self.game.player_score(player.id()),
                        ));
                    }
                    ui.label(format!("Next-card pool: {}", odds.next_pool_size));
                    render_bust_hover(ui, &odds.bust_cards);
                    egui::CollapsingHeader::new("Expected score").show(ui, |ui| {
                        for detail in &odds.expected_value_details {
                            ui.horizontal_wrapped(|ui| {
                                render_card(ui, detail.card);
                                ui.label(format!(
                                    "{} copies | {}",
                                    detail.count,
                                    format_percent(detail.probability)
                                ));
                                ui.label(format!("Score {:.1}", detail.score_after_draw));
                            });
                        }
                    });
                });
            }
        });
        ui.add_space(8.0);
        ui.label(&self.status);

        if let Some(action) = pending {
            if action.needs_target() {
                ui.colored_label(
                    egui::Color32::from_rgb(245, 184, 86),
                    format!("{}: choose a player", special_action_label(action.action())),
                );
                let targets: Vec<_> = self
                    .game
                    .legal_pending_targets()
                    .into_iter()
                    .filter_map(|id| {
                        self.game
                            .players()
                            .iter()
                            .find(|player| player.id() == id)
                            .map(|player| (id, player.name().to_owned()))
                    })
                    .collect();
                for (id, name) in targets {
                    if ui
                        .add_sized([ui.available_width(), 48.0], egui::Button::new(name))
                        .clicked()
                    {
                        self.choose_target(id);
                    }
                }
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(245, 184, 86),
                    format!(
                        "Flip Three: {} more cards{}",
                        action.remaining_draws(),
                        queued_action_suffix(self.game.queued_action_count())
                    ),
                );
            }
        }

        ui.add_space(8.0);
        self.render_mobile_deal(ui);
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Players")
            .default_open(true)
            .show(ui, |ui| {
                for player in self.game.players() {
                    player_frame(
                        player.status(),
                        self.game
                            .current_player()
                            .is_some_and(|current| current.id() == player.id()),
                    )
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(player.name());
                            status_chip(ui, player.status());
                            ui.label(format!(
                                "Score {}",
                                display_round_score(player, self.game.player_score(player.id()))
                            ));
                        });
                        render_hand(ui, player.hand());
                    });
                    ui.add_space(6.0);
                }
            });
    }

    fn render_mobile_deal(&mut self, ui: &mut egui::Ui) {
        let counts = self.game.next_draw_pool_counts();
        let enabled = !self.game.round_over()
            && self
                .game
                .pending_action()
                .is_none_or(PendingStep::awaiting_selected_draws);
        let mut dealt = None;
        manual_deal_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong("Deal card");
            for (group, label) in [(0, "Numbers"), (1, "Bonus"), (2, "Special")] {
                ui.small(label);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for &(card, count) in &counts {
                        if card.sort_key().0 != group {
                            continue;
                        }
                        let (fill, stroke, text) = card_colors(card);
                        let button = egui::Button::new(
                            egui::RichText::new(format!("{}\nx{}", card.short_label(), count))
                                .color(text)
                                .size(14.0),
                        )
                        .fill(fill)
                        .stroke(egui::Stroke::new(1.0_f32, stroke))
                        .min_size(egui::vec2(52.0, 48.0));
                        if ui
                            .add_enabled(enabled && dealt.is_none(), button)
                            .on_hover_text(card_tooltip(card))
                            .clicked()
                        {
                            dealt = Some(card);
                        }
                    }
                });
                ui.add_space(4.0);
            }
            egui::CollapsingHeader::new("Card details").show(ui, |ui| {
                for card in [Card::SecondChance, Card::FlipThree, Card::Freeze] {
                    ui.label(card_tooltip(card));
                }
            });
        });
        if let Some(card) = dealt {
            self.selected_card = Some(card);
            self.deal_selected_card();
        }
    }

    fn render_mobile_scores(&self, ui: &mut egui::Ui) {
        ui.heading("Scoreboard");
        for score in self.game.score_board() {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.strong(&score.player_name);
                ui.strong(format!("Total {}", score.total_score));
                if let Some(bank) = score.current_round_score {
                    ui.label(format!("Banked {bank}"));
                }
            });
            egui::CollapsingHeader::new("Round history")
                .id_salt(score.player_id.index())
                .show(ui, |ui| {
                    if score.round_scores.is_empty() {
                        ui.label("No completed rounds yet.");
                    }
                    for round in &score.round_scores {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(format!("Round {}: {}", round.round_number, round.score));
                            ui.label(round_outcome_label(round.outcome));
                        });
                        ui.label(round_bust_risk_tooltip(round));
                        ui.separator();
                    }
                });
        }
    }

    fn render_mobile_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.horizontal_wrapped(|ui| self.render_player_count_control(ui));
        if ui.button("Reset game").clicked() {
            self.reset_game();
            self.mobile_view = MobileView::Play;
        }
        ui.checkbox(&mut self.show_diagnostics, "Diagnostics");
        ui.separator();
        ui.heading("Deck");
        ui.label(format!(
            "{} draw / {} discard",
            self.game.draw_pile_count(),
            self.game.discard_pile_count()
        ));
        if ui
            .add_enabled(
                self.game.discard_pile_count() > 0,
                egui::Button::new("Reshuffle discard"),
            )
            .clicked()
        {
            self.reshuffle_discard();
        }
        ui.label(&self.status);
        egui::CollapsingHeader::new("Draw pile").show(ui, |ui| {
            render_card_count_column(ui, &self.game.draw_pile_counts(), |_| true);
        });
        egui::CollapsingHeader::new("Discard pile").show(ui, |ui| {
            render_card_count_column(ui, &self.game.discard_pile_counts(), |_| true);
        });
        #[cfg(target_arch = "wasm32")]
        ui.hyperlink_to("Licenses", "third-party-licenses.html");
    }
}

fn touch_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size.y = 44.0;
    ui.spacing_mut().button_padding = egui::vec2(10.0, 8.0);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(235, 232, 224));
}

fn navigation_tab(ui: &mut egui::Ui, selected: &mut MobileView, view: MobileView, label: &str) {
    if ui
        .add_sized(
            [ui.available_width(), 44.0],
            egui::Button::new(label).selected(*selected == view),
        )
        .clicked()
    {
        *selected = view;
    }
}
