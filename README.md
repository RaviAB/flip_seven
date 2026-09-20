# Flip Seven

A Rust/egui scorekeeper and probability companion with a native desktop app
and an Android-friendly web app. One device manages the table. Game state is
kept in memory only: reloading or closing the app starts a new game.

## Desktop

```sh
cargo run
```

Offline strategy simulations remain available separately:

```sh
cargo run --release --bin offline -- --help
```

The library keeps gameplay internals private. Native simulation consumers use
the feature-gated `SimulationConfig`, `ComparisonMode`, `StrategyKind`,
`SimulationReport`, and `run_simulation` facade. Invalid settings are reported
as errors rather than silently adjusted.

Simulation reports distinguish completed matches from matches stopped by
`max_rounds`. Only a unique leader who reaches the target wins. Win rates,
confidence intervals, and average rounds to finish use completed matches;
score, risk, and seat-exposure statistics include all runs. With no completed
matches, numeric win rate and average rounds to finish are zero, the confidence
interval is `[0, 1]`, and the table displays `-` for win rate. JSON and CSV include
completion and round-limit counts. Rejected commands or stalled rounds fail the
simulation instead of producing a partial success report.

Both comparison modes support `min_matches`, `max_matches`, and `win_ci_width`.
Match limits are per seating; mirrored seatings and multiple strategy pairings
increase the total number of runs. Convergence uses completed matches and extends
the seeded sequences without rerunning earlier samples. Results are aggregated
in bounded batches with the same output for serial and parallel execution.

When a player who already holds Second Chance draws another, the duplicate is
given immediately to an eligible active player. If nobody can receive it, the
card is discarded. During Flip Three, this transfer pauses the sequence; other
special cards are resolved later in draw order.

## Web Development

Install Rust, Node.js 22+, and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked --version 0.21.14
env NO_COLOR=true trunk serve --address 0.0.0.0 --port 8080
```

Open `http://localhost:8080`. A phone on the same Wi-Fi can open the computer's
LAN address on port 8080. Home-screen installation and offline operation require
HTTPS on the phone; use the deployed GitHub Pages URL for those checks.

Below 700 logical pixels (or in a short phone landscape viewport), the app uses
touch-sized controls with Play, Scores, and Settings views. Desktop keeps its
two-column layout. Game rules and undo behavior are shared between both builds.

## GitHub Pages

Create an **empty public** GitHub repository named `flip_seven`, then push this
repository's history. With GitHub CLI installed and authenticated:

```sh
gh repo create RaviAB/flip_seven --public --source=. --remote=origin --push
gh api --method POST repos/RaviAB/flip_seven/pages -f build_type=workflow
gh workflow run pages.yml
```

Alternatively, set **Settings > Pages > Build and deployment > Source** to
**GitHub Actions**, then run the GitHub Pages workflow from the Actions tab.
The workflow also runs automatically on pushes to `main`; pull requests build
and validate without publishing. After the first successful deployment, the
default address is `https://raviab.github.io/flip_seven/`.

The build uses relative asset paths, so it works under the repository subpath,
at a user-site root, or on a custom domain without changing source URLs.

In Android Chrome, open the deployed page and use the browser's install / add
to home screen action. Once the service worker finishes caching, the app can
start offline. Only app assets are cached, never games. New releases activate
after all open instances close, avoiding a reload during play.

For a local offline smoke test, serve a production `dist/` build with a static
server on localhost and open `/?pwa`. Regular local previews don't register a
service worker, to avoid stale development builds.

## Validation

```sh
cargo fmt
cargo test
cargo test --no-default-features
cargo clippy --all-targets -- -D warnings
cargo clippy --target wasm32-unknown-unknown --no-default-features --bin flip_seven -- -D warnings
env NO_COLOR=true trunk build --release
```

Smoke-test the native app with `cargo run`. For web releases, check portrait
and landscape layouts, dealing, undo, special-card targets, scoreboard,
installation, offline reload, and upgrading from an older cached build.

## License

Original source code and generated app icons are licensed under the
[MIT License](LICENSE), copyright 2026 RaviAB.

This is an independent companion, not an official Flip 7 product. The license
does not cover third-party names, trademarks, artwork, or other assets.
No publisher artwork or rulebook is included.

Rust dependencies retain their own licenses. The web distribution includes
`third-party-licenses.html`, generated with cargo-about, and the notices for
egui's bundled fonts. Regenerate these after changing dependencies:

```sh
cargo install cargo-about --locked --version 0.9.2 --features cli
node scripts/update-licenses.mjs
```

The original app icons can be regenerated with `scripts/make-icons.py` using
Python and Pillow; the checked-in PNG files are used by normal builds.
