use crate::tasks::workflows::{
    runners::{Arch, Platform},
    steps::{CommonJobConditions, NamedJob},
};

use super::{runners, steps, steps::named, vars};
use gh_workflow::*;

pub(crate) fn build_nix(
    platform: Platform,
    arch: Arch,
    flake_output: &str,
    // Upstream uses this to filter what gets pushed to the Cachix binary cache.
    // The Esperanta fork doesn't push to a remote binary cache, so the filter
    // is unused. Kept in the signature so callers (release_nightly,
    // run_bundling) compile unchanged.
    _cachix_filter: Option<&str>,
    deps: &[&NamedJob],
) -> NamedJob {
    pub fn install_nix() -> Step<Use> {
        named::uses(
            "cachix",
            "install-nix-action",
            "02a151ada4993995686f9ed4f1be7cfbb229e56f", // v31
        )
        .add_with(("github_access_token", vars::GITHUB_TOKEN))
    }

    pub fn build(flake_output: &str) -> Step<Run> {
        named::bash(&format!(
            "nix build .#{} -L --accept-flake-config",
            flake_output
        ))
    }

    let runner = match platform {
        Platform::Windows => unimplemented!(),
        Platform::Linux => runners::LINUX_X86_BUNDLER,
        Platform::Mac => runners::MAC_DEFAULT,
    };
    let mut job = Job::default()
        .timeout_minutes(60u32)
        .continue_on_error(true)
        .with_repository_owner_guard()
        .runs_on(runner)
        .add_env(("ZED_CLIENT_CHECKSUM_SEED", vars::ZED_CLIENT_CHECKSUM_SEED))
        .add_env((
            "ZED_CLOUD_PROVIDER_ADDITIONAL_MODELS_JSON",
            vars::ZED_CLOUD_PROVIDER_ADDITIONAL_MODELS_JSON,
        ))
        .add_env(("GIT_LFS_SKIP_SMUDGE", "1")) // breaks the livekit rust sdk examples which we don't actually depend on
        .add_step(steps::checkout_repo());

    if deps.len() > 0 {
        job = job.needs(deps.iter().map(|d| d.name.clone()).collect::<Vec<String>>());
    }

    // Upstream's Linux flow used Namespace's `nscloud-cache-action` to
    // bind-mount a persistent /nix store before `install-nix-action`, and
    // `cachix-action` to push the build closure to a remote binary cache. The
    // fork does neither: the cache helpers are no-ops on GH-hosted runners
    // (see steps::cache_nix_*), and the cachix push is dropped because the
    // `zed` Cachix workspace isn't ours. The build still runs locally; it
    // just always rebuilds from scratch.
    job = match platform {
        Platform::Linux => job
            .add_step(steps::cache_nix_dependencies_namespace())
            .add_step(install_nix())
            .add_step(build(flake_output)),
        Platform::Mac => job
            .add_step(steps::cache_nix_store_macos())
            .add_step(install_nix())
            .add_step(build(flake_output)),
        Platform::Windows => unimplemented!(),
    };

    NamedJob {
        name: format!("build_nix_{platform}_{arch}"),
        job,
    }
}
