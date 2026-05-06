//! `ama10` — Kal'tsit·Esperanta fork-local code.
//!
//! Most non-trivial additions on top of upstream Zed land here so that the
//! source diff against `zed-industries/zed` stays small and rebases stay sane.
//! The Wuling DevOps API client used by the editor is one such addition.

pub mod wuling_api;
