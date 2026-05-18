use gh_workflow::{Event, Expression, Level, Push, Run, Step, Use, Workflow, ctx::Context};
use indoc::formatdoc;

use crate::tasks::workflows::{
    run_bundling::{bundle_linux, bundle_mac, bundle_windows},
    run_tests,
    runners::{self, Arch, Platform},
    steps::{self, NamedJob, TokenPermissions, dependant_job, named, release_job},
    vars::{self, StepOutput, assets},
};

pub(crate) fn release() -> Workflow {
    let macos_tests = run_tests::run_platform_tests_no_filter(Platform::Mac);
    let linux_tests = run_tests::run_platform_tests_no_filter(Platform::Linux);
    let windows_tests = run_tests::run_platform_tests_no_filter(Platform::Windows);
    let macos_clippy = run_tests::clippy(Platform::Mac, None);
    let linux_clippy = run_tests::clippy(Platform::Linux, None);
    let windows_clippy = run_tests::clippy(Platform::Windows, None);
    let check_scripts = run_tests::check_scripts();

    let create_draft_release = create_draft_release();

    let bundle = ReleaseBundleJobs {
        linux_aarch64: bundle_linux(
            Arch::AARCH64,
            None,
            &[&linux_tests, &linux_clippy, &check_scripts],
        ),
        linux_x86_64: bundle_linux(
            Arch::X86_64,
            None,
            &[&linux_tests, &linux_clippy, &check_scripts],
        ),
        mac_aarch64: bundle_mac(
            Arch::AARCH64,
            None,
            &[&macos_tests, &macos_clippy, &check_scripts],
        ),
        mac_x86_64: bundle_mac(
            Arch::X86_64,
            None,
            &[&macos_tests, &macos_clippy, &check_scripts],
        ),
        windows_aarch64: bundle_windows(
            Arch::AARCH64,
            None,
            &[&windows_tests, &windows_clippy, &check_scripts],
        ),
        windows_x86_64: bundle_windows(
            Arch::X86_64,
            None,
            &[&windows_tests, &windows_clippy, &check_scripts],
        ),
    };

    let upload_release_assets = upload_release_assets(&[&create_draft_release], &bundle);
    let validate_release_assets = validate_release_assets(&[&upload_release_assets]);
    let auto_release_preview = auto_release_preview(&[&validate_release_assets]);

    named::workflow()
        .on(Event::default().push(Push::default().tags(vec!["v*".to_string()])))
        .concurrency(vars::one_workflow_per_non_main_branch())
        .add_env(("CARGO_TERM_COLOR", "always"))
        .add_env(("RUST_BACKTRACE", "1"))
        .add_job(macos_tests.name, macos_tests.job)
        .add_job(linux_tests.name, linux_tests.job)
        .add_job(windows_tests.name, windows_tests.job)
        .add_job(macos_clippy.name, macos_clippy.job)
        .add_job(linux_clippy.name, linux_clippy.job)
        .add_job(windows_clippy.name, windows_clippy.job)
        .add_job(check_scripts.name, check_scripts.job)
        .add_job(create_draft_release.name, create_draft_release.job)
        .add_job(bundle.linux_aarch64.name, bundle.linux_aarch64.job)
        .add_job(bundle.linux_x86_64.name, bundle.linux_x86_64.job)
        .add_job(bundle.mac_aarch64.name, bundle.mac_aarch64.job)
        .add_job(bundle.mac_x86_64.name, bundle.mac_x86_64.job)
        .add_job(bundle.windows_aarch64.name, bundle.windows_aarch64.job)
        .add_job(bundle.windows_x86_64.name, bundle.windows_x86_64.job)
        .add_job(upload_release_assets.name, upload_release_assets.job)
        .add_job(validate_release_assets.name, validate_release_assets.job)
        .add_job(auto_release_preview.name, auto_release_preview.job)
}

pub(crate) struct ReleaseBundleJobs {
    pub linux_aarch64: NamedJob,
    pub linux_x86_64: NamedJob,
    pub mac_aarch64: NamedJob,
    pub mac_x86_64: NamedJob,
    pub windows_aarch64: NamedJob,
    pub windows_x86_64: NamedJob,
}

impl ReleaseBundleJobs {
    pub fn jobs(&self) -> Vec<&NamedJob> {
        vec![
            &self.linux_aarch64,
            &self.linux_x86_64,
            &self.mac_aarch64,
            &self.mac_x86_64,
            &self.windows_aarch64,
            &self.windows_x86_64,
        ]
    }

    #[allow(
        dead_code,
        reason = "kept for parity with upstream's release workflow which iterates ReleaseBundleJobs by-value"
    )]
    pub fn into_jobs(self) -> Vec<NamedJob> {
        vec![
            self.linux_aarch64,
            self.linux_x86_64,
            self.mac_aarch64,
            self.mac_x86_64,
            self.windows_aarch64,
            self.windows_x86_64,
        ]
    }
}

fn validate_release_assets(deps: &[&NamedJob]) -> NamedJob {
    let expected_assets: Vec<String> = assets::all().iter().map(|a| format!("\"{a}\"")).collect();
    let expected_assets_json = format!("[{}]", expected_assets.join(", "));

    let validation_script = formatdoc! {r#"
        EXPECTED_ASSETS='{expected_assets_json}'
        TAG="$GITHUB_REF_NAME"

        ACTUAL_ASSETS=$(gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json assets -q '[.assets[].name]')

        MISSING_ASSETS=$(echo "$EXPECTED_ASSETS" | jq -r --argjson actual "$ACTUAL_ASSETS" '. - $actual | .[]')

        if [ -n "$MISSING_ASSETS" ]; then
            echo "Error: The following assets are missing from the release:"
            echo "$MISSING_ASSETS"
            exit 1
        fi

        echo "All expected assets are present in the release."
        "#,
    };

    named::job(
        dependant_job(deps).runs_on(runners::LINUX_SMALL).add_step(
            named::bash(&validation_script).add_env(("GITHUB_TOKEN", vars::GITHUB_TOKEN)),
        ),
    )
}

fn auto_release_preview(deps: &[&NamedJob]) -> NamedJob {
    fn auto_release_preview_step(token: &StepOutput) -> Step<Run> {
        named::bash(indoc::indoc! {r#"
            tag="$GITHUB_REF_NAME"

            if [[ ! "$tag" =~ ^v([0-9]+)\.([0-9]+)\.([0-9]+)-pre$ ]]; then
                echo "::error::expected preview release tag in the form vMAJOR.MINOR.PATCH-pre, got $tag"
                exit 1
            fi

            major="${BASH_REMATCH[1]}"
            minor="${BASH_REMATCH[2]}"
            should_release=true

            released_preview="$(script/get-released-version preview)"
            if [[ -z "$released_preview" || "$released_preview" == "null" ]]; then
                echo "::error::could not determine released preview version"
                exit 1
            fi

            released_preview_major="$(echo "$released_preview" | cut -d. -f1)"
            released_preview_minor="$(echo "$released_preview" | cut -d. -f2)"

            if [[ "$released_preview_major" != "$major" || "$released_preview_minor" != "$minor" ]]; then
                should_release=false
                echo "Leaving $tag as a draft because it is the first preview release for v${major}.${minor}.x"
            fi

            if [[ "$should_release" == "true" ]]; then
                gh release edit "$tag" --repo "$GITHUB_REPOSITORY" --draft=false
            fi
        "#})
        .id("auto-release-preview")
        .add_env(("GITHUB_TOKEN", token))
    }

    let (authenticate, token) = steps::authenticate_as_zippy().into();
    let auto_release_step = auto_release_preview_step(&token);

    named::job(
        dependant_job(deps)
            .runs_on(runners::LINUX_SMALL)
            .cond(Expression::new(indoc::indoc!(
                r#"startsWith(github.ref, 'refs/tags/v') && endsWith(github.ref, '-pre')"#
            )))
            .add_step(authenticate)
            .add_step(
                steps::checkout_repo()
                    .with_token(&token)
                    .with_ref(Context::github().ref_()),
            )
            .add_step(auto_release_step),
    )
}

pub(crate) fn download_workflow_artifacts() -> Step<Use> {
    named::uses(
        "actions",
        "download-artifact",
        "018cc2cf5baa6db3ef3c5f8a56943fffe632ef53", // v6.0.0
    )
    .add_with(("path", "./artifacts/"))
}

pub(crate) fn prep_release_artifacts() -> Step<Run> {
    let mut script_lines = vec!["mkdir -p release-artifacts/\n".to_string()];
    for asset in assets::all() {
        let mv_command = format!("mv ./artifacts/{asset}/{asset} release-artifacts/{asset}");
        script_lines.push(mv_command)
    }

    named::bash(&script_lines.join("\n"))
}

fn upload_release_assets(deps: &[&NamedJob], bundle: &ReleaseBundleJobs) -> NamedJob {
    let mut deps = deps.to_vec();
    deps.extend(bundle.jobs());

    named::job(
        dependant_job(&deps)
            .runs_on(runners::LINUX_MEDIUM)
            .add_step(download_workflow_artifacts())
            .add_step(steps::script("ls -lR ./artifacts"))
            .add_step(prep_release_artifacts())
            .add_step(
                steps::script(
                    "gh release upload \"$GITHUB_REF_NAME\" --repo \"$GITHUB_REPOSITORY\" release-artifacts/*",
                )
                .add_env(("GITHUB_TOKEN", vars::GITHUB_TOKEN)),
            ),
    )
}

fn create_draft_release() -> NamedJob {
    fn generate_release_notes() -> Step<Run> {
        named::bash(
            r#"node --redirect-warnings=/dev/null ./script/draft-release-notes "$RELEASE_VERSION" "$RELEASE_CHANNEL" > target/release-notes.md"#,
        )
    }

    fn create_release(token: StepOutput) -> Step<Run> {
        named::bash("script/create-draft-release target/release-notes.md")
            .add_env(("GITHUB_TOKEN", token.to_string()))
    }

    let (authenticate_step, token) = steps::authenticate_as_zippy()
        .with_permissions([(TokenPermissions::Contents, Level::Write)])
        .into();

    named::job(
        release_job(&[])
            .runs_on(runners::LINUX_SMALL)
            // We need to fetch more than one commit so that `script/draft-release-notes`
            // is able to diff between the current and previous tag.
            //
            // 25 was chosen arbitrarily.
            .add_step(authenticate_step)
            .add_step(
                steps::checkout_repo()
                    .with_custom_fetch_depth(25)
                    .with_ref(Context::github().ref_()),
            )
            .add_step(steps::script("script/determine-release-channel"))
            .add_step(steps::script("mkdir -p target/"))
            .add_step(generate_release_notes())
            .add_step(create_release(token)),
    )
}
