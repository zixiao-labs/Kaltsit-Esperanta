//! Workspace Item: org runner-config.yaml wizard + YAML editor.

use ama10::auth::{Org, PutRunnerConfigRequest, RunnerConfigDocument};
use ama10::runner_config::{
    AliyunPool, AwsPool, EMPTY_SEED, PROVIDER_ALIYUN, PROVIDER_AWS, Pool, RunnerConfig, TierSpec,
};
use ama10_i18n::tr;
use client::Client;
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, WeakEntity, Window,
    actions, prelude::*,
};
use language::Buffer;
use ui::{ToggleButtonGroup, ToggleButtonSimple, prelude::*};
use workspace::{
    Workspace,
    item::{Item, ItemEvent},
};

use crate::wuling_client::{load_access_token, require_slug, wuling_client};

actions!(
    ama10,
    [
        /// Open the Wuling runner-config configurator in an editor tab.
        OpenRunnerConfig,
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Wizard,
    Yaml,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Basics,
    Tiers,
    Pools,
    Review,
}

pub struct RunnerConfigView {
    #[allow(dead_code)]
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    mode: ViewMode,
    step: WizardStep,
    orgs: Vec<Org>,
    selected_org: Option<SharedString>,
    config: RunnerConfig,
    yaml_editor: Entity<Editor>,
    pending_yaml: Option<String>,
    blob_sha: String,
    status: SharedString,
    error: Option<SharedString>,
    warnings: Vec<SharedString>,
    dirty: bool,
    loading: bool,
}

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &OpenRunnerConfig, window, cx| {
            open_or_reuse_runner_config(workspace, window, cx);
        });
    })
    .detach();
}

pub fn open_or_reuse_runner_config(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if let Some(existing) = workspace.item_of_type::<RunnerConfigView>(cx) {
        workspace.activate_item(&existing, true, true, window, cx);
        return;
    }
    let view = cx.new(|cx| RunnerConfigView::new(workspace.weak_handle(), window, cx));
    workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
}

impl RunnerConfigView {
    fn new(
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = RunnerConfig::default_seed();
        let yaml = config.to_yaml().unwrap_or_else(|_| EMPTY_SEED.to_string());
        let yaml_editor = create_yaml_editor(&yaml, window, cx);
        let this = Self {
            workspace,
            focus_handle: cx.focus_handle(),
            mode: ViewMode::Wizard,
            step: WizardStep::Basics,
            orgs: Vec::new(),
            selected_org: None,
            config,
            yaml_editor,
            pending_yaml: None,
            blob_sha: String::new(),
            status: tr!("Connect Wuling, then pick an organization.").into(),
            error: None,
            warnings: Vec::new(),
            dirty: false,
            loading: false,
        };
        this.spawn_load_orgs(cx);
        this
    }

    fn flush_pending_yaml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(yaml) = self.pending_yaml.take() {
            set_editor_text(&self.yaml_editor, &yaml, window, cx);
        }
    }

    fn spawn_load_orgs(&self, cx: &mut Context<Self>) {
        let credentials = Client::global(cx).credentials_provider();
        let Ok(client) = wuling_client(credentials, cx) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let token = match load_access_token(&client, cx).await {
                Ok(token) => token,
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.error = Some(format!("{error:#}").into());
                        this.status = tr!("Not signed in to Wuling DevOps.").into();
                        cx.notify();
                    });
                    return;
                }
            };
            match client.list_orgs(&token).await {
                Ok(orgs) => {
                    this.update(cx, |this, cx| {
                        this.orgs = orgs;
                        if this.selected_org.is_none()
                            && let Some(first) = this.orgs.first()
                            && let Some(slug) = first.slug.clone()
                        {
                            this.selected_org = Some(slug.into());
                            this.spawn_reload(cx);
                        }
                        this.error = None;
                        this.status = tr!("Select an organization to load runner-config.").into();
                        cx.notify();
                    });
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.error = Some(format!("{error:#}").into());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn spawn_reload(&mut self, cx: &mut Context<Self>) {
        let Some(org) = self.selected_org.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        cx.notify();
        let credentials = Client::global(cx).credentials_provider();
        let Ok(client) = wuling_client(credentials, cx) else {
            self.loading = false;
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = async {
                let token = load_access_token(&client, cx).await?;
                let org = require_slug(Some(org.as_ref()))?;
                anyhow::Ok(client.get_runner_config(&token, &org).await?)
            }
            .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(doc) => this.apply_document(doc, cx),
                    Err(error) => {
                        this.error = Some(format!("{error:#}").into());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn apply_document(&mut self, doc: RunnerConfigDocument, cx: &mut Context<Self>) {
        let content = if doc.exists {
            doc.content.clone()
        } else {
            EMPTY_SEED.to_string()
        };
        self.blob_sha = doc.blob_sha.clone();
        self.warnings = doc
            .warnings
            .iter()
            .map(|warning| SharedString::from(warning.clone()))
            .collect();
        match RunnerConfig::parse(&content) {
            Ok((config, report)) => {
                self.config = config;
                self.warnings
                    .extend(report.warnings.into_iter().map(SharedString::from));
                self.error = doc.parse_error.clone().map(SharedString::from);
            }
            Err(error) => {
                self.error = Some(format!("{error:#}").into());
            }
        }
        self.pending_yaml = Some(content);
        self.dirty = !doc.exists;
        self.status = if doc.exists {
            tr!("Loaded runner-config.yaml.").into()
        } else {
            tr!("No runner-config yet — editing the seed template.").into()
        };
        cx.notify();
    }

    fn spawn_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.sync_from_active_mode(window, cx) {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        let Some(org) = self.selected_org.clone() else {
            self.error = Some(tr!("Select an organization first.").into());
            cx.notify();
            return;
        };
        let content = match self.config.to_yaml() {
            Ok(yaml) => yaml,
            Err(error) => {
                self.error = Some(format!("{error:#}").into());
                cx.notify();
                return;
            }
        };
        if let Err(error) = RunnerConfig::parse(&content) {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        self.loading = true;
        self.error = None;
        cx.notify();
        let credentials = Client::global(cx).credentials_provider();
        let Ok(client) = wuling_client(credentials, cx) else {
            self.loading = false;
            return;
        };
        let blob_sha = self.blob_sha.clone();
        let request: PutRunnerConfigRequest = match serde_json::from_value(serde_json::json!({
            "content": content,
            "message": "Update runner-config.yaml from Esperanta",
            "base_blob_sha": blob_sha,
        })) {
            Ok(request) => request,
            Err(error) => {
                self.loading = false;
                self.error = Some(format!("{error:#}").into());
                cx.notify();
                return;
            }
        };
        cx.spawn(async move |this, cx| {
            let result = async {
                let token = load_access_token(&client, cx).await?;
                let org = require_slug(Some(org.as_ref()))?;
                let if_match = if blob_sha.is_empty() {
                    None
                } else {
                    Some(blob_sha.as_str())
                };
                anyhow::Ok(
                    client
                        .put_runner_config(&token, &org, request, if_match)
                        .await?,
                )
            }
            .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(doc) => {
                        this.status = if doc.unchanged.unwrap_or(false) {
                            tr!("Unchanged — already up to date.").into()
                        } else {
                            tr!("Saved runner-config.yaml.").into()
                        };
                        this.apply_document(doc, cx);
                        this.dirty = false;
                    }
                    Err(error) => {
                        this.error = Some(format!("{error:#}").into());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn sync_from_active_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match self.mode {
            ViewMode::Wizard => {
                let yaml = self.config.to_yaml()?;
                set_editor_text(&self.yaml_editor, &yaml, window, cx);
                Ok(())
            }
            ViewMode::Yaml => {
                let text = self.yaml_editor.read(cx).text(cx);
                let (config, report) = RunnerConfig::parse(&text)?;
                self.config = config;
                self.warnings = report.warnings.into_iter().map(SharedString::from).collect();
                Ok(())
            }
        }
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
        self.mode = mode;
        self.error = None;
        cx.notify();
    }

    fn mark_dirty(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let org_buttons = h_flex().gap_1().children(self.orgs.iter().take(8).map(|org| {
            let slug = org.slug.clone().unwrap_or_default();
            let selected = self
                .selected_org
                .as_ref()
                .is_some_and(|selected| selected.as_ref() == slug);
            let label = org
                .display_name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| slug.clone());
            Button::new(SharedString::from(format!("org-{slug}")), label)
                .style(if selected {
                    ButtonStyle::Filled
                } else {
                    ButtonStyle::Subtle
                })
                .on_click(cx.listener({
                    let slug = slug.clone();
                    move |this, _, _, cx| {
                        this.selected_org = Some(slug.clone().into());
                        this.spawn_reload(cx);
                    }
                }))
        }));

        h_flex()
            .w_full()
            .gap_2()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(org_buttons)
            .child(
                ToggleButtonGroup::single_row(
                    "runner-config-mode",
                    [
                        ToggleButtonSimple::new(
                            tr!("Wizard"),
                            cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewMode::Wizard, window, cx);
                            }),
                        ),
                        ToggleButtonSimple::new(
                            tr!("YAML"),
                            cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewMode::Yaml, window, cx);
                            }),
                        ),
                    ],
                )
                .selected_index(usize::from(self.mode != ViewMode::Wizard)),
            )
            .child(
                Button::new("reload-runner-config", tr!("Reload"))
                    .disabled(self.loading || self.selected_org.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.spawn_reload(cx))),
            )
            .child(
                Button::new("save-runner-config", tr!("Save"))
                    .style(ButtonStyle::Filled)
                    .disabled(self.loading || self.selected_org.is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.spawn_save(window, cx))),
            )
            .when(self.dirty, |el| {
                el.child(
                    Label::new(tr!("Unsaved"))
                        .color(Color::Warning)
                        .size(LabelSize::Small),
                )
            })
    }

    fn render_wizard(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let steps = [
            (WizardStep::Basics, tr!("Basics")),
            (WizardStep::Tiers, tr!("Tiers")),
            (WizardStep::Pools, tr!("Pools")),
            (WizardStep::Review, tr!("Review")),
        ];
        h_flex().size_full().child(
            v_flex()
                .w(rems(12.))
                .p_2()
                .gap_1()
                .border_r_1()
                .border_color(cx.theme().colors().border)
                .children(steps.into_iter().map(|(step, label)| {
                    let selected = self.step == step;
                    Button::new(SharedString::from(format!("step-{label}")), label)
                        .style(if selected {
                            ButtonStyle::Filled
                        } else {
                            ButtonStyle::Subtle
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.step = step;
                            cx.notify();
                        }))
                })),
        )
        .child(
            v_flex()
                .size_full()
                .p_3()
                .gap_3()
                .child(match self.step {
                    WizardStep::Basics => self.render_basics(cx).into_any_element(),
                    WizardStep::Tiers => self.render_tiers(cx).into_any_element(),
                    WizardStep::Pools => self.render_pools(cx).into_any_element(),
                    WizardStep::Review => self.render_review(cx).into_any_element(),
                }),
        )
    }

    fn render_basics(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(Label::new(tr!("Default tier and idle timeout")).size(LabelSize::Large))
            .child(
                h_flex()
                    .gap_2()
                    .child(Label::new(tr!("default_tier")))
                    .children(["low", "medium", "high"].map(|tier| {
                        let selected = self.config.default_tier == tier;
                        Button::new(SharedString::from(format!("default-tier-{tier}")), tier)
                            .style(if selected {
                                ButtonStyle::Filled
                            } else {
                                ButtonStyle::Subtle
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.config.default_tier = tier.to_string();
                                this.mark_dirty(cx);
                            }))
                    })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(Label::new(tr!("idle_timeout")))
                    .children(["1m", "5m", "15m", "30m", "1h"].map(|timeout| {
                        let selected = self.config.idle_timeout == timeout;
                        Button::new(SharedString::from(format!("idle-{timeout}")), timeout)
                            .style(if selected {
                                ButtonStyle::Filled
                            } else {
                                ButtonStyle::Subtle
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.config.idle_timeout = timeout.to_string();
                                this.mark_dirty(cx);
                            }))
                    })),
            )
    }

    fn render_tiers(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let names: Vec<_> = self.config.tiers.keys().cloned().collect();
        v_flex()
            .gap_3()
            .child(Label::new(tr!("Resource tiers")).size(LabelSize::Large))
            .children(names.into_iter().map(|name| {
                let tier = self.config.tiers.get(&name).cloned().unwrap_or(TierSpec {
                    cpu: 0,
                    memory: String::new(),
                    storage: String::new(),
                });
                v_flex()
                    .gap_1()
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_md()
                    .child(Label::new(name.clone()))
                    .child(
                        Label::new(format!(
                            "cpu={}  memory={}  storage={}",
                            tier.cpu, tier.memory, tier.storage
                        ))
                        .color(Color::Muted)
                        .size(LabelSize::Small),
                    )
                    .child(
                        h_flex().gap_1().child(Label::new(tr!("CPU")).size(LabelSize::Small)).children(
                            [2, 4, 8, 16].map(|cpu| {
                                Button::new(
                                    SharedString::from(format!("{name}-cpu-{cpu}")),
                                    format!("{cpu}"),
                                )
                                .style(if tier.cpu == cpu {
                                    ButtonStyle::Filled
                                } else {
                                    ButtonStyle::Subtle
                                })
                                .on_click(cx.listener({
                                    let name = name.clone();
                                    move |this, _, _, cx| {
                                        if let Some(tier) = this.config.tiers.get_mut(&name) {
                                            tier.cpu = cpu;
                                        }
                                        this.mark_dirty(cx);
                                    }
                                }))
                            }),
                        ),
                    )
            }))
    }

    fn render_pools(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(Label::new(tr!("Autoscaler pools")).size(LabelSize::Large))
                    .child(Button::new("add-aliyun-pool", tr!("Add Aliyun pool")).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.config.pools.push(Pool {
                                name: format!("pool-{}", this.config.pools.len() + 1),
                                provider: PROVIDER_ALIYUN.into(),
                                tier: this.config.default_tier.clone(),
                                os: "linux".into(),
                                labels: vec!["linux".into(), "docker".into()],
                                min: 0,
                                max: 3,
                                aliyun: Some(AliyunPool {
                                    region: "cn-hangzhou".into(),
                                    instance_type: Some("ecs.g7.large".into()),
                                    credentials_secret: "ALIYUN_CREDS".into(),
                                    ..AliyunPool::default()
                                }),
                                aws: None,
                                proxmox: None,
                                vcenter: None,
                            });
                            this.mark_dirty(cx);
                        }),
                    ))
                    .child(Button::new("add-aws-pool", tr!("Add AWS pool")).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.config.pools.push(Pool {
                                name: format!("aws-{}", this.config.pools.len() + 1),
                                provider: PROVIDER_AWS.into(),
                                tier: this.config.default_tier.clone(),
                                os: "linux".into(),
                                labels: vec!["linux".into(), "docker".into()],
                                min: 0,
                                max: 3,
                                aliyun: None,
                                aws: Some(AwsPool {
                                    region: "us-west-2".into(),
                                    instance_type: "c6i.xlarge".into(),
                                    credentials_secret: "AWS_CREDS".into(),
                                    ..AwsPool::default()
                                }),
                                proxmox: None,
                                vcenter: None,
                            });
                            this.mark_dirty(cx);
                        },
                    ))),
            )
            .child(
                Label::new(tr!(
                    "Fill image/network IDs in YAML for production. Proxmox/vCenter: YAML only.",
                ))
                .color(Color::Muted)
                .size(LabelSize::Small),
            )
            .children(self.config.pools.iter().enumerate().map(|(index, pool)| {
                v_flex()
                    .gap_1()
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_md()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(Label::new(pool.name.clone()))
                            .child(
                                Button::new(
                                    SharedString::from(format!("remove-pool-{index}")),
                                    tr!("Remove"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if index < this.config.pools.len() {
                                        this.config.pools.remove(index);
                                        this.mark_dirty(cx);
                                    }
                                })),
                            ),
                    )
                    .child(
                        Label::new(format!(
                            "{} · tier={} · os={} · min={} max={} · labels=[{}]",
                            pool.provider,
                            pool.tier,
                            pool.os,
                            pool.min,
                            pool.max,
                            pool.labels.join(", ")
                        ))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
            }))
    }

    fn render_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let yaml = self
            .config
            .to_yaml()
            .unwrap_or_else(|error| format!("# serialize error: {error:#}"));
        let validation = RunnerConfig::parse(&yaml);
        v_flex()
            .gap_2()
            .child(Label::new(tr!("Review")).size(LabelSize::Large))
            .child(match &validation {
                Ok((_, report)) if report.warnings.is_empty() => {
                    Label::new(tr!("Configuration is valid.")).color(Color::Success)
                }
                Ok((_, report)) => Label::new(format!(
                    "Valid with warnings:\n{}",
                    report.warnings.join("\n")
                ))
                .color(Color::Warning),
                Err(error) => Label::new(format!("{error:#}")).color(Color::Error),
            })
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().colors().editor_background)
                    .child(
                        Label::new(yaml)
                            .buffer_font(cx)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
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

impl Focusable for RunnerConfigView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<ItemEvent> for RunnerConfigView {}

impl Item for RunnerConfigView {
    type Event = ItemEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        tr!("Wuling Runner Config").into()
    }
}

impl Render for RunnerConfigView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.flush_pending_yaml(window, cx);
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
            .when(!self.warnings.is_empty(), |el| {
                el.child(div().px_2().py_1().children(self.warnings.iter().map(|warning| {
                    Label::new(warning.clone())
                        .color(Color::Warning)
                        .size(LabelSize::Small)
                })))
            })
            .child(
                div().p_2().child(
                    Label::new(self.status.clone())
                        .color(Color::Muted)
                        .size(LabelSize::Small),
                ),
            )
            .child(match self.mode {
                ViewMode::Wizard => self.render_wizard(cx).into_any_element(),
                ViewMode::Yaml => div()
                    .size_full()
                    .child(self.yaml_editor.clone())
                    .into_any_element(),
            })
    }
}
