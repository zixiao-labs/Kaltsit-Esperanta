use crate::tasks::workflows::{
    nix_build::build_nix,
    release::{ReleaseBundleJobs, download_workflow_artifacts, prep_release_artifacts},
    run_bundling::{bundle_linux, bundle_mac, bundle_windows},
    run_tests::{clippy, run_platform_tests_no_filter},
    runners::{Arch, Platform, ReleaseChannel},
    steps::{CommonJobConditions, FluentBuilder, NamedJob},
};

use super::{runners, steps, steps::named, vars};
use gh_workflow::*;

/// Generates the release_nightly.yml workflow
pub fn release_nightly() -> Workflow {
    let style = check_style();
    // run only on windows as that's our fastest platform right now.
    let tests = run_platform_tests_no_filter(Platform::Windows);
    let clippy_job = clippy(Platform::Windows, None);
    let nightly = Some(ReleaseChannel::Nightly);

    let bundle = ReleaseBundleJobs {
        linux_aarch64: bundle_linux(Arch::AARCH64, nightly, &[&style, &tests, &clippy_job]),
        linux_x86_64: bundle_linux(Arch::X86_64, nightly, &[&style, &tests, &clippy_job]),
        mac_aarch64: bundle_mac(Arch::AARCH64, nightly, &[&style, &tests, &clippy_job]),
        mac_x86_64: bundle_mac(Arch::X86_64, nightly, &[&style, &tests, &clippy_job]),
        windows_aarch64: bundle_windows(Arch::AARCH64, nightly, &[&style, &tests, &clippy_job]),
        windows_x86_64: bundle_windows(Arch::X86_64, nightly, &[&style, &tests, &clippy_job]),
    };

    let nix_linux_x86 = build_nix(
        Platform::Linux,
        Arch::X86_64,
        "default",
        None,
        &[&style, &tests],
    );
    let nix_mac_arm = build_nix(
        Platform::Mac,
        Arch::AARCH64,
        "default",
        None,
        &[&style, &tests],
    );
    let publish_nightly = publish_nightly_release_job(&bundle);

    named::workflow()
        .on(Event::default()
            // Fire every day at 7:00am UTC (Roughly before EU workday and after US workday)
            .schedule([Schedule::new("0 7 * * *")])
            .push(Push::default().add_tag("nightly")))
        .add_env(("CARGO_TERM_COLOR", "always"))
        .add_env(("RUST_BACKTRACE", "1"))
        .add_job(style.name, style.job)
        .add_job(tests.name, tests.job)
        .add_job(clippy_job.name, clippy_job.job)
        .map(|mut workflow| {
            for job in bundle.into_jobs() {
                workflow = workflow.add_job(job.name, job.job);
            }
            workflow
        })
        .add_job(nix_linux_x86.name, nix_linux_x86.job)
        .add_job(nix_mac_arm.name, nix_mac_arm.job)
        .add_job(publish_nightly.name, publish_nightly.job)
}

fn check_style() -> NamedJob {
    let job = release_job(&[])
        .runs_on(runners::MAC_DEFAULT)
        .add_step(steps::checkout_repo().with_full_history())
        .add_step(steps::cargo_fmt())
        .add_step(steps::script("./script/clippy"));

    named::job(job)
}

fn release_job(deps: &[&NamedJob]) -> Job {
    let job = Job::default()
        .with_repository_owner_guard()
        .timeout_minutes(60u32);
    if deps.len() > 0 {
        job.needs(deps.iter().map(|j| j.name.clone()).collect::<Vec<_>>())
    } else {
        job
    }
}

// The Esperanta fork publishes nightly artifacts to a GitHub Releases
// prerelease tagged `nightly`, replacing upstream's DigitalOcean Spaces
// `script/upload-nightly` flow. The tag is force-moved to the current commit,
// then the prerelease is recreated and re-uploaded so asset names stay stable
// run-to-run. Authentication uses `secrets.SYNC_PAT` (the same PAT used by
// `sync_upstream.yml`) — the default `GITHUB_TOKEN` from a scheduled workflow
// is not guaranteed to have `contents: write`.
fn publish_nightly_release_job(bundle: &ReleaseBundleJobs) -> NamedJob {
    fn publish_nightly_release(token: &vars::StepOutput) -> Step<Run> {
        named::bash(indoc::indoc! {r#"
            NIGHTLY_REV=$(git rev-parse nightly 2>/dev/null || echo "")
            HEAD_REV=$(git rev-parse HEAD)
            if [ "$NIGHTLY_REV" = "$HEAD_REV" ]; then
              echo "Nightly tag already points to current commit. Skipping republish."
              exit 0
            fi
            git config user.name github-actions
            git config user.email github-actions@github.com
            git tag -f nightly
            git push origin nightly --force

            # Recreate the prerelease so it points at the new tag and the
            # asset list reflects exactly the current run. `gh release delete`
            # without `--cleanup-tag` leaves the git tag we just pushed alone.
            gh release delete nightly \
                --yes \
                --repo "$GITHUB_REPOSITORY" \
                2>/dev/null || true
            gh release create nightly \
                --prerelease \
                --title "Nightly" \
                --notes "Nightly build from commit \`$HEAD_REV\`" \
                --repo "$GITHUB_REPOSITORY" \
                release-artifacts/*
        "#})
        .add_env(("GITHUB_TOKEN", token.to_string()))
    }

    let (authenticate_step, token) = steps::authenticate_as_zippy().into();
    let publish_step = publish_nightly_release(&token);

    NamedJob {
        name: "publish_nightly_release".to_owned(),
        job: steps::release_job(&bundle.jobs())
            .runs_on(runners::LINUX_MEDIUM)
            .add_step(authenticate_step)
            .add_step(steps::checkout_repo().with_full_history().with_token(&token))
            .add_step(download_workflow_artifacts())
            .add_step(steps::script("ls -lR ./artifacts"))
            .add_step(prep_release_artifacts())
            .add_step(publish_step),
    }
}
