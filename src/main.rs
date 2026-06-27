use flip_seven::app::FlipSevenApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Flip 7 Simulator")
            .with_inner_size([720.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Flip 7 Simulator",
        options,
        Box::new(|_cc| Ok(Box::<FlipSevenApp>::default())),
    )
}
