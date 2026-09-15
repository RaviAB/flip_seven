#[cfg(not(target_arch = "wasm32"))]
use flip_seven::app::FlipSevenApp;

#[cfg(not(target_arch = "wasm32"))]
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
        Box::new(|cc| Ok(Box::new(FlipSevenApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    flip_seven::web::start();
}
