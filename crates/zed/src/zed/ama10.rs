//! Esperanta-local glue between the editor and the Ama10 / Wuling DevOps stack.
//!
//! `ama10` ships the Wuling DevOps OpenAPI client and `ama10-ui` grows the
//! user-facing surface for it. This module is the single entry point the
//! editor calls during startup so the rest of `crates/zed` doesn't have to
//! sprout fork-local module references everywhere.

use gpui::App;

pub fn init(cx: &mut App) {
    ama10_ui::init(cx);
}
