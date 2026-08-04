//! Workspace Item: Wuling workflow flowchart, YAML editor, and local simulate.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ama10::workflow::{WORKFLOW_DIR, Workflow};
use ama10::workflow_simulate::WorkflowSimulation;
use ama10_i18n::{tr, tr_f};
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, Subscription,
    WeakEntity, Window, actions, prelude::*,
};
use language::{Buffer, BufferEvent};
use project::Project;
use ui::{ToggleButtonGroup, ToggleButtonSimple, prelude::*};
use workspace::{
    Workspace,
    item::{Item, ItemEvent},
};

use crate::job_flow_canvas::{FlowLayout, status_color, status_label};

actions!(
    ama10,
    [
        /// Open the Wuling workflow configurator in an editor tab.
        OpenWulingWorkflow,
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Flow,
    Yaml,
    Simulate,
}

pub struct WorkflowView {
    #[allow(dead_code)]
    workspace: WeakEntity<Workspace>,
    project: Entity<Project>,
    focus_handle: FocusHandle,
    mode: ViewMode,
    workflow_path: Option<PathBuf>,
    disk_mtime: Option<SystemTime>,
    workflow: Workflow,
    layout: FlowLayout,
    selected_job: Option<SharedString>,
    yaml_editor: Entity<Editor>,
    syncing_yaml: bool,
    simulation: Option<WorkflowSimulation>,
    status: SharedString,
    error: Option<SharedString>,
    dirty: bool,
    available_files: Vec<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &OpenWulingWorkflow, window, cx| {
            open_or_reuse_workflow(workspace, None, window, cx);
        });
    })
    .detach();
}

pub fn open_or_reuse_workflow(
    workspace: &mut Workspace,
    path: Option<PathBuf>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if let Some(existing) = workspace.item_of_type::<WorkflowView>(cx) {
        if let Some(path) = path {
            existing.update(cx, |view, cx| {
                view.load_path(path, window, cx);
            });
        }
        workspace.activate_item(&existing, true, true, window, cx);
        return;
    }
    let project = workspace.project().clone();
    let view = cx.new(|cx| WorkflowView::new(workspace.weak_handle(), project, path, window, cx));
    workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
}

impl WorkflowView {
    fn new(
        workspace: WeakEntity<Workspace>,
        project: Entity<Project>,
        path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let workflow = Workflow::default_ci_seed();
        let yaml = workflow.to_yaml().unwrap_or_default();
        let layout = FlowLayout::from_workflow(&workflow).unwrap_or_default();
        let (yaml_editor, buffer) = create_yaml_editor(&yaml, window, cx);
        let subscription = cx.subscribe(&buffer, |this, _, event, cx| {
            if this.syncing_yaml {
                return;
            }
            if matches!(event, BufferEvent::Edited { .. }) {
                this.dirty = true;
                cx.notify();
            }
        });
        let mut this = Self {
            workspace,
            project,
            focus_handle: cx.focus_handle(),
            mode: ViewMode::Flow,
            workflow_path: None,
            disk_mtime: None,
            workflow,
            layout,
            selected_job: None,
            yaml_editor,
            syncing_yaml: false,
            simulation: None,
            status: tr!("Edit .wuling/workflows locally. Simulate does not trigger remote runs."),
            error: None,
            dirty: false,
            available_files: Vec::new(),
            _subscriptions: vec![subscription],
        };
        this.refresh_file_list(cx);
        if let Some(path) = path {
            this.load_path(path, window, cx);
        } else if let Some(first) = this.available_files.first().cloned() {
            this.load_path(first, window, cx);
        }
        this
    }

    fn refresh_file_list(&mut self, cx: &mut Context<Self>) {
        self.available_files.clear();
        let project = self.project.read(cx);
        for worktree in project.worktrees(cx) {
            let snapshot = worktree.read(cx).snapshot();
            let root = snapshot.abs_path().to_path_buf();
            let dir = root.join(WORKFLOW_DIR);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext == "yml" || ext == "yaml")
                    {
                        self.available_files.push(path);
                    }
                }
            }
        }
        self.available_files.sort();
    }

    fn load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&path) {
            Ok(text) => match Workflow::parse(&text) {
                Ok(workflow) => {
                    self.apply_workflow(workflow, Some(text), window, cx);
                    self.disk_mtime = file_mtime(&path);
                    self.workflow_path = Some(path);
                    self.dirty = false;
                    self.status = tr!("Loaded workflow file.");
                    self.error = None;
                }
                Err(error) => {
                    self.workflow_path = Some(path);
                    self.disk_mtime = None;
                    self.set_editor_text(&text, window, cx);
                    self.mode = ViewMode::Yaml;
                    self.error = Some(format!("{error:#}").into());
                }
            },
            Err(error) => {
                self.error = Some(format!("{error:#}").into());
            }
        }
        cx.notify();
    }

    fn apply_workflow(
        &mut self,
        workflow: Workflow,
        yaml: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.layout = FlowLayout::from_workflow(&workflow).unwrap_or_default();
        self.workflow = workflow;
        if let Some(yaml) = yaml {
            self.set_editor_text(&yaml, window, cx);
        } else if let Ok(yaml) = self.workflow.to_yaml() {
            self.set_editor_text(&yaml, window, cx);
        }
        self.simulation = None;
        if self
            .selected_job
            .as_ref()
            .is_some_and(|id| !self.workflow.jobs.contains_key(id.as_ref()))
        {
            self.selected_job = None;
        }
    }

    fn set_editor_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing_yaml = true;
        self.yaml_editor.update(cx, |editor, cx| {
            editor.set_text(text, window, cx);
        });
        self.syncing_yaml = false;
    }

    fn sync_from_active_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match self.mode {
            ViewMode::Yaml => {
                let text = self.yaml_editor.read(cx).text(cx);
                let workflow = Workflow::parse(&text)?;
                self.apply_workflow(workflow, None, window, cx);
            }
            ViewMode::Flow | ViewMode::Simulate => {
                let yaml = self.workflow.to_yaml()?;
                self.set_editor_text(&yaml, window, cx);
            }
        }
        Ok(())
    }

    fn set_mode(&mut self, mode: ViewMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        if let Err(error) = self.sync_from_active_mode(window, cx) {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        if mode == ViewMode::Simulate {
            match WorkflowSimulation::from_workflow(&self.workflow) {
                Ok(simulation) => self.simulation = Some(simulation),
                Err(error) => {
                    self.error = Some(format!("{error:#}").into());
                    cx.notify();
                    return;
                }
            }
        }
        self.mode = mode;
        self.error = None;
        cx.notify();
    }

    fn save_to_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.sync_from_active_mode(window, cx) {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        let yaml = match self.workflow.to_yaml() {
            Ok(yaml) => yaml,
            Err(error) => {
                self.error = Some(format!("{error:#}").into());
                cx.notify();
                return;
            }
        };
        let path = self.workflow_path.clone().unwrap_or_else(|| {
            let fallback = self
                .project
                .read(cx)
                .worktrees(cx)
                .next()
                .map(|tree| tree.read(cx).abs_path().join(WORKFLOW_DIR).join("ci.yml"))
                .unwrap_or_else(|| PathBuf::from(format!("{WORKFLOW_DIR}/ci.yml")));
            fallback
        });
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        if let (Some(baseline), Some(current)) = (self.disk_mtime, file_mtime(&path))
            && baseline != current
        {
            self.error = Some(tr!(
                "File changed on disk — reload before saving to avoid overwriting."
            ));
            cx.notify();
            return;
        }
        match std::fs::write(&path, yaml.as_bytes()) {
            Ok(()) => {
                self.workflow_path = Some(path.clone());
                self.disk_mtime = file_mtime(&path);
                self.dirty = false;
                self.refresh_file_list(cx);
                self.status = tr_f!("Saved {}", path.display());
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("{error:#}").into());
            }
        }
        cx.notify();
    }

    fn create_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workflow = Workflow::default_ci_seed();
        self.apply_workflow(workflow, None, window, cx);
        self.workflow_path = self
            .project
            .read(cx)
            .worktrees(cx)
            .next()
            .map(|tree| tree.read(cx).abs_path().join(WORKFLOW_DIR).join("ci.yml"));
        self.disk_mtime = None;
        self.dirty = true;
        self.mode = ViewMode::Flow;
        self.status = tr!("Created a CI workflow seed — Save to write .wuling/workflows/ci.yml.");
        cx.notify();
    }

    fn add_job(&mut self, cx: &mut Context<Self>) {
        let mut index = self.workflow.jobs.len() + 1;
        let mut id = format!("job_{index}");
        while self.workflow.jobs.contains_key(&id) {
            index += 1;
            id = format!("job_{index}");
        }
        self.workflow.jobs.insert(
            id.clone(),
            ama10::workflow::Job {
                name: String::new(),
                runs_on: vec!["linux".into()].into(),
                resource: "medium".into(),
                container: None,
                needs: Vec::new().into(),
                strategy: None,
                env: Default::default(),
                steps: vec![ama10::workflow::Step {
                    name: String::new(),
                    run: "echo hello".into(),
                    uses: String::new(),
                    with: Default::default(),
                    env: Default::default(),
                    if_expr: String::new(),
                    timeout_minutes: 0,
                }],
            },
        );
        self.layout = FlowLayout::from_workflow(&self.workflow).unwrap_or_default();
        self.selected_job = Some(id.into());
        self.dirty = true;
        cx.notify();
    }

    fn remove_selected_job(&mut self, cx: &mut Context<Self>) {
        let Some(job_id) = self.selected_job.clone() else {
            return;
        };
        self.workflow.jobs.remove(job_id.as_ref());
        for job in self.workflow.jobs.values_mut() {
            job.needs.0.retain(|need| need != job_id.as_ref());
        }
        self.selected_job = None;
        self.layout = FlowLayout::from_workflow(&self.workflow).unwrap_or_default();
        self.dirty = true;
        cx.notify();
    }

    fn toggle_need(&mut self, need: &str, cx: &mut Context<Self>) {
        let Some(job_id) = self.selected_job.clone() else {
            return;
        };
        if job_id.as_ref() == need {
            return;
        }
        let Some(job) = self.workflow.jobs.get_mut(job_id.as_ref()) else {
            return;
        };
        if let Some(index) = job.needs.iter().position(|existing| existing == need) {
            job.needs.remove(index);
        } else {
            job.needs.push(need.to_string());
        }
        match FlowLayout::from_workflow(&self.workflow) {
            Ok(layout) => {
                self.layout = layout;
                self.error = None;
                self.dirty = true;
            }
            Err(error) => {
                // revert
                let job = self.workflow.jobs.get_mut(job_id.as_ref()).unwrap();
                if let Some(index) = job.needs.iter().position(|existing| existing == need) {
                    job.needs.remove(index);
                } else {
                    job.needs.push(need.to_string());
                }
                self.error = Some(format!("{error:#}").into());
            }
        }
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let file_label = self
            .workflow_path
            .as_ref()
            .map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("workflow")
                    .to_string()
            })
            .unwrap_or_else(|| tr!("No file").to_string());

        h_flex()
            .w_full()
            .gap_2()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(Label::new(file_label).size(LabelSize::Small))
            .children(self.available_files.iter().take(6).map(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("workflow")
                    .to_string();
                let path = path.clone();
                Button::new(SharedString::from(format!("wf-{}", path.display())), name)
                    .style(ButtonStyle::Subtle)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.load_path(path.clone(), window, cx);
                    }))
            }))
            .child(
                Button::new("new-workflow", tr!("New"))
                    .on_click(cx.listener(|this, _, window, cx| this.create_new(window, cx))),
            )
            .child(
                ToggleButtonGroup::single_row(
                    "workflow-mode",
                    [
                        ToggleButtonSimple::new(
                            tr!("Flow"),
                            cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewMode::Flow, window, cx);
                            }),
                        ),
                        ToggleButtonSimple::new(
                            tr!("YAML"),
                            cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewMode::Yaml, window, cx);
                            }),
                        ),
                        ToggleButtonSimple::new(
                            tr!("Simulate"),
                            cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewMode::Simulate, window, cx);
                            }),
                        ),
                    ],
                )
                .selected_index(match self.mode {
                    ViewMode::Flow => 0,
                    ViewMode::Yaml => 1,
                    ViewMode::Simulate => 2,
                }),
            )
            .child(
                Button::new("save-workflow", tr!("Save"))
                    .style(ButtonStyle::Filled)
                    .on_click(cx.listener(|this, _, window, cx| this.save_to_disk(window, cx))),
            )
            .when(self.dirty, |el| {
                el.child(
                    Label::new(tr!("Unsaved"))
                        .color(Color::Warning)
                        .size(LabelSize::Small),
                )
            })
    }

    fn render_flow(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .size_full()
            .child(
                h_flex().flex_1().gap_4().p_3().children(
                    self.layout
                        .layers
                        .iter()
                        .enumerate()
                        .map(|(layer_ix, ids)| {
                            v_flex()
                                .gap_2()
                                .min_w(rems(14.))
                                .child(
                                    Label::new(tr_f!("stage {}", layer_ix))
                                        .size(LabelSize::XSmall)
                                        .color(Color::Muted),
                                )
                                .children(ids.iter().map(|id| {
                                    let node = self.layout.node(id);
                                    let label = node
                                        .map(|node| node.label.clone())
                                        .unwrap_or_else(|| id.clone());
                                    let needs = node
                                        .map(|node| {
                                            if node.needs.is_empty() {
                                                tr!("needs: —").to_string()
                                            } else {
                                                tr_f!("needs: {}", node.needs.join(", "))
                                                    .to_string()
                                            }
                                        })
                                        .unwrap_or_default();
                                    let is_selected =
                                        self.selected_job.as_ref().map(|s| s.as_ref())
                                            == Some(id.as_str());
                                    let status = self
                                        .simulation
                                        .as_ref()
                                        .and_then(|sim| sim.status_for_job_id(id));
                                    let job_id = id.clone();
                                    v_flex()
                                        .gap_1()
                                        .p_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(cx.theme().colors().border)
                                        .child(
                                            Button::new(
                                                SharedString::from(format!("select-job-{id}")),
                                                label,
                                            )
                                            .style(if is_selected {
                                                ButtonStyle::Filled
                                            } else {
                                                ButtonStyle::Subtle
                                            })
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.selected_job = Some(job_id.clone().into());
                                                    cx.notify();
                                                }),
                                            ),
                                        )
                                        .child(
                                            Label::new(needs)
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        )
                                        .when(status.is_some(), |el| {
                                            el.child(
                                                Label::new(status_label(status))
                                                    .size(LabelSize::XSmall)
                                                    .color(status_color(status)),
                                            )
                                        })
                                }))
                        }),
                ),
            )
            .child(self.render_inspector(cx))
            .into_any_element()
    }

    fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_job.clone();
        v_flex()
            .w(rems(18.))
            .h_full()
            .p_2()
            .gap_2()
            .border_l_1()
            .border_color(cx.theme().colors().border)
            .child(Label::new(tr!("Inspector")).size(LabelSize::Large))
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("add-job", tr!("Add job"))
                            .on_click(cx.listener(|this, _, _, cx| this.add_job(cx))),
                    )
                    .child(
                        Button::new("remove-job", tr!("Remove"))
                            .disabled(selected.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.remove_selected_job(cx))),
                    ),
            )
            .when_some(selected.clone(), |el, job_id| {
                let job = self.workflow.jobs.get(job_id.as_ref());
                el.child(Label::new(tr_f!("job: {}", job_id)))
                    .when_some(job.cloned(), |el, job| {
                        el.child(
                            Label::new(tr_f!(
                                "runs-on: [{}]  resource: {}",
                                job.runs_on.join(", "),
                                if job.resource.is_empty() {
                                    "—"
                                } else {
                                    job.resource.as_str()
                                }
                            ))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        )
                        .child(
                            Label::new(tr_f!("{} step(s)", job.steps.len())).size(LabelSize::Small),
                        )
                        .child(Label::new(tr!("Toggle needs")).size(LabelSize::Small))
                        .children(
                            self.workflow
                                .jobs
                                .keys()
                                .filter(|id| id.as_str() != job_id.as_ref())
                                .map(|need| {
                                    let active = job.needs.iter().any(|existing| existing == need);
                                    let need = need.clone();
                                    Button::new(
                                        SharedString::from(format!("need-{job_id}-{need}")),
                                        need.clone(),
                                    )
                                    .style(if active {
                                        ButtonStyle::Filled
                                    } else {
                                        ButtonStyle::Subtle
                                    })
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.toggle_need(&need, cx);
                                        },
                                    ))
                                }),
                        )
                    })
            })
            .when(selected.is_none(), |el| {
                el.child(
                    Label::new(tr!("Select a job node to edit dependencies."))
                        .color(Color::Muted)
                        .size(LabelSize::Small),
                )
            })
    }

    fn render_simulate_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .p_2()
            .child(
                Button::new("sim-step", tr!("Step")).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(sim) = this.simulation.as_mut() {
                        match sim.step() {
                            Ok(changed) => {
                                this.status = tr_f!("Advanced: {}", changed.join(", "));
                                this.error = None;
                            }
                            Err(error) => this.error = Some(format!("{error:#}").into()),
                        }
                    }
                    cx.notify();
                })),
            )
            .child(
                Button::new("sim-play", tr!("Play all")).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(sim) = this.simulation.as_mut() {
                        match sim.play_all() {
                            Ok(()) => {
                                this.status = tr!("Simulation finished.");
                                this.error = None;
                            }
                            Err(error) => this.error = Some(format!("{error:#}").into()),
                        }
                    }
                    cx.notify();
                })),
            )
            .child(Button::new("sim-reset", tr!("Reset")).on_click(cx.listener(
                |this, _, _, cx| {
                    if let Some(sim) = this.simulation.as_mut() {
                        sim.reset();
                        this.status = tr!("Simulation reset.");
                    }
                    cx.notify();
                },
            )))
            .child(
                Button::new("sim-fail", tr!("Fail selected")).on_click(cx.listener(
                    |this, _, _, cx| {
                        if let (Some(sim), Some(job)) =
                            (this.simulation.as_mut(), this.selected_job.clone())
                        {
                            let keys = sim.keys_for_job_id(job.as_ref());
                            if keys.is_empty() {
                                this.error =
                                    Some(format!("unknown simulated job {}", job.as_ref()).into());
                            } else {
                                for key in keys {
                                    if let Err(error) = sim.mark_failed(&key) {
                                        this.error = Some(format!("{error:#}").into());
                                        break;
                                    }
                                }
                            }
                        }
                        cx.notify();
                    },
                )),
            )
    }
}

fn create_yaml_editor(
    text: &str,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<Editor>, Entity<Buffer>) {
    let buffer = cx.new(|cx| Buffer::local(text.to_string(), cx));
    let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer.clone(), cx));
    let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));
    (editor, buffer)
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

impl Focusable for WorkflowView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<ItemEvent> for WorkflowView {}

impl Item for WorkflowView {
    type Event = ItemEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        tr!("Wuling Workflow")
    }

    fn is_dirty(&self, _cx: &App) -> bool {
        self.dirty
    }
}

impl Render for WorkflowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .track_focus(&self.focus_handle(cx))
            .child(self.render_toolbar(cx))
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(cx.theme().status().error_background)
                        .child(Label::new(error).color(Color::Error).size(LabelSize::Small)),
                )
            })
            .child(
                div().px_2().child(
                    Label::new(self.status.clone())
                        .color(Color::Muted)
                        .size(LabelSize::Small),
                ),
            )
            .when(self.mode == ViewMode::Simulate, |el| {
                el.child(self.render_simulate_controls(cx))
            })
            .child(match self.mode {
                ViewMode::Flow | ViewMode::Simulate => self.render_flow(cx),
                ViewMode::Yaml => div()
                    .size_full()
                    .child(self.yaml_editor.clone())
                    .into_any_element(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ama10::workflow_simulate::SimulatedJobStatus;
    use gpui::TestAppContext;
    use project::Project;
    use serde_json::json;
    use settings::SettingsStore;

    fn init_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            editor::init(cx);
            release_channel::init(semver::Version::new(0, 0, 0), cx);
        });
    }

    #[gpui::test]
    async fn workflow_view_marks_dirty_on_edit_and_not_on_failed_toggle(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = fs::FakeFs::new(cx.executor());
        fs.insert_tree("/root", json!({ "README.md": "" })).await;
        let project = Project::test(fs, ["/root".as_ref()], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let view = workspace.update_in(cx, |workspace, window, cx| {
            cx.new(|cx| {
                WorkflowView::new(workspace.weak_handle(), project.clone(), None, window, cx)
            })
        });

        view.update(cx, |view, cx| {
            assert!(!view.is_dirty(cx));
            assert_eq!(view.tab_content_text(0, cx).as_ref(), "Wuling Workflow");
            view.add_job(cx);
            assert!(view.is_dirty(cx));
            assert!(view.selected_job.is_some());
        });

        view.update(cx, |view, cx| {
            view.dirty = false;
            view.selected_job = Some("test".into());
            // Self-dependency is ignored and must not mark dirty.
            view.toggle_need("test", cx);
            assert!(!view.dirty);
            assert!(view.error.is_none());
        });
    }

    #[gpui::test]
    async fn workflow_view_simulation_keys_and_fail_selected(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = fs::FakeFs::new(cx.executor());
        fs.insert_tree("/root", json!({ "README.md": "" })).await;
        let project = Project::test(fs, ["/root".as_ref()], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let view = workspace.update_in(cx, |workspace, window, cx| {
            cx.new(|cx| {
                WorkflowView::new(workspace.weak_handle(), project.clone(), None, window, cx)
            })
        });

        view.update(cx, |view, cx| {
            let simulation = WorkflowSimulation::from_workflow(&view.workflow).unwrap();
            assert_eq!(
                simulation.status_for_job_id("build"),
                Some(SimulatedJobStatus::Ready)
            );
            view.simulation = Some(simulation);
            view.selected_job = Some("build".into());
            view.mode = ViewMode::Simulate;
            cx.notify();
        });

        view.update(cx, |view, cx| {
            let sim = view.simulation.as_mut().unwrap();
            for key in sim.keys_for_job_id("build") {
                sim.mark_failed(&key).unwrap();
            }
            assert_eq!(
                sim.status_for_job_id("build"),
                Some(SimulatedJobStatus::Failed)
            );
            assert_eq!(
                sim.status_for_job_id("test"),
                Some(SimulatedJobStatus::Blocked)
            );
            cx.notify();
        });
    }

    #[gpui::test]
    async fn workflow_view_save_detects_disk_conflict(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = fs::FakeFs::new(cx.executor());
        fs.insert_tree("/root", json!({ "README.md": "" })).await;
        let project = Project::test(fs, ["/root".as_ref()], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let workflow_yaml = Workflow::default_ci_seed().to_yaml().unwrap();
        let temp_dir = std::env::temp_dir().join(format!(
            "ama10-workflow-conflict-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let workflow_dir = temp_dir.join(WORKFLOW_DIR);
        std::fs::create_dir_all(&workflow_dir).unwrap();
        let path = workflow_dir.join("ci.yml");
        std::fs::write(&path, &workflow_yaml).unwrap();
        let baseline = file_mtime(&path);
        assert!(baseline.is_some());

        let view = workspace.update_in(cx, |workspace, window, cx| {
            cx.new(|cx| {
                WorkflowView::new(
                    workspace.weak_handle(),
                    project.clone(),
                    Some(path.clone()),
                    window,
                    cx,
                )
            })
        });

        // Change the file on disk after load so save sees a stale baseline.
        std::fs::write(&path, format!("{workflow_yaml}\n# external edit\n")).unwrap();
        let current = file_mtime(&path);
        assert_ne!(baseline, current);

        view.update_in(cx, |view, window, cx| {
            view.disk_mtime = baseline;
            view.dirty = true;
            view.save_to_disk(window, cx);
            assert!(
                view.error
                    .as_ref()
                    .is_some_and(|error| error.as_ref().contains("File changed on disk")),
                "expected conflict error, got {:?}",
                view.error
            );
            assert!(view.dirty, "conflict must preserve in-memory dirty state");
        });

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
