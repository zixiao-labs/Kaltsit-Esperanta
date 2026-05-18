use gh_workflow::{Event, Expression, Level, Push, Run, Step, Use, Workflow, ctx::Context};
use indoc::formatdoc;

use crate::tasks::workflows::{
    run_bundling::{bundle_linux, bundle_mac, bundle_windows},
    run_tests,
    runners::{self, Arch, Platform},
<<<<<<< HEAD
    steps::{self, NamedJob, TokenPermissions, dependant_job, named, release_job},
    vars::{self, StepOutput, assets},
=======
    steps::{
        self, DownloadArtifactStep, FluentBuilder, NamedJob, TokenPermissions, dependant_job,
        named, release_job,
    },
    vars::{self, JobOutput, StepOutput, assets},
>>>>>>> upstream/main
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

<<<<<<< HEAD
=======
pub(crate) fn create_sentry_release() -> Step<Use> {
    named::uses(
        "getsentry",
        "action-release",
        "526942b68292201ac6bbb99b9a0747d4abee354c", // v3
    )
    .add_env(("SENTRY_ORG", "zed-dev"))
    .add_env(("SENTRY_PROJECT", "zed"))
    .add_env(("SENTRY_AUTH_TOKEN", vars::SENTRY_AUTH_TOKEN))
    .add_with(("environment", "production"))
}

pub(crate) const COMPLIANCE_REPORT_PATH: &str = "compliance-report-${GITHUB_REF_NAME}.md";
pub(crate) const COMPLIANCE_REPORT_ARTIFACT_PATH: &str =
    "compliance-report-${{ github.ref_name }}.md";
pub(crate) const COMPLIANCE_STEP_ID: &str = "run-compliance-check";
const NEEDS_REVIEW_PULLS_URL: &str = "https://github.com/zed-industries/zed/pulls?q=is%3Apr+is%3Aclosed+label%3A%22PR+state%3Aneeds+review%22";

pub(crate) enum ComplianceContext {
    Release { non_blocking_outcome: JobOutput },
    ReleaseNonBlocking,
    Scheduled { tag_source: StepOutput },
}

impl ComplianceContext {
    fn tag_source(&self) -> Option<&StepOutput> {
        match self {
            ComplianceContext::Scheduled { tag_source } => Some(tag_source),
            _ => None,
        }
    }
}

pub(crate) fn add_compliance_steps(
    job: gh_workflow::Job,
    context: ComplianceContext,
) -> (gh_workflow::Job, StepOutput) {
    fn run_compliance_check(context: &ComplianceContext) -> (Step<Run>, StepOutput) {
        let job = named::bash(
            formatdoc! {r#"
                cargo xtask compliance version {target} --report-path "{COMPLIANCE_REPORT_PATH}"
                "#,
                target = if context.tag_source().is_some() { r#""$LATEST_TAG" --branch main"# } else { r#""$GITHUB_REF_NAME""# },
            }
        )
        .id(COMPLIANCE_STEP_ID)
        .add_env(("GITHUB_APP_ID", vars::ZED_ZIPPY_APP_ID))
        .add_env(("GITHUB_APP_KEY", vars::ZED_ZIPPY_APP_PRIVATE_KEY))
        .when_some(context.tag_source(), |step, tag_source| {
            step.add_env(("LATEST_TAG", tag_source.to_string()))
        })
        .when(
            matches!(
                context,
                ComplianceContext::Scheduled { .. } | ComplianceContext::ReleaseNonBlocking
            ),
            |step| step.continue_on_error(true),
        );

        let result = StepOutput::new_unchecked(&job, "outcome");
        (job, result)
    }

    let upload_step = upload_artifact(COMPLIANCE_REPORT_ARTIFACT_PATH)
        .if_condition(Expression::new("always()"))
        .when(
            matches!(context, ComplianceContext::Release { .. }),
            |step| step.overwrite(true),
        );

    let (success_prefix, failure_prefix) = match context {
        ComplianceContext::Release { .. } => {
            ("✅ Compliance check passed", "❌ Compliance check failed")
        }
        ComplianceContext::ReleaseNonBlocking => (
            "✅ Compliance check passed",
            "❌ Preliminary compliance check failed (but this can still be fixed while the builds are running!)",
        ),
        ComplianceContext::Scheduled { .. } => (
            "✅ Scheduled compliance check passed",
            "⚠️ Scheduled compliance check failed",
        ),
    };

    let script = formatdoc! {r#"
        if [ "$COMPLIANCE_OUTCOME" == "success" ]; then
            STATUS="{success_prefix} for $COMPLIANCE_TAG"
            MESSAGE=$(printf "%s\n\nReport: %s" "$STATUS" "$ARTIFACT_URL")
        else
            STATUS="{failure_prefix} for $COMPLIANCE_TAG"
            MESSAGE=$(printf "%s\n\nReport: %s\nPRs needing review: %s" "$STATUS" "$ARTIFACT_URL" "{NEEDS_REVIEW_PULLS_URL}")
        fi

        curl -X POST -H 'Content-type: application/json' \
            --data "$(jq -n --arg text "$MESSAGE" '{{"text": $text}}')" \
            "$SLACK_WEBHOOK"
        "#,
    };

    let notification_step = Step::new("send_compliance_slack_notification")
        .run(&script)
        .if_condition(match &context {
            ComplianceContext::Release {
                non_blocking_outcome,
            } => Expression::new(format!(
                "${{{{ failure() || {prior_outcome} != 'success' }}}}",
                prior_outcome = non_blocking_outcome.expr()
            )),
            ComplianceContext::Scheduled { .. } | ComplianceContext::ReleaseNonBlocking => {
                Expression::new("${{ always() }}")
            }
        })
        .add_env(("SLACK_WEBHOOK", vars::SLACK_WEBHOOK_WORKFLOW_FAILURES))
        .add_env((
            "COMPLIANCE_OUTCOME",
            format!("${{{{ steps.{COMPLIANCE_STEP_ID}.outcome }}}}"),
        ))
        .add_env((
            "COMPLIANCE_TAG",
            match &context {
                ComplianceContext::Release { .. } | ComplianceContext::ReleaseNonBlocking => {
                    Context::github().ref_name().to_string()
                }
                ComplianceContext::Scheduled { tag_source } => tag_source.to_string(),
            },
        ))
        .add_env((
            "ARTIFACT_URL",
            format!("{CURRENT_ACTION_RUN_URL}#artifacts"),
        ));

    let (compliance_step, check_result) = run_compliance_check(&context);

    (
        job.add_step(compliance_step)
            .add_step(upload_step)
            .add_step(notification_step)
            .when(
                matches!(context, ComplianceContext::ReleaseNonBlocking),
                |step| step.outputs([("outcome".to_string(), check_result.to_string())]),
            ),
        check_result,
    )
}

fn compliance_check() -> (NamedJob, JobOutput) {
    let job = release_job(&[])
        .runs_on(runners::LINUX_SMALL)
        .add_step(
            steps::checkout_repo()
                .with_full_history()
                .with_ref(Context::github().ref_()),
        )
        .add_step(steps::cache_rust_dependencies_namespace());

    let (compliance_job, check_result) =
        add_compliance_steps(job, ComplianceContext::ReleaseNonBlocking);
    let compliance_job = named::job(compliance_job);
    let check_result = check_result.as_job_output(&compliance_job);

    (compliance_job, check_result)
}

>>>>>>> upstream/main
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

pub(crate) fn download_workflow_artifacts() -> DownloadArtifactStep {
    steps::download_artifact().path("./artifacts/")
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
