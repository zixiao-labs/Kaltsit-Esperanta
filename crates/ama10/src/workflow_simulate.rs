//! Local dry-run simulation for Wuling workflow job DAGs.

use std::collections::BTreeMap;

use anyhow::{Result, bail};

use crate::workflow::{Job, Workflow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatedJobStatus {
    Pending,
    Ready,
    Running,
    Succeeded,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedJob {
    pub key: String,
    pub job_id: String,
    pub display_name: String,
    pub needs: Vec<String>,
    pub status: SimulatedJobStatus,
}

#[derive(Debug, Clone)]
pub struct WorkflowSimulation {
    pub jobs: Vec<SimulatedJob>,
    pub order: Vec<String>,
    pub finished: bool,
}

impl WorkflowSimulation {
    pub fn from_workflow(workflow: &Workflow) -> Result<Self> {
        let mut jobs = Vec::new();
        for job_id in workflow.job_order()? {
            let job = workflow
                .jobs
                .get(&job_id)
                .expect("job_order only yields known jobs");
            let combos = job.matrix_combinations()?;
            if combos.len() <= 1 && combos.first().map(|c| c.is_empty()).unwrap_or(true) {
                jobs.push(simulated_job(
                    job_id.clone(),
                    job_id.clone(),
                    job,
                    BTreeMap::new(),
                    &job.needs,
                ));
                continue;
            }
            for combo in combos {
                let suffix = combo.values().cloned().collect::<Vec<_>>().join(", ");
                let key = format!("{job_id} ({suffix})");
                // Matrix legs inherit the template job's needs against template ids.
                jobs.push(simulated_job(key, job_id.clone(), job, combo, &job.needs));
            }
        }

        // Remap needs for non-matrix jobs; matrix legs still depend on template job ids.
        // Expand needs so a dependent waits on all legs of a needed job id.
        let template_to_keys: BTreeMap<String, Vec<String>> = {
            let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for job in &jobs {
                map.entry(job.job_id.clone())
                    .or_default()
                    .push(job.key.clone());
            }
            map
        };
        for job in &mut jobs {
            let mut expanded = Vec::new();
            for need in &job.needs {
                if let Some(keys) = template_to_keys.get(need) {
                    expanded.extend(keys.iter().cloned());
                } else {
                    expanded.push(need.clone());
                }
            }
            job.needs = expanded;
        }

        let order: Vec<String> = jobs.iter().map(|job| job.key.clone()).collect();
        let mut simulation = Self {
            jobs,
            order,
            finished: false,
        };
        simulation.refresh_ready();
        Ok(simulation)
    }

    pub fn status_map(&self) -> BTreeMap<String, SimulatedJobStatus> {
        self.jobs
            .iter()
            .map(|job| (job.key.clone(), job.status))
            .collect()
    }

    pub fn mark_failed(&mut self, key: &str) -> Result<()> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.key == key)
            .ok_or_else(|| anyhow::anyhow!("unknown simulated job {key}"))?;
        job.status = SimulatedJobStatus::Failed;
        self.refresh_ready();
        self.finished = self.is_finished();
        Ok(())
    }

    /// Advance one scheduling step: start all Ready jobs, then complete Running as Succeeded.
    pub fn step(&mut self) -> Result<Vec<String>> {
        if self.finished {
            return Ok(Vec::new());
        }

        let mut changed = Vec::new();
        for job in &mut self.jobs {
            if job.status == SimulatedJobStatus::Ready {
                job.status = SimulatedJobStatus::Running;
                changed.push(job.key.clone());
            }
        }
        if !changed.is_empty() {
            return Ok(changed);
        }

        for job in &mut self.jobs {
            if job.status == SimulatedJobStatus::Running {
                job.status = SimulatedJobStatus::Succeeded;
                changed.push(job.key.clone());
            }
        }
        self.refresh_ready();
        self.finished = self.is_finished();
        if changed.is_empty() && !self.finished {
            bail!("simulation stalled; check for blocked jobs");
        }
        Ok(changed)
    }

    pub fn play_all(&mut self) -> Result<()> {
        let mut guard = 0;
        while !self.finished {
            self.step()?;
            guard += 1;
            if guard > self.jobs.len() * 4 + 8 {
                bail!("simulation exceeded step budget");
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        for job in &mut self.jobs {
            job.status = SimulatedJobStatus::Pending;
        }
        self.finished = false;
        self.refresh_ready();
    }

    fn is_finished(&self) -> bool {
        self.jobs.iter().all(|job| {
            matches!(
                job.status,
                SimulatedJobStatus::Succeeded
                    | SimulatedJobStatus::Failed
                    | SimulatedJobStatus::Blocked
            )
        })
    }

    fn refresh_ready(&mut self) {
        let status: BTreeMap<String, SimulatedJobStatus> = self.status_map();
        for job in &mut self.jobs {
            if matches!(
                job.status,
                SimulatedJobStatus::Succeeded
                    | SimulatedJobStatus::Failed
                    | SimulatedJobStatus::Running
            ) {
                continue;
            }
            let mut blocked = false;
            let mut waiting = false;
            for need in &job.needs {
                match status
                    .get(need)
                    .copied()
                    .unwrap_or(SimulatedJobStatus::Pending)
                {
                    SimulatedJobStatus::Succeeded => {}
                    SimulatedJobStatus::Failed | SimulatedJobStatus::Blocked => blocked = true,
                    _ => waiting = true,
                }
            }
            job.status = if blocked {
                SimulatedJobStatus::Blocked
            } else if waiting {
                SimulatedJobStatus::Pending
            } else {
                SimulatedJobStatus::Ready
            };
        }
    }
}

fn simulated_job(
    key: String,
    job_id: String,
    job: &Job,
    combo: BTreeMap<String, String>,
    needs: &[String],
) -> SimulatedJob {
    let display_name = if job.name.is_empty() {
        key.clone()
    } else if combo.is_empty() {
        job.name.clone()
    } else {
        let suffix = combo.values().cloned().collect::<Vec<_>>().join(", ");
        format!("{} ({suffix})", job.name)
    };
    SimulatedJob {
        key,
        job_id,
        display_name,
        needs: needs.to_vec(),
        status: SimulatedJobStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::Workflow;

    #[test]
    fn simulates_linear_pipeline() {
        let workflow = Workflow::parse(
            r#"
name: CI
on: workflow_dispatch
jobs:
  build:
    runs-on: linux
    steps: [{ run: echo build }]
  test:
    needs: [build]
    runs-on: linux
    steps: [{ run: echo test }]
"#,
        )
        .unwrap();
        let mut sim = WorkflowSimulation::from_workflow(&workflow).unwrap();
        assert_eq!(sim.jobs[0].status, SimulatedJobStatus::Ready);
        assert_eq!(sim.jobs[1].status, SimulatedJobStatus::Pending);
        sim.step().unwrap(); // build running
        sim.step().unwrap(); // build succeeded, test ready
        assert_eq!(sim.jobs[0].status, SimulatedJobStatus::Succeeded);
        assert_eq!(sim.jobs[1].status, SimulatedJobStatus::Ready);
        sim.play_all().unwrap();
        assert!(sim.finished);
        assert!(
            sim.jobs
                .iter()
                .all(|job| job.status == SimulatedJobStatus::Succeeded)
        );
    }

    #[test]
    fn failure_blocks_dependents() {
        let workflow = Workflow::parse(
            r#"
name: CI
on: workflow_dispatch
jobs:
  build:
    runs-on: linux
    steps: [{ run: echo build }]
  test:
    needs: [build]
    runs-on: linux
    steps: [{ run: echo test }]
"#,
        )
        .unwrap();
        let mut sim = WorkflowSimulation::from_workflow(&workflow).unwrap();
        sim.step().unwrap();
        sim.mark_failed("build").unwrap();
        assert_eq!(sim.jobs[1].status, SimulatedJobStatus::Blocked);
    }
}
