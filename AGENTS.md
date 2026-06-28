# Flip Seven Agent Guide

## Project Shape

- This is a Rust 2024 native GUI app using `eframe`/`egui`.
- `src/model/game.rs` owns live game state, scoring, special-card behavior, rounds, undo, selected-card deals, and UI-facing probability calculations.
- `src/model/strategy.rs` owns AI recommendations, random replay policy, rollout settings, strategy comparison, and offline simulation behavior.
- `src/model/deck.rs`, `src/model/player.rs`, and `src/model/card.rs` own focused deck, player, scoring, and card primitives.
- `src/app.rs` owns egui rendering, layout, hover/detail panels, diagnostics, and user interaction wiring.
- `src/main.rs` should stay minimal and only bootstrap the native eframe app.

## Separation Of Concerns

- Keep game-rule decisions out of UI code. If the UI needs new game data, add or extend model-facing APIs.
- UI should consume model types such as `GameState`, `DrawOdds`, `DealOutcome`, `PlayerScore`, and card label/chip helpers.
- Keep rendering helpers focused on display concerns. Avoid duplicating scoring, deck, or special-card logic in egui handlers.
- Keep AI/strategy/replay decisions out of live game-state APIs. Strategy code may clone game state for replay, but it should own target choice, draw/stay policy, rollout loops, and random card selection.
- Keep the native GUI focused on live game operation and model-provided analysis. Do not add offline simulation controls or run strategy rollouts in `src/app.rs` unless explicitly requested.

## Rust Code Health

- Prefer typed internal outcomes over strings or sentinel values. Card application, pending actions, scoring outcomes, and replay decisions should be represented with enums or structs.
- Avoid boolean parameters in rule code when an enum makes intent clearer.
- Keep public APIs narrow. Use `pub(crate)` for helpers that exist only to connect model modules or tests.
- Large modules are acceptable only while actively evolving behavior; when a section stabilizes, split it into focused modules rather than adding more unrelated helpers.

## Game Model Expectations

- Preserve the 94-card deck distribution:
  - Number cards `0..=12`, with `0` once and each `n` card appearing `n` times.
  - Three each of `SecondChance`, `FlipThree`, and `Freeze`.
  - One each of `+2`, `+4`, `+6`, `+8`, `+10`, and `x2`.
- The draw pile reshuffles from discard only when the draw pile is empty.
- Inactive players' hands move to discard when they become inactive.
- Manual selected-card deals must remove one matching card from the current next-draw pool and remain undoable.
- Undo tracking is part of live gameplay. Replay/simulation code may disable undo history on cloned state, but it must not change live undo semantics.
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
