//! Job DAG layout helpers for the Wuling workflow configurator.

use ama10::workflow::Workflow;
use ama10::workflow_simulate::SimulatedJobStatus;
use std::collections::BTreeMap;
use ui::prelude::*;

#[derive(Clone)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub needs: Vec<String>,
    pub layer: usize,
}

#[derive(Clone, Default)]
pub struct FlowLayout {
    pub nodes: Vec<FlowNode>,
    pub layers: Vec<Vec<String>>,
}

impl FlowLayout {
    pub fn from_workflow(workflow: &Workflow) -> anyhow::Result<Self> {
        let order = workflow.job_order()?;
        let mut layer_of: BTreeMap<String, usize> = BTreeMap::new();
        for job_id in &order {
            let job = workflow.jobs.get(job_id).expect("known job");
            let layer = job
                .needs
                .iter()
                .filter_map(|need| layer_of.get(need).copied())
                .max()
                .map(|layer| layer + 1)
                .unwrap_or(0);
            layer_of.insert(job_id.clone(), layer);
        }

        let max_layer = layer_of.values().copied().max().unwrap_or(0);
        let mut layers = vec![Vec::new(); max_layer + 1];
        let mut nodes = Vec::new();
        for job_id in &order {
            let job = workflow.jobs.get(job_id).expect("known job");
            let layer = layer_of[job_id];
            let label = if job.name.is_empty() {
                job_id.clone()
            } else {
                format!("{job_id} · {}", job.name)
            };
            layers[layer].push(job_id.clone());
            nodes.push(FlowNode {
                id: job_id.clone(),
                label,
                needs: job.needs.0.clone(),
                layer,
            });
        }
        for layer in &mut layers {
            layer.sort();
        }
        Ok(Self { nodes, layers })
    }

    pub fn node(&self, id: &str) -> Option<&FlowNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

pub fn status_label(status: Option<SimulatedJobStatus>) -> SharedString {
    SharedString::from(match status {
        Some(SimulatedJobStatus::Pending) => "pending",
        Some(SimulatedJobStatus::Ready) => "ready",
        Some(SimulatedJobStatus::Running) => "running",
        Some(SimulatedJobStatus::Succeeded) => "ok",
        Some(SimulatedJobStatus::Failed) => "failed",
        Some(SimulatedJobStatus::Blocked) => "blocked",
        None => "",
    })
}

pub fn status_color(status: Option<SimulatedJobStatus>) -> Color {
    match status {
        Some(SimulatedJobStatus::Ready) => Color::Warning,
        Some(SimulatedJobStatus::Running) => Color::Accent,
        Some(SimulatedJobStatus::Succeeded) => Color::Success,
        Some(SimulatedJobStatus::Failed) | Some(SimulatedJobStatus::Blocked) => Color::Error,
        _ => Color::Muted,
    }
}
