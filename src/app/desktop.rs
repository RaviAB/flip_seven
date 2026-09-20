use eframe::egui;

#[cfg(test)]
mod tests;

use super::FlipSevenApp;
use crate::model::{Card, PendingStep, Player, PlayerId};

use super::components::{render_card_count_column, render_hand, render_selectable_card_group};
use super::odds::render_odds;
use super::presentation::{
    deck_tooltip, display_round_score, player_score_tooltip, queued_action_suffix,
    special_action_label,
};
use super::scoreboard::render_scoreboard_player_row;
use super::theme::{
    current_player_frame, manual_deal_frame, neutral_panel_frame, player_frame, scoreboard_frame,
    status_chip,
};

impl FlipSevenApp {
    pub(super) fn render_desktop(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(31, 35, 39)))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(235, 232, 224));
                ui.heading("Flip 7 Simulator");
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 20.0).max(360.0) * 0.52);
                        self.render_controls(ui);
                        ui.add_space(10.0);
                        self.render_manual_deal_panel(ui);
                        ui.add_space(10.0);
                        self.render_current_player(ui);
                        ui.add_space(10.0);
                        self.render_scoreboard(ui);
                    });
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        neutral_panel_frame().show(ui, |ui| {
                            ui.label(egui::RichText::new("Players").strong());
                            ui.add_space(8.0);
                            self.render_players(ui);
                        });
                    });
                });
            });
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        neutral_panel_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                self.render_player_count_control(ui);

                let action_in_progress = self.game.pending_action().is_some();
                if ui
                    .add_enabled(
                        !action_in_progress && !self.game.round_over(),
                        egui::Button::new("Stay"),
                    )
                    .clicked()
                {
                    self.stay();
                }

                if ui
                    .add_enabled(self.game.round_over(), egui::Button::new("Next Round"))
                    .clicked()
                {
                    self.next_round();
                }

                if ui
                    .add_enabled(self.game.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }

                if ui.button("Reset").clicked() {
                    self.reset_game();
                }

                if ui
                    .add_enabled(
                        self.game.discard_pile_count() > 0,
                        egui::Button::new("Reshuffle discard"),
                    )
                    .on_hover_text("Move the discard pile back into the draw pile and shuffle it. Active player hands stay in play.")
                    .clicked()
                {
                    self.reshuffle_discard();
                }

                let deck_response = ui
                    .button(format!(
                        "Deck: {} draw / {} discard",
                        self.game.draw_pile_count(),
                        self.game.discard_pile_count()
                    ))
                    .on_hover_text(deck_tooltip(&self.game));
                if deck_response.clicked() {
                    self.show_deck_details = true;
                }

                ui.checkbox(&mut self.show_diagnostics, "Diagnostics")
                    .on_hover_text("Show FPS and frame timing. While enabled, the app requests continuous repainting.");
            });

            ui.add_space(8.0);
            ui.label(format!(
                "Round {} | {} cards available",
                self.game.round_number(),
                self.game.total_available_cards()
            ));

            if let Some(pending_action) = self.game.pending_action() {
                ui.colored_label(egui::Color32::from_rgb(245, 184, 86), {
                    if pending_action.awaiting_selected_draws() {
                        let target = pending_action
                            .target_player_id()
                            .and_then(|target_id| {
                                self.game
                                    .players()
                                    .iter()
                                    .find(|player| player.id() == target_id)
                            })
                            .map(Player::name)
                            .unwrap_or("the target");
                        format!(
                            "Flip Three for {target}: choose {} more card{} from Deal card.{}",
                            pending_action.remaining_draws(),
                            if pending_action.remaining_draws() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            queued_action_suffix(self.game.queued_action_count())
                        )
                    } else {
                        format!(
                            "{} is waiting for a target. Choose an active player below.{}",
                            special_action_label(pending_action.action()),
                            queued_action_suffix(self.game.queued_action_count())
                        )
                    }
                });
            }

            ui.label(&self.status);
        });
    }

    fn render_manual_deal_panel(&mut self, ui: &mut egui::Ui) {
        let available_cards = self.game.next_draw_pool_counts();
        if !available_cards
            .iter()
            .any(|(card, _)| Some(*card) == self.selected_card)
        {
            self.selected_card = available_cards.first().map(|(card, _)| *card);
        }

        manual_deal_frame().show(ui, |ui| {
            egui::CollapsingHeader::new("Deal card")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("Click any card chip to deal that card next.");
                    ui.add_space(4.0);
                    let can_deal = !self.game.round_over()
                        && self
                            .game
                            .pending_action()
                            .is_none_or(|pending_action| pending_action.awaiting_selected_draws());
                    let mut clicked_card = None;
                    render_selectable_card_group(
                        ui,
                        "Numbers",
                        &available_cards,
                        &mut self.selected_card,
                        can_deal,
                        &mut clicked_card,
                        |card| matches!(card, Card::Number(_)),
                    );
                    render_selectable_card_group(
                        ui,
                        "Bonus",
                        &available_cards,
                        &mut self.selected_card,
                        can_deal,
                        &mut clicked_card,
                        |card| matches!(card, Card::Bonus(_)),
                    );
                    render_selectable_card_group(
                        ui,
                        "Special",
                        &available_cards,
                        &mut self.selected_card,
                        can_deal,
                        &mut clicked_card,
                        |card| matches!(card, Card::SecondChance | Card::FlipThree | Card::Freeze),
                    );
                    if let Some(card) = clicked_card {
                        self.selected_card = Some(card);
                        self.deal_selected_card();
                    }
                });
        });
    }

    fn render_current_player(&self, ui: &mut egui::Ui) {
        current_player_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Current player").strong());
                if let Some(player) = self.game.current_player() {
                    ui.label(player.name());
                    status_chip(ui, player.status());
                    let score = self.game.player_score(player.id());
                    ui.label(format!(
                        "Score if round ended: {}",
                        display_round_score(player, score)
                    ))
                    .on_hover_text(player_score_tooltip(player, score));
                    ui.label(format!(
                        "Distinct numbers: {}",
                        player.distinct_number_count()
                    ))
                    .on_hover_text(
                        "Flip 7 triggers when a player reaches 7 distinct number cards.",
                    );
                } else {
                    ui.label("-");
                }
            });

            ui.add_space(8.0);
            if let Some(odds) = self.game.current_player_odds() {
                render_odds(ui, &odds);
            } else if self.game.round_over() {
                ui.label("Round over. Start the next round to continue.");
            } else {
                ui.label("No probability information available.");
            }
        });
    }

    fn render_scoreboard(&self, ui: &mut egui::Ui) {
        scoreboard_frame().show(ui, |ui| {
            ui.label(egui::RichText::new("Scoreboard").strong());
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("scoreboard_scroll")
                .max_height(240.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    for score in self.game.score_board() {
                        render_scoreboard_player_row(ui, score);
                    }
                });
        });
    }

    fn render_players(&mut self, ui: &mut egui::Ui) {
        let current_player_index = self.game.current_player_index();
        let pending_action = self.game.pending_action();
        let legal_targets = self.game.legal_pending_targets();
        let mut selected_target = None;
        egui::ScrollArea::vertical()
            .id_salt("players_scroll")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;

                for (index, player) in self.game.players().iter().enumerate() {
                    let is_current = current_player_index == Some(index);
                    if let Some(player_id) = self.render_player_panel(
                        ui,
                        player,
                        is_current,
                        pending_action.as_ref(),
                        &legal_targets,
                    ) {
                        selected_target = Some(player_id);
                    }
                }
            });

        if let Some(player_id) = selected_target {
            self.choose_target(player_id);
        }
    }

    fn render_player_panel(
        &self,
        ui: &mut egui::Ui,
        player: &Player,
        is_current: bool,
        pending_action: Option<&PendingStep>,
        legal_targets: &[PlayerId],
    ) -> Option<PlayerId> {
        let mut selected_target = None;
        let frame = player_frame(player.status(), is_current);

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let name = if is_current {
                            format!("CURRENT - {}", player.name())
                        } else {
                            player.name().to_owned()
                        };
                        let name_text = if is_current {
                            egui::RichText::new(name)
                                .strong()
                                .size(17.0)
                                .color(egui::Color32::from_rgb(255, 237, 145))
                        } else {
                            egui::RichText::new(name).strong().size(16.0)
                        };
                        ui.label(name_text);
                        status_chip(ui, player.status());
                    });

                    let score_response = ui.label(format!(
                        "Round score: {} | Distinct: {}",
                        display_round_score(player, self.game.player_score(player.id())),
                        player.distinct_number_count()
                    ));
                    score_response.on_hover_text(player_score_tooltip(
                        player,
                        self.game.player_score(player.id()),
                    ));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(pending_action) = pending_action
                        && pending_action.needs_target()
                    {
                        let enabled = legal_targets.contains(&player.id());
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(format!(
                                    "Choose for {}",
                                    special_action_label(pending_action.action())
                                )),
                            )
                            .clicked()
                        {
                            selected_target = Some(player.id());
                        }
                    }
                });
            });

            ui.add_space(5.0);
            render_hand(ui, player.hand());
        });

        selected_target
    }

    pub(super) fn render_deck_details(&mut self, ctx: &egui::Context) {
        if !self.show_deck_details {
            return;
        }

        let mut open = self.show_deck_details;
        egui::Window::new("Deck")
            .open(&mut open)
            .default_size([650.0, 420.0])
            .show(ctx, |ui| {
                let draw_counts = self.game.draw_pile_counts();
                let discard_counts = self.game.discard_pile_counts();
                ui.columns(3, |columns| {
                    columns[0].label(format!("Draw numbers ({})", self.game.draw_pile_count()));
                    render_card_count_column(&mut columns[0], &draw_counts, |card| {
                        matches!(card, Card::Number(_))
                    });

                    columns[1].label("Draw bonus/special");
                    render_card_count_column(&mut columns[1], &draw_counts, |card| {
                        !matches!(card, Card::Number(_))
                    });

                    columns[2].label(format!("Discard ({})", self.game.discard_pile_count()));
                    render_card_count_column(&mut columns[2], &discard_counts, |_| true);
                });
            });
        self.show_deck_details = open;
    }
}
