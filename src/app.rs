use eframe::egui;
use std::time::Duration;
use web_time::Instant;

mod mobile;

use crate::model::{
    ActionChoice, BustCardDetail, Card, DealOutcome, DrawOdds, ExpectedValueDetail, GameState,
    PendingAction, Player, PlayerId, PlayerScore, PlayerStatus, RoundOutcome, RoundScore,
    ScoreBreakdown, SpecialAction,
};

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
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(egui::Color32::from_rgb(31, 35, 39)))
                .show(ctx, |ui| {
                    ui.visuals_mut().override_text_color =
                        Some(egui::Color32::from_rgb(235, 232, 224));
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

    fn render_player_count_control(&mut self, ui: &mut egui::Ui) {
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
                    let breakdown = player.score_breakdown(false);
                    ui.label(format!("Score if round ended: {}", breakdown.total))
                        .on_hover_text(score_breakdown_tooltip(&breakdown));
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
        let pending_action = self.game.pending_action().cloned();
        let mut selected_target = None;
        let players = self.game.players().to_vec();

        egui::ScrollArea::vertical()
            .id_salt("players_scroll")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;

                for (index, player) in players.iter().enumerate() {
                    let is_current = current_player_index == Some(index);
                    if let Some(player_id) =
                        self.render_player_panel(ui, player, is_current, pending_action.as_ref())
                    {
                        selected_target = Some(player_id);
                    }
                }
            });

        if let Some(player_id) = selected_target {
            self.choose_target(player_id);
        }
    }

    fn render_player_panel(
        &mut self,
        ui: &mut egui::Ui,
        player: &Player,
        is_current: bool,
        pending_action: Option<&PendingAction>,
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
                        let enabled = player.status() == PlayerStatus::Active;
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

    fn render_deck_details(&mut self, ctx: &egui::Context) {
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
                        1.0,
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
        self.status = status_for_outcome(outcome);
    }

    fn stay(&mut self) {
        let outcome = self.game.stay_current_player();
        self.status = status_for_outcome(outcome);
    }

    fn next_round(&mut self) {
        let outcome = self.game.start_next_round();
        self.status = status_for_outcome(outcome);
    }

    fn reset_game(&mut self) {
        self.normalize_player_count_input();
        self.game.reset(self.selected_player_count);
        self.status = format!(
            "Reset for {} player{}.",
            self.selected_player_count,
            if self.selected_player_count == 1 {
                ""
            } else {
                "s"
            }
        );
    }

    fn undo(&mut self) {
        let outcome = self.game.undo();
        if matches!(outcome, DealOutcome::UndoApplied) {
            self.selected_player_count = self.game.players().len();
            self.player_count_input = self.selected_player_count.to_string();
        }
        self.status = status_for_outcome(outcome);
    }

    fn reshuffle_discard(&mut self) {
        let outcome = self.game.reshuffle_discard_into_draw_pile();
        self.status = status_for_outcome(outcome);
    }

    fn choose_target(&mut self, player_id: PlayerId) {
        let outcome = self
            .game
            .resolve_pending_action(ActionChoice::Player(player_id));
        self.status = status_for_outcome(outcome);
    }

    fn normalize_player_count_input(&mut self) {
        if let Ok(value) = self.player_count_input.parse::<usize>() {
            self.selected_player_count = value.clamp(MIN_PLAYERS, MAX_PLAYERS);
        }
        self.player_count_input = self.selected_player_count.to_string();
    }
}

fn render_odds(ui: &mut egui::Ui, odds: &DrawOdds) {
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

fn render_scoreboard_player_row(ui: &mut egui::Ui, score: &PlayerScore) {
    egui::CollapsingHeader::new(score_summary_label(score))
        .id_salt(("score_player_details", score.player_id.index()))
        .default_open(false)
        .show(ui, |ui| {
            render_scoreboard_player_details(ui, score);
        })
        .header_response
        .on_hover_text(score_history_tooltip(score));
}

fn render_scoreboard_player_details(ui: &mut egui::Ui, score: &PlayerScore) {
    ui.horizontal_wrapped(|ui| {
        render_score_stat(
            ui,
            "Avg",
            score
                .average_round_score()
                .map(|value| format!("{value:.1}"))
                .unwrap_or_else(|| "-".to_owned()),
            "Average score across completed rounds.",
        );
        render_score_stat(
            ui,
            "Best",
            optional_u32(score.best_round_score()),
            "Best completed round score.",
        );
        render_score_stat(
            ui,
            "Worst",
            optional_u32(score.worst_round_score()),
            "Worst completed round score.",
        );
        render_score_stat(
            ui,
            "Cards",
            score.total_cards_dealt.to_string(),
            "Cards dealt to this player, including Flip Three draws.",
        );
        render_score_stat(
            ui,
            "Avg bust",
            score
                .average_bust_risk_basis_points()
                .map(format_basis_points)
                .unwrap_or_else(|| "-".to_owned()),
            &score_bust_risk_tooltip(score),
        );
        render_score_stat(
            ui,
            "Peak bust",
            format_basis_points(score.peak_bust_risk_basis_points),
            "Highest effective pre-draw bust probability this player has faced.",
        );
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.small(format!("Rounds {}", score.rounds_completed));
        ui.small(format!("Stayed {}", score.stayed_count));
        ui.small(format!("Frozen {}", score.frozen_count));
        ui.small(format!("Busted {}", score.busted_count));
        ui.small(format!("Flip 7 {}", score.flip_seven_count));
        ui.small(format!("Active end {}", score.round_ended_active_count));
        if let Some(outcome) = score.current_round_outcome {
            ui.small(format!("Current {}", round_outcome_label(outcome)));
        }
    });

    ui.add_space(2.0);
    egui::CollapsingHeader::new("Round history")
        .id_salt(("score_round_history", score.player_id.index()))
        .default_open(false)
        .show(ui, |ui| {
            render_round_history(ui, score);
        });
}

fn score_summary_label(score: &PlayerScore) -> String {
    let avg_bust = score
        .average_bust_risk_basis_points()
        .map(format_basis_points)
        .unwrap_or_else(|| "-".to_owned());
    let current = score
        .current_round_score
        .map(|score| format!(" | Bank {score}"))
        .unwrap_or_default();

    format!(
        "{} | T {} | L {} | R {} | Bust {}{}",
        score.player_name,
        score.total_score,
        score.last_round_score,
        score.rounds_completed,
        avg_bust,
        current
    )
}

fn render_score_stat(ui: &mut egui::Ui, label: &str, value: String, hover: &str) {
    ui.vertical(|ui| {
        ui.small(label);
        ui.label(egui::RichText::new(value).strong())
            .on_hover_text(hover);
    });
}

fn render_round_history(ui: &mut egui::Ui, score: &PlayerScore) {
    if score.round_scores.is_empty() {
        ui.label("No completed rounds yet.");
        return;
    }

    egui::Grid::new(("round_history_grid", score.player_id.index()))
        .num_columns(6)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Round");
            ui.strong("Score");
            ui.strong("Outcome");
            ui.strong("Cards");
            ui.strong("Avg bust");
            ui.strong("Peak");
            ui.end_row();

            for round in &score.round_scores {
                ui.label(round.round_number.to_string());
                ui.label(round.score.to_string());
                ui.label(round_outcome_label(round.outcome));
                ui.label(round.cards_dealt.to_string());
                ui.label(format_basis_points(round.average_bust_risk_basis_points))
                    .on_hover_text(round_bust_risk_tooltip(round));
                ui.label(format_basis_points(round.peak_bust_risk_basis_points))
                    .on_hover_text(round_bust_risk_tooltip(round));
                ui.end_row();
            }
        });
}

fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn round_outcome_label(outcome: RoundOutcome) -> &'static str {
    match outcome {
        RoundOutcome::Stayed => "Stayed",
        RoundOutcome::Frozen => "Frozen",
        RoundOutcome::Busted => "Busted",
        RoundOutcome::FlipSeven => "Flip 7",
        RoundOutcome::RoundEndedActive => "Round end",
    }
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
                    1.0,
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

fn render_hand(ui: &mut egui::Ui, hand: &[Card]) {
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

fn render_card(ui: &mut egui::Ui, card: Card) {
    render_card_chip(ui, card, false).on_hover_text(card_tooltip(card));
}

fn render_selectable_card_group(
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

fn render_card_count_column(
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
                ui.small(format!("x{count}")).on_hover_text(format!(
                    "{} copies of {} in this pile.",
                    count,
                    card.label()
                ));
                ui.end_row();
            }
        });

    if !rendered_any {
        ui.label("-");
    }
}

fn render_card_chip(ui: &mut egui::Ui, card: Card, selected: bool) -> egui::Response {
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
        card_chip_size(),
        egui::Button::new(text)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(5)),
    )
}

fn card_chip_size() -> egui::Vec2 {
    egui::vec2(38.0, 24.0)
}

fn display_round_score(player: &Player, score: Option<&PlayerScore>) -> u32 {
    score
        .and_then(|score| score.current_round_score)
        .unwrap_or_else(|| player.round_score(false))
}

fn player_score_tooltip(player: &Player, score: Option<&PlayerScore>) -> String {
    let mut lines = Vec::new();
    if let Some(score) = score.and_then(|score| score.current_round_score) {
        lines.push(format!("Banked this round: {score}"));
        lines.push(
            "Hand has already moved to discard because this player is no longer active.".to_owned(),
        );
    } else {
        lines.push(score_breakdown_tooltip(&player.score_breakdown(false)));
    }
    lines.push(format!(
        "Distinct number cards: {}",
        player.distinct_number_count()
    ));
    lines.push(format!("Cards currently in hand: {}", player.hand().len()));
    lines.join("\n")
}

fn score_breakdown_tooltip(breakdown: &ScoreBreakdown) -> String {
    format!(
        "Numbers: {}\nMultiplier: x{}\nBonuses: +{}\nFlip 7 bonus: +{}\nTotal: {}",
        breakdown.number_sum,
        breakdown.multiplier,
        breakdown.additive_bonus,
        breakdown.flip_seven_bonus,
        breakdown.total
    )
}

fn protected_duplicate_probability(details: &[BustCardDetail]) -> f64 {
    details
        .iter()
        .filter(|detail| detail.protected_by_second_chance)
        .map(|detail| detail.probability)
        .sum()
}

fn bust_probability_label(odds: &DrawOdds) -> String {
    let protected = protected_duplicate_probability(&odds.bust_cards);
    if protected > 0.0 {
        format!(
            "{} (SC {})",
            format_percent(odds.bust_probability),
            format_percent(protected)
        )
    } else {
        format_percent(odds.bust_probability)
    }
}

fn render_bust_hover(ui: &mut egui::Ui, details: &[BustCardDetail]) {
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

fn score_history_tooltip(score: &PlayerScore) -> String {
    let mut lines = vec![
        format!("Total: {}", score.total_score),
        format!("Last completed round: {}", score.last_round_score),
        format!(
            "Average round: {}",
            score
                .average_round_score()
                .map(|value| format!("{value:.1}"))
                .unwrap_or_else(|| "-".to_owned())
        ),
        format!(
            "Best / worst: {} / {}",
            optional_u32(score.best_round_score()),
            optional_u32(score.worst_round_score())
        ),
    ];

    if let Some(current_round_score) = score.current_round_score {
        lines.push(format!("Banked current round: {current_round_score}"));
    }

    if score.round_scores.is_empty() {
        lines.push("No completed round history yet.".to_owned());
    } else {
        lines.push("Completed rounds:".to_owned());
        for round_score in &score.round_scores {
            lines.push(format!(
                "Round {}: {}",
                round_score.round_number, round_score.score
            ));
        }
    }

    lines.join("\n")
}

fn score_bust_risk_tooltip(score: &PlayerScore) -> String {
    if score.bust_risk_sample_count == 0 {
        return "No dealt cards have produced a pre-draw risk sample yet.".to_owned();
    }

    format!(
        "Average effective pre-draw bust risk across {} dealt cards.\nPeak risk: {}\nSecond Chance protected duplicate risk on {} sample{}.",
        score.bust_risk_sample_count,
        format_basis_points(score.peak_bust_risk_basis_points),
        score.second_chance_protected_samples,
        if score.second_chance_protected_samples == 1 {
            ""
        } else {
            "s"
        }
    )
}

fn round_bust_risk_tooltip(round: &RoundScore) -> String {
    if round.bust_risk_samples == 0 {
        return "No dealt cards produced a pre-draw risk sample in this round.".to_owned();
    }

    format!(
        "Average effective pre-draw bust risk across {} dealt card{} this round.\nPeak risk: {}\nSecond Chance protected duplicate risk on {} sample{}.",
        round.bust_risk_samples,
        if round.bust_risk_samples == 1 {
            ""
        } else {
            "s"
        },
        format_basis_points(round.peak_bust_risk_basis_points),
        round.second_chance_protected_samples,
        if round.second_chance_protected_samples == 1 {
            ""
        } else {
            "s"
        }
    )
}

fn deck_tooltip(game: &GameState) -> String {
    format!(
        "Click to inspect deck contents.\nDraw pile: {}\nDiscard pile: {}\nAvailable cards: {}\nWhen the draw pile empties, the discard pile is shuffled back into the deck.",
        game.draw_pile_count(),
        game.discard_pile_count(),
        game.total_available_cards()
    )
}

fn card_tooltip(card: Card) -> String {
    match card {
        Card::Number(value) => format!(
            "Number card {value}\nAdds {value} points.\nDrawing a duplicate number busts unless the player has Second Chance."
        ),
        Card::Bonus(crate::model::BonusCard::Plus(value)) => {
            format!("Bonus card +{value}\nAdds {value} points when the player scores.")
        }
        Card::Bonus(crate::model::BonusCard::Double) => {
            "Bonus card x2\nDoubles the sum of number cards before additive bonuses.".to_owned()
        }
        Card::SecondChance => {
            "Second Chance\nPrevents one duplicate-number bust, then moves to discard.".to_owned()
        }
        Card::FlipThree => {
            "Flip Three\nChoose an active player; they draw up to three cards.".to_owned()
        }
        Card::Freeze => {
            "Freeze\nChoose an active player; they bank their current score and their hand moves to discard.".to_owned()
        }
    }
}

fn card_colors(card: Card) -> (egui::Color32, egui::Color32, egui::Color32) {
    match card {
        Card::Number(value) => match value {
            0..=3 => (
                egui::Color32::from_rgb(189, 230, 211),
                egui::Color32::from_rgb(70, 155, 116),
                egui::Color32::from_rgb(15, 65, 46),
            ),
            4..=6 => (
                egui::Color32::from_rgb(196, 216, 245),
                egui::Color32::from_rgb(76, 126, 190),
                egui::Color32::from_rgb(24, 58, 104),
            ),
            7..=9 => (
                egui::Color32::from_rgb(245, 215, 162),
                egui::Color32::from_rgb(198, 137, 48),
                egui::Color32::from_rgb(90, 56, 13),
            ),
            _ => (
                egui::Color32::from_rgb(244, 190, 188),
                egui::Color32::from_rgb(191, 81, 78),
                egui::Color32::from_rgb(92, 31, 30),
            ),
        },
        Card::Bonus(_) => (
            egui::Color32::from_rgb(241, 224, 139),
            egui::Color32::from_rgb(196, 155, 38),
            egui::Color32::from_rgb(75, 58, 9),
        ),
        Card::SecondChance => (
            egui::Color32::from_rgb(214, 196, 243),
            egui::Color32::from_rgb(125, 87, 181),
            egui::Color32::from_rgb(63, 39, 100),
        ),
        Card::FlipThree => (
            egui::Color32::from_rgb(177, 222, 227),
            egui::Color32::from_rgb(48, 143, 154),
            egui::Color32::from_rgb(21, 73, 80),
        ),
        Card::Freeze => (
            egui::Color32::from_rgb(185, 211, 248),
            egui::Color32::from_rgb(55, 118, 202),
            egui::Color32::from_rgb(20, 56, 111),
        ),
    }
}

fn status_chip(ui: &mut egui::Ui, status: PlayerStatus) {
    let (label, color) = match status {
        PlayerStatus::Active => ("Active", egui::Color32::from_rgb(107, 190, 134)),
        PlayerStatus::Stayed => ("Stayed", egui::Color32::from_rgb(224, 194, 91)),
        PlayerStatus::Frozen => ("Frozen", egui::Color32::from_rgb(108, 167, 229)),
        PlayerStatus::Busted => ("Busted", egui::Color32::from_rgb(222, 102, 96)),
    };

    ui.colored_label(color, label);
}

fn special_action_label(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::FlipThree => "Flip Three",
        SpecialAction::Freeze => "Freeze",
    }
}

fn player_frame(status: PlayerStatus, is_current: bool) -> egui::Frame {
    let fill = if is_current {
        egui::Color32::from_rgb(63, 58, 42)
    } else {
        egui::Color32::from_rgb(41, 43, 48)
    };
    let stroke_color = if is_current {
        egui::Color32::from_rgb(255, 215, 92)
    } else {
        match status {
            PlayerStatus::Active => egui::Color32::from_rgb(107, 190, 134),
            PlayerStatus::Stayed => egui::Color32::from_rgb(224, 194, 91),
            PlayerStatus::Frozen => egui::Color32::from_rgb(108, 167, 229),
            PlayerStatus::Busted => egui::Color32::from_rgb(222, 102, 96),
        }
    };
    let stroke = egui::Stroke::new(if is_current { 3.0 } else { 1.0 }, stroke_color);

    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn neutral_panel_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(39, 43, 47),
        egui::Color32::from_rgb(73, 77, 81),
    )
}

fn manual_deal_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(34, 48, 61),
        egui::Color32::from_rgb(79, 142, 184),
    )
}

fn current_player_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(42, 49, 43),
        egui::Color32::from_rgb(96, 145, 111),
    )
}

fn scoreboard_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(47, 42, 50),
        egui::Color32::from_rgb(122, 98, 143),
    )
}

fn app_panel_frame(fill: egui::Color32, stroke: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn format_basis_points(value: u32) -> String {
    format!("{:.1}%", value as f64 / 100.0)
}

fn status_for_outcome(outcome: DealOutcome) -> String {
    match outcome {
        DealOutcome::DealtNumber {
            player_name, card, ..
        } => format!("{player_name} received {card}."),
        DealOutcome::Bonus {
            player_name, card, ..
        } => format!("{player_name} received bonus {card}."),
        DealOutcome::SecondChance { player_name, .. } => {
            format!("{player_name} received Second Chance.")
        }
        DealOutcome::UsedSecondChance {
            player_name,
            duplicate,
            ..
        } => format!("{player_name} used Second Chance on duplicate {duplicate}."),
        DealOutcome::Busted {
            player_name,
            duplicate,
            ..
        } => format!("{player_name} busted on duplicate {duplicate}."),
        DealOutcome::Stayed { player_name, .. } => format!("{player_name} stayed."),
        DealOutcome::FlipSeven { player_name, .. } => {
            format!("{player_name} hit Flip 7 and ended the round.")
        }
        DealOutcome::RoundEnded { reason } => format!("Round ended. {reason}"),
        DealOutcome::NewRoundStarted => "Started the next round.".to_owned(),
        DealOutcome::SpecialNeedsTarget {
            action,
            source_player_name,
            ..
        } => format!(
            "{source_player_name} drew {}; choose an active player.",
            special_action_label(action)
        ),
        DealOutcome::SpecialResolved {
            action,
            target_player_name,
            drawn_cards,
            notes,
            ..
        } => {
            let cards = if drawn_cards.is_empty() {
                String::new()
            } else {
                format!(
                    " Cards: {}.",
                    drawn_cards
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let notes = if notes.is_empty() {
                String::new()
            } else {
                format!(" {}", notes.join(" "))
            };
            format!(
                "{} resolved for {target_player_name}.{cards}{notes}",
                special_action_label(action)
            )
        }
        DealOutcome::FlipThreeTargetSelected {
            target_player_name,
            remaining_draws,
            ..
        } => format!(
            "Flip Three targeted {target_player_name}. Choose {remaining_draws} card{} from Deal card.",
            if remaining_draws == 1 { "" } else { "s" }
        ),
        DealOutcome::FlipThreeCardDealt {
            target_player_name,
            card,
            remaining_draws,
            notes,
            ..
        } => {
            let notes = if notes.is_empty() {
                String::new()
            } else {
                format!(" {}", notes.join(" "))
            };
            if remaining_draws == 0 {
                format!(
                    "Flip Three dealt {card} to {target_player_name}. Flip Three complete.{notes}"
                )
            } else {
                format!(
                    "Flip Three dealt {card} to {target_player_name}. Choose {remaining_draws} more card{}.{notes}",
                    if remaining_draws == 1 { "" } else { "s" }
                )
            }
        }
        DealOutcome::WaitingForTarget { action } => {
            format!(
                "Choose a target for {} before dealing.",
                special_action_label(action)
            )
        }
        DealOutcome::InvalidTarget => "Choose an active player for that special card.".to_owned(),
        DealOutcome::UndoApplied => "Undid the last action.".to_owned(),
        DealOutcome::NothingToUndo => "Nothing to undo.".to_owned(),
        DealOutcome::NoPlayers => "Add at least one player before dealing.".to_owned(),
        DealOutcome::NoActivePlayers => {
            "No active players remain. Start the next round.".to_owned()
        }
        DealOutcome::RoundOver => "Round over. Start the next round to continue.".to_owned(),
        DealOutcome::DeckEmpty => "No cards are available to draw.".to_owned(),
        DealOutcome::SelectedCardUnavailable => {
            "That card is not available in the current draw pile.".to_owned()
        }
        DealOutcome::DiscardReshuffled { cards_moved } => {
            format!(
                "Reshuffled {cards_moved} discard card{} into the draw pile.",
                if cards_moved == 1 { "" } else { "s" }
            )
        }
        DealOutcome::NoDiscardToReshuffle => "There are no discard cards to reshuffle.".to_owned(),
    }
}

fn queued_action_suffix(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!(
            " {count} queued special action{} after this.",
            if count == 1 { "" } else { "s" }
        )
    }
}
