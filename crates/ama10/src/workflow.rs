//! Wuling workflow YAML (`.wuling/workflows/*`) — GitHub Actions subset + `resource:`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;

pub const WORKFLOW_DIR: &str = ".wuling/workflows";
pub const MAX_MATRIX_JOBS: usize = 256;
pub const MAX_RUN_JOBS: usize = 1024;

const SUPPORTED_USES: &[&str] = &[
    "actions/checkout",
    "actions/upload-artifact",
    "actions/cache",
    "actions/setup-node",
    "actions/setup-rust",
    "dtolnay/rust-toolchain",
    "actions-rust-lang/setup-rust-toolchain",
    "pnpm/action-setup",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "on")]
    pub on: Triggers,
    pub jobs: BTreeMap<String, Job>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Triggers {
    pub push: Option<RefFilter>,
    pub pull_request: Option<RefFilter>,
    pub workflow_dispatch: bool,
}

impl Triggers {
    pub fn any(&self) -> bool {
        self.push.is_some() || self.pull_request.is_some() || self.workflow_dispatch
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RefFilter {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(rename = "runs-on", default)]
    pub runs_on: StringList,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<Container>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: StringList,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<Strategy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub run: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uses: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub with: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "String::is_empty", rename = "if")]
    pub if_expr: String,
    #[serde(
        default,
        skip_serializing_if = "is_zero_i32",
        rename = "timeout-minutes"
    )]
    pub timeout_minutes: i32,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strategy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<Matrix>,
    #[serde(default, rename = "fail-fast", skip_serializing_if = "Option::is_none")]
    pub fail_fast: Option<bool>,
    #[serde(default, rename = "max-parallel", skip_serializing_if = "is_zero_i32")]
    pub max_parallel: i32,
}

/// Simplified matrix: axes map to scalar string values; include/exclude are maps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Matrix {
    #[serde(flatten)]
    pub axes: BTreeMap<String, Vec<JsonValue>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<BTreeMap<String, JsonValue>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<BTreeMap<String, JsonValue>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StringList(pub Vec<String>);

impl std::ops::Deref for StringList {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for StringList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<String>> for StringList {
    fn from(value: Vec<String>) -> Self {
        Self(value)
    }
}

impl<'de> Deserialize<'de> for StringList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        match value {
            JsonValue::String(s) => Ok(Self(vec![s])),
            JsonValue::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        JsonValue::String(s) => out.push(s),
                        other => {
                            return Err(serde::de::Error::custom(format!(
                                "expected string list entry, got {other}"
                            )));
                        }
                    }
                }
                Ok(Self(out))
            }
            JsonValue::Null => Ok(Self(Vec::new())),
            other => Err(serde::de::Error::custom(format!(
                "expected string or string list, got {other}"
            ))),
        }
    }
}

impl Serialize for StringList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.0.len() == 1 {
            serializer.serialize_str(&self.0[0])
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Container {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        match value {
            JsonValue::String(image) => Ok(Self { image }),
            JsonValue::Object(map) => {
                let image = map
                    .get("image")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| serde::de::Error::custom("container.image is required"))?
                    .to_string();
                Ok(Self { image })
            }
            other => Err(serde::de::Error::custom(format!(
                "expected container image string or mapping, got {other}"
            ))),
        }
    }
}

impl Serialize for Container {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.image)
    }
}

impl<'de> Deserialize<'de> for Triggers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let mut triggers = Triggers::default();
        match value {
            JsonValue::String(name) => {
                set_trigger(&mut triggers, &name, None);
            }
            JsonValue::Array(items) => {
                for item in items {
                    let name = item
                        .as_str()
                        .ok_or_else(|| serde::de::Error::custom("trigger name must be a string"))?;
                    set_trigger(&mut triggers, name, None);
                }
            }
            JsonValue::Object(map) => {
                for (key, val) in map {
                    let filter = if val.is_null() {
                        Some(RefFilter::default())
                    } else {
                        Some(
                            RefFilter::deserialize(val).map_err(|err| {
                                serde::de::Error::custom(format!("invalid trigger filter: {err}"))
                            })?,
                        )
                    };
                    set_trigger(&mut triggers, &key, filter);
                }
            }
            other => {
                return Err(serde::de::Error::custom(format!(
                    "invalid `on:` block: {other}"
                )));
            }
        }
        Ok(triggers)
    }
}

impl Serialize for Triggers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        if let Some(filter) = &self.push {
            map.serialize_entry("push", filter)?;
        }
        if let Some(filter) = &self.pull_request {
            map.serialize_entry("pull_request", filter)?;
        }
        if self.workflow_dispatch {
            map.serialize_entry("workflow_dispatch", &JsonValue::Null)?;
        }
        map.end()
    }
}

fn set_trigger(triggers: &mut Triggers, name: &str, filter: Option<RefFilter>) {
    match name.trim() {
        "push" => triggers.push = Some(filter.unwrap_or_default()),
        "pull_request" => triggers.pull_request = Some(filter.unwrap_or_default()),
        "workflow_dispatch" => triggers.workflow_dispatch = true,
        _ => {}
    }
}

pub fn valid_tier(value: &str) -> bool {
    matches!(value, "low" | "medium" | "high")
}

pub fn is_job_id(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn uses_action_name(uses: &str) -> &str {
    uses.split('@').next().unwrap_or(uses)
}

fn is_supported_uses(uses: &str) -> bool {
    let name = uses_action_name(uses);
    SUPPORTED_USES.contains(&name)
}

fn is_supported_if(expr: &str) -> bool {
    matches!(
        expr.trim(),
        "success()" | "failure()" | "always()" | "${{ success() }}" | "${{ failure() }}" | "${{ always() }}"
    )
}

fn has_expr(value: &str) -> bool {
    value.contains("${{")
}

impl Workflow {
    pub fn parse(yaml: &str) -> Result<Self> {
        let workflow: Self = serde_yaml::from_str(yaml).context("parse workflow")?;
        workflow.validate()?;
        Ok(workflow)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("serialize workflow")
    }

    pub fn validate(&self) -> Result<()> {
        if !self.on.any() {
            bail!(
                "workflow {:?} declares no supported triggers (push/pull_request/workflow_dispatch)",
                self.name
            );
        }
        if self.jobs.is_empty() {
            bail!("workflow {:?} has no jobs", self.name);
        }

        let mut total_jobs = 0usize;
        for (job_id, job) in &self.jobs {
            if !is_job_id(job_id) {
                bail!(
                    "invalid job id {job_id:?} (must match ^[A-Za-z_][A-Za-z0-9_-]*$)"
                );
            }
            if !job.resource.is_empty()
                && !has_expr(&job.resource)
                && !valid_tier(&job.resource)
            {
                bail!("job {job_id:?}: resource must be one of low|medium|high");
            }
            if job.steps.is_empty() {
                bail!("job {job_id:?} has no steps");
            }
            for (index, step) in job.steps.iter().enumerate() {
                let has_run = !step.run.is_empty();
                let has_uses = !step.uses.is_empty();
                if has_run == has_uses {
                    bail!(
                        "job {job_id:?} step {}: exactly one of `run` or `uses` is required",
                        index + 1
                    );
                }
                if has_uses && !is_supported_uses(&step.uses) {
                    bail!(
                        "job {job_id:?} step {}: unsupported action {:?}",
                        index + 1,
                        step.uses
                    );
                }
                if !step.if_expr.is_empty() && !is_supported_if(&step.if_expr) {
                    bail!(
                        "job {job_id:?} step {}: unsupported `if` {:?}",
                        index + 1,
                        step.if_expr
                    );
                }
                if step.timeout_minutes < 0 {
                    bail!(
                        "job {job_id:?} step {}: timeout-minutes cannot be negative",
                        index + 1
                    );
                }
            }
            for dep in job.needs.iter() {
                if !self.jobs.contains_key(dep) {
                    bail!("job {job_id:?} needs unknown job {dep:?}");
                }
            }
            total_jobs += job.matrix_combination_count()?;
        }
        if total_jobs > MAX_RUN_JOBS {
            bail!(
                "workflow {:?} expands to {total_jobs} jobs, which exceeds the cap of {MAX_RUN_JOBS}",
                self.name
            );
        }
        self.job_order()?;
        Ok(())
    }

    pub fn job_order(&self) -> Result<Vec<String>> {
        let mut indegree: BTreeMap<String, usize> =
            self.jobs.keys().cloned().map(|id| (id, 0)).collect();
        let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (job_id, job) in &self.jobs {
            for dep in job.needs.iter() {
                adjacency.entry(dep.clone()).or_default().push(job_id.clone());
                *indegree.get_mut(job_id).expect("job exists") += 1;
            }
        }
        let mut ready: VecDeque<String> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut sorted_ready: Vec<_> = ready.drain(..).collect();
        sorted_ready.sort();
        ready.extend(sorted_ready);

        let mut order = Vec::new();
        while let Some(job_id) = ready.pop_front() {
            order.push(job_id.clone());
            if let Some(dependents) = adjacency.get(&job_id) {
                let mut next = dependents.clone();
                next.sort();
                for dependent in next {
                    let degree = indegree.get_mut(&dependent).expect("job exists");
                    *degree -= 1;
                    if *degree == 0 {
                        ready.push_back(dependent);
                    }
                }
                let mut resorted: Vec<_> = ready.drain(..).collect();
                resorted.sort();
                ready.extend(resorted);
            }
        }
        if order.len() != self.jobs.len() {
            bail!("workflow {:?} has a cycle in job `needs`", self.name);
        }
        Ok(order)
    }

    pub fn default_ci_seed() -> Self {
        Self::parse(
            r#"
name: CI
on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
jobs:
  build:
    runs-on: [linux, docker]
    resource: medium
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: Build
        run: echo build
  test:
    needs: [build]
    runs-on: linux
    resource: low
    steps:
      - run: echo test
"#,
        )
        .expect("default CI seed must parse")
    }
}

impl Job {
    pub fn effective_tier(&self, default_tier: &str) -> String {
        if valid_tier(&self.resource) {
            return self.resource.clone();
        }
        for label in self.runs_on.iter() {
            if let Some(rest) = label.strip_prefix("tier:")
                && valid_tier(rest)
            {
                return rest.to_string();
            }
        }
        if valid_tier(default_tier) {
            default_tier.to_string()
        } else {
            "medium".into()
        }
    }

    pub fn matrix_combination_count(&self) -> Result<usize> {
        Ok(self.matrix_combinations()?.len().max(1))
    }

    pub fn matrix_combinations(&self) -> Result<Vec<BTreeMap<String, String>>> {
        let Some(strategy) = &self.strategy else {
            return Ok(vec![BTreeMap::new()]);
        };
        let Some(matrix) = &strategy.matrix else {
            return Ok(vec![BTreeMap::new()]);
        };

        let mut axis_names = Vec::new();
        let mut axis_values = Vec::new();
        for (name, values) in &matrix.axes {
            if name == "include" || name == "exclude" {
                continue;
            }
            if !is_job_id(name) {
                bail!("invalid matrix axis {name:?}");
            }
            if values.is_empty() {
                bail!("matrix axis {name:?} must not be empty");
            }
            axis_names.push(name.clone());
            axis_values.push(
                values
                    .iter()
                    .map(json_to_text)
                    .collect::<Result<Vec<_>>>()?,
            );
        }

        let mut combos = if axis_names.is_empty() {
            vec![BTreeMap::new()]
        } else {
            cartesian(&axis_names, &axis_values)
        };

        for exclude in &matrix.exclude {
            combos.retain(|combo| !entry_matches(combo, exclude));
        }
        for include in &matrix.include {
            let mut row = BTreeMap::new();
            for (key, value) in include {
                row.insert(key.clone(), json_to_text(value)?);
            }
            combos.push(row);
        }

        if combos.is_empty() {
            bail!("strategy.matrix expands to zero combinations");
        }
        if combos.len() > MAX_MATRIX_JOBS {
            bail!(
                "strategy.matrix expands to {} combinations, exceeding {MAX_MATRIX_JOBS}",
                combos.len()
            );
        }
        Ok(combos)
    }
}

fn json_to_text(value: &JsonValue) -> Result<String> {
    match value {
        JsonValue::String(s) => Ok(s.clone()),
        JsonValue::Number(n) => Ok(n.to_string()),
        JsonValue::Bool(b) => Ok(b.to_string()),
        JsonValue::Null => Ok(String::new()),
        other => bail!("unsupported matrix value {other}"),
    }
}

fn cartesian(names: &[String], values: &[Vec<String>]) -> Vec<BTreeMap<String, String>> {
    let mut out = vec![BTreeMap::new()];
    for (name, axis) in names.iter().zip(values.iter()) {
        let mut next = Vec::new();
        for existing in &out {
            for value in axis {
                let mut row = existing.clone();
                row.insert(name.clone(), value.clone());
                next.push(row);
            }
        }
        out = next;
    }
    out
}

fn entry_matches(combo: &BTreeMap<String, String>, entry: &BTreeMap<String, JsonValue>) -> bool {
    entry.iter().all(|(key, value)| {
        json_to_text(value)
            .ok()
            .and_then(|text| combo.get(key).map(|v| v == &text))
            .unwrap_or(false)
    })
}

pub fn unique_job_ids(workflow: &Workflow) -> BTreeSet<String> {
    workflow.jobs.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ci_fixture() {
        let yaml = r#"
name: CI
on:
  push:
    branches: [main, "release/*"]
  pull_request:
  workflow_dispatch:
jobs:
  build:
    runs-on: [linux, docker]
    resource: medium
    container: node:20
    env:
      CI: "true"
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: Build
        run: npm ci && npm run build
  test:
    needs: [build]
    runs-on: linux
    resource: low
    steps:
      - run: npm test
"#;
        let workflow = Workflow::parse(yaml).unwrap();
        assert_eq!(workflow.jobs.len(), 2);
        assert_eq!(
            workflow.job_order().unwrap(),
            vec!["build".to_string(), "test".to_string()]
        );
        let roundtrip = workflow.to_yaml().unwrap();
        let again = Workflow::parse(&roundtrip).unwrap();
        assert!(again.jobs.contains_key("test"));
    }

    #[test]
    fn rejects_cycle() {
        let yaml = r#"
name: Cycle
on: [workflow_dispatch]
jobs:
  a:
    needs: [b]
    runs-on: linux
    steps: [{ run: echo a }]
  b:
    needs: [a]
    runs-on: linux
    steps: [{ run: echo b }]
"#;
        assert!(Workflow::parse(yaml).is_err());
    }

    #[test]
    fn matrix_cartesian() {
        let yaml = r#"
name: Matrix
on: workflow_dispatch
jobs:
  build:
    runs-on: linux
    strategy:
      matrix:
        os: [linux, windows]
        ver: [1, 2]
    steps:
      - run: echo hi
"#;
        let workflow = Workflow::parse(yaml).unwrap();
        let combos = workflow.jobs["build"].matrix_combinations().unwrap();
        assert_eq!(combos.len(), 4);
    }
}
