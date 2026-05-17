//! `ama10-ui` — Wuling DevOps user-facing surface inside Kal'tsit·Esperanta.
//!
//! Anything that ends up on screen for the Wuling DevOps integration (panels,
//! modals, command-palette actions, etc.) lives here so the engine's
//! `crates/zed` glue can stay tiny — see `crates/zed/src/zed/ama10.rs`.
//!
//! Today this is a placeholder: the SDK in `ama10::wuling_api` is shipped but
//! the auth migration (Stage 2 of the README roadmap) hasn't landed yet, so
//! there's nothing to mount. Keep `init` cheap and side-effect-free until the
//! first real view shows up.

use gpui::App;

pub fn init(_cx: &mut App) {}
