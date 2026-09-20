mod app;
mod model;

pub use app::FlipSevenApp;

#[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
pub mod simulation;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::start;
