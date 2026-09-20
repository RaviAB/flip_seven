use eframe::egui;

use crate::model::{Card, PlayerStatus};

pub(super) fn card_colors(card: Card) -> (egui::Color32, egui::Color32, egui::Color32) {
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

pub(super) fn status_chip(ui: &mut egui::Ui, status: PlayerStatus) {
    let (label, color) = match status {
        PlayerStatus::Active => ("Active", egui::Color32::from_rgb(107, 190, 134)),
        PlayerStatus::Stayed => ("Stayed", egui::Color32::from_rgb(224, 194, 91)),
        PlayerStatus::Frozen => ("Frozen", egui::Color32::from_rgb(108, 167, 229)),
        PlayerStatus::Busted => ("Busted", egui::Color32::from_rgb(222, 102, 96)),
    };

    ui.colored_label(color, label);
}

pub(super) fn player_frame(status: PlayerStatus, is_current: bool) -> egui::Frame {
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
    let stroke = egui::Stroke::new(if is_current { 3.0_f32 } else { 1.0_f32 }, stroke_color);

    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

pub(super) fn neutral_panel_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(39, 43, 47),
        egui::Color32::from_rgb(73, 77, 81),
    )
}

pub(super) fn manual_deal_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(34, 48, 61),
        egui::Color32::from_rgb(79, 142, 184),
    )
}

pub(super) fn current_player_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(42, 49, 43),
        egui::Color32::from_rgb(96, 145, 111),
    )
}

pub(super) fn scoreboard_frame() -> egui::Frame {
    app_panel_frame(
        egui::Color32::from_rgb(47, 42, 50),
        egui::Color32::from_rgb(122, 98, 143),
    )
}

fn app_panel_frame(fill: egui::Color32, stroke: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, stroke))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
}
