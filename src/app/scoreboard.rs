use eframe::egui;

use crate::model::PlayerScore;

use super::presentation::{
    format_basis_points, optional_u32, round_bust_risk_tooltip, round_outcome_label,
    score_bust_risk_tooltip, score_history_tooltip, score_summary_label,
};

pub(super) fn render_scoreboard_player_row(ui: &mut egui::Ui, score: &PlayerScore) {
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
            score.telemetry.cards_dealt.to_string(),
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
            format_basis_points(score.telemetry.peak_bust_risk_basis_points),
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
