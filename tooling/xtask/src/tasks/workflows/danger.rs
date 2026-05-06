use gh_workflow::*;

use crate::tasks::workflows::steps::{CommonJobConditions, NamedJob, named};

use super::{runners, steps, vars};

/// Generates the danger.yml workflow
pub fn danger() -> Workflow {
    let danger = danger_job();

    named::workflow()
        .on(
            Event::default().pull_request(PullRequest::default().add_branch("main").types([
                PullRequestType::Opened,
                PullRequestType::Synchronize,
                PullRequestType::Reopened,
                PullRequestType::Edited,
            ])),
        )
        .add_job(danger.name, danger.job)
}

fn danger_job() -> NamedJob {
    pub fn install_deps() -> Step<Run> {
        named::bash("pnpm install --dir script/danger")
    }

    pub fn run() -> Step<Run> {
        // Upstream Zed proxies Danger through `danger-proxy.zed.dev` so it can
        // authenticate to GitHub without exposing repo-scoped secrets to PRs
        // from forks. The Esperanta fork doesn't have that proxy, so we hand
        // Danger the workflow's `GITHUB_TOKEN` directly. For PRs from forks
        // this token is automatically downgraded to read-only by GitHub —
        // Danger can read but not comment in that case, which is the expected
        // trade-off for a small fork without dedicated bot infrastructure.
        named::bash("pnpm run --dir script/danger danger ci")
            .add_env(("GITHUB_TOKEN", vars::GITHUB_TOKEN))
    }

    NamedJob {
        name: "danger".to_string(),
        job: Job::default()
            .with_repository_owner_guard()
            .runs_on(runners::LINUX_SMALL)
            .add_step(steps::checkout_repo())
            .add_step(steps::setup_pnpm())
            .add_step(
                steps::setup_node()
                    .add_with(("cache", "pnpm"))
                    .add_with(("cache-dependency-path", "script/danger/pnpm-lock.yaml")),
            )
            .add_step(install_deps())
            .add_step(run()),
    }
}
