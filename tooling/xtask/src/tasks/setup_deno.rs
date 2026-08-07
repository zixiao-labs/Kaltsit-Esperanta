#![allow(clippy::disallowed_methods, reason = "tooling is exempt")]

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};
use cargo_toml::Manifest;
use clap::Parser;

use crate::workspace::load_workspace;

#[derive(Parser)]
pub struct SetupDenoArgs {
    /// Skip the warm-up `cargo check` that pulls the V8 prebuilt.
    #[arg(long)]
    skip_build: bool,
}

pub fn run_setup_deno(args: SetupDenoArgs) -> Result<()> {
    let metadata = load_workspace()?;
    let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();
    let version = read_deno_core_version(&workspace_root)?;
    eprintln!("Pinned deno_core version: {version}");
    eprintln!("Feature flag: extension_deno/deno-core (wired as zed/deno-core)");

    if args.skip_build {
        eprintln!("Skipping cargo check warm-up (--skip-build).");
        return Ok(());
    }

    eprintln!("Warming V8 / deno_core via cargo check (this may download a large prebuilt)...");
    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .args([
            "check",
            "-p",
            "extension_deno",
            "--features",
            "deno-core",
        ])
        .status()
        .context("running cargo check -p extension_deno --features deno-core")?;
    if !status.success() {
        bail!("cargo check failed while preparing deno-core / V8");
    }

    eprintln!();
    eprintln!("Done. Production bundles can enable Deno with:");
    eprintln!("  ./build-mac.sh --with-deno");
    eprintln!("  cargo build -p zed --release --features deno-core");
    Ok(())
}

fn read_deno_core_version(workspace_root: &Path) -> Result<String> {
    let manifest_path = workspace_root.join("crates/extension_deno/Cargo.toml");
    let manifest = Manifest::from_path(&manifest_path)
        .with_context(|| format!("parsing {}", manifest_path.display()))?;

    let dep = manifest
        .dependencies
        .get("deno_core")
        .context("extension_deno Cargo.toml is missing deno_core dependency")?;
    let req = dep.req();
    let version = req.strip_prefix('^').unwrap_or(req).to_owned();
    if version.is_empty() {
        bail!("deno_core dependency is missing a version");
    }
    Ok(version)
}
