# Flip Seven Agent Guide

## Project Shape

- This is a Rust 2024 native GUI app using `eframe`/`egui`.
- `src/model/*` owns game state, deck/player models, scoring, special-card behavior, rounds, undo, selected-card deals, and probability calculations.
- `src/app.rs` owns egui rendering, layout, hover/detail panels, diagnostics, and user interaction wiring.
- `src/main.rs` should stay minimal and only bootstrap the native eframe app.

## Separation Of Concerns

- Keep game-rule decisions out of UI code. If the UI needs new game data, add or extend model-facing APIs.
- UI should consume model types such as `GameState`, `DrawOdds`, `DealOutcome`, `PlayerScore`, and card label/chip helpers.
- Keep rendering helpers focused on display concerns. Avoid duplicating scoring, deck, or special-card logic in egui handlers.

## Game Model Expectations

- Preserve the 94-card deck distribution:
  - Number cards `0..=12`, with `0` once and each `n` card appearing `n` times.
  - Three each of `SecondChance`, `FlipThree`, and `Freeze`.
  - One each of `+2`, `+4`, `+6`, `+8`, `+10`, and `x2`.
- The draw pile reshuffles from discard only when the draw pile is empty.
- Inactive players' hands move to discard when they become inactive.
- Manual selected-card deals must remove one matching card from the current next-draw pool and remain undoable.
- Special-card sequencing should preserve current behavior:
  - Second Chance is kept or used immediately.
  - Flip Three and Freeze drawn during Flip Three are queued and resolved after the current sequence.

## UI Expectations

- Keep card displays compact, using `Card::short_label()` and consistent chip rendering.
- Avoid white backgrounds. Maintain distinct panel colors for controls, manual deal, current player, scoreboard, and player status.
- Hover and expanded detail views should be fast and informational.
- Probability hovers should use precomputed model data, not expensive calculations during painting.
- Diagnostics/FPS UI is acceptable for investigating perceived freezes.

## Testing And Validation

- Add model tests for new card rules, scoring, deck/discard behavior, selected deals, undo, probability calculations, and multi-round scoreboard behavior.
- Run these before considering work complete:
  - `cargo fmt`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
- Smoke-test GUI changes with `cargo run`.

## Repo Hygiene

- Use `rg`/`sed` for inspection.
- Exclude `target/` from broad searches, for example: `rg --glob '!target/**' ...`.
- Do not edit or commit generated build artifacts under `target/`.
