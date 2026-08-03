//! Workspace Item: Wuling workflow flowchart, YAML editor, and local simulate.

use std::path::PathBuf;

use ama10::workflow::{WORKFLOW_DIR, Workflow};
use ama10::workflow_simulate::WorkflowSimulation;
use ama10_i18n::tr;
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, WeakEntity, Window,
    actions, prelude::*,
};
use language::Buffer;
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
    workflow: Workflow,
    layout: FlowLayout,
    selected_job: Option<SharedString>,
    yaml_editor: Entity<Editor>,
    pending_yaml: Option<String>,
    simulation: Option<WorkflowSimulation>,
    status: SharedString,
    error: Option<SharedString>,
    dirty: bool,
    available_files: Vec<PathBuf>,
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
        let yaml_editor = create_yaml_editor(&yaml, window, cx);
        let mut this = Self {
            workspace,
            project,
            focus_handle: cx.focus_handle(),
            mode: ViewMode::Flow,
            workflow_path: None,
            workflow,
            layout,
            selected_job: None,
            yaml_editor,
            pending_yaml: None,
            simulation: None,
            status: tr!("Edit .wuling/workflows locally. Simulate does not trigger remote runs.")
                .into(),
            error: None,
            dirty: false,
            available_files: Vec::new(),
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
                    self.workflow_path = Some(path);
                    self.dirty = false;
                    self.status = tr!("Loaded workflow file.").into();
                    self.error = None;
                }
                Err(error) => {
                    self.workflow_path = Some(path);
                    set_editor_text(&self.yaml_editor, &text, window, cx);
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
            set_editor_text(&self.yaml_editor, &yaml, window, cx);
        } else if let Ok(yaml) = self.workflow.to_yaml() {
            set_editor_text(&self.yaml_editor, &yaml, window, cx);
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
                set_editor_text(&self.yaml_editor, &yaml, window, cx);
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
        match std::fs::write(&path, yaml.as_bytes()) {
            Ok(()) => {
                self.workflow_path = Some(path.clone());
                self.dirty = false;
                self.refresh_file_list(cx);
                self.status = SharedString::from(format!("Saved {}", path.display()));
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
        self.dirty = true;
        self.mode = ViewMode::Flow;
        self.status =
            tr!("Created a CI workflow seed — Save to write .wuling/workflows/ci.yml.").into();
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
        self.dirty = true;
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
        let status_map = self
            .simulation
            .as_ref()
            .map(|sim| sim.status_map())
            .unwrap_or_default();
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
                                    Label::new(format!("stage {layer_ix}"))
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
                                                "needs: —".to_string()
                                            } else {
                                                format!("needs: {}", node.needs.join(", "))
                                            }
                                        })
                                        .unwrap_or_default();
                                    let is_selected =
                                        self.selected_job.as_ref().map(|s| s.as_ref())
                                            == Some(id.as_str());
                                    let status = status_map.get(id).copied();
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
                el.child(Label::new(format!("job: {job_id}")))
                    .when_some(job.cloned(), |el, job| {
                        el.child(
                            Label::new(format!(
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
                            Label::new(format!("{} step(s)", job.steps.len()))
                                .size(LabelSize::Small),
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
                                this.status =
                                    SharedString::from(format!("Advanced: {}", changed.join(", ")));
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
                                this.status = tr!("Simulation finished.").into();
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
                        this.status = tr!("Simulation reset.").into();
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
                            if let Err(error) = sim.mark_failed(job.as_ref()) {
                                this.error = Some(format!("{error:#}").into());
                            }
                        }
                        cx.notify();
                    },
                )),
            )
    }
}

fn create_yaml_editor(text: &str, window: &mut Window, cx: &mut App) -> Entity<Editor> {
    let buffer = cx.new(|cx| Buffer::local(text.to_string(), cx));
    let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
    cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx))
}

fn set_editor_text(editor: &Entity<Editor>, text: &str, window: &mut Window, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        editor.set_text(text, window, cx);
    });
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
        tr!("Wuling Workflow").into()
    }
}

impl Render for WorkflowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(yaml) = self.pending_yaml.take() {
            set_editor_text(&self.yaml_editor, &yaml, window, cx);
        }
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
