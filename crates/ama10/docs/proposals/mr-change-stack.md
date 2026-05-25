# MR Change Stack — 设计提案

**状态**：调研期，未启动实施。
**目标会议**：2026-06-15 @HwlloChen 回归后的架构对齐会议。
**前置参考**：[[esperanta-is-an-independent-fork-release]] · [[architect-hwllochen-return-2026-06-15]] · [[ama10-+-wuling-openapi-client-conventions]]

---

## 1. 动机

目前 Esperanta 端只有 `crates/ama10-ui/src/issues_panel.rs` 一个只读列表面板，无法在编辑器内对 Wuling-DevOps 上的 Merge Request 做评审。期望在编辑器侧达到 CodeRabbit Change Stack 那种"一站式 MR 评审"体验，同时复用 Zed 自身已经成熟的 `MultiBuffer` + `BufferDiff` + `BlockMap` + ACP/native agent 基础设施，避免另起炉灶。

核心交付：

1. 在 workspace tab 里以一个 **Item** 形态打开一个 MR，整页占据中心区。
2. 左栏 Change Stack（按 commit 分层）+ 中心多文件 Diff（MultiBuffer 一个滚动视图，所有变动文件全装进去）+ 右栏可切换 Context / Comments。AI 线程不内嵌 — "Review with AI" 按钮一键派发到 Zed 全局 `agent_panel`（用户布局决定它在左还是右）。
3. 行内评论锚定在 diff 的具体行上，跨 buffer 编辑能正确漂移；评论 / 线程 / 已解决态走后端 schema（见 §6）。
4. "Review with AI" 一键弹出 agent picker（内部 native agent 或外部 ACP agent 均可），把 MR 全文上下文打包成 `AgentInitialContent::ContentBlock` 起线程。

## 2. 非目标

- 不替代 Wuling-DevOps 网页端，不实现 web 端独有的功能（如管理员后台、CI 配置）。
- 不做多 MR 比对、跨仓库 stack（不属于单 MR 评审范畴）。
- 不接管 Zed 已有的 `git_ui` 本地分支 diff 流程；那条路保持独立，本 panel 只负责远端 MR。
- 不在第一版做 suggested edits / "apply suggestion" 自动写回（取决于后端是否提 schema）。

## 3. UX 形态（已对齐）

Workspace Item 内部三栏布局：

```
┌─ agent_panel ─┬─[Tabs: MR #30 ✕]───────────────────────────────────┬─ project_panel ─┐
│   (AGENT      │┌─Stack──────┬─MultiBuffer Editor──────┬─Right Pane┐│   (AGENT 布局     │
│   布局停左)   ││[Review w/ AI]│ ▼ frontend/.../diff.tsx │ Tabs:    ││   停右；EDITOR    │
│               ││            │   @@ -1,7 +1,5 @@ ...   │  Context ││   布局二者互换)   │
│  (Acp/native  ││ ⊙ commit 1 │   ...                    │  Comments││                  │
│   线程列表)   ││   diff.tsx │ ▼ frontend/.../empty.tsx│          ││                  │
│               ││ ⊙ commit 2 │   @@ -5,3 +5,7 @@ ...   │ (按行 / 文 ││                 │
│               ││ (active)   │   ...                    │ 件聚合的 ││                 │
│               ││ ⊙ commit 3 │ ▼ frontend/.../layout   │ 评论线程) ││                 │
│               │└────────────┴──────────────────────────┴──────────┘│                  │
└───────────────┴────────────────────────────────────────────────────┴──────────────────┘
```

> **Dock 协调**：本 workspace item **不嵌 AI Chat pane**。AI 线程一律走 Zed 全局 `agent_panel` —— 它落在用户偏好的位置（[[mr-change-stack-design-pending]] 引述的 `agent_settings::PanelLayout::AGENT`/`EDITOR` 两种预设决定）。这样 AGENT 布局用户开 AI 时不会被强行拽到右边，EDITOR 布局用户也不会觉得 workspace item 抢了 agent panel 的位置。

- **左栏 Stack**：按 commit 分层（用 `/merge-requests/{n}/commits` 返回的 commit 列表）。每层下展开该 commit 涉及的文件 + viewed 勾选。点击 layer 切换 active commit，editor 的 MultiBuffer 切到该 commit 的 excerpts。**顶部一个 `[Review with AI]` 按钮**（不是嵌入 chat，而是一次性 dispatch），点了去打开/聚焦全局 agent_panel。
- **中栏 Editor**：所有变动文件 excerpt 进同一个 `MultiBuffer`，从上到下滚到底；每个文件之间用 Zed 自带的 excerpt 分隔。光标/选择/搜索 等编辑器交互直接复用。
- **右栏 Right Pane**：可切换 "Context" / "Comments"（**不含 AI Chat**）：
  - Context：MR 标题、描述、状态、author、target/source ref、CI 状态摘要。
  - Comments：当前 active 行 / 选中范围的所有评论线程（含未解决数量徽章）。

## 4. 架构概览

### 4.1 crate 落点

```
crates/
  ama10/                       # Wuling HTTP client 扩展
    src/wuling_api.rs            # 新增 mr_diff / mr_comments 辅助函数
  ama10-ui/                    # fork 专属 UI
    src/mr_change_stack/
      panel.rs                   # Workspace Item，路由 + 容器
      stack_pane.rs              # 左栏 Change Stack
      diff_pane.rs               # 中栏 MultiBuffer 包装
      right_pane.rs              # 右栏切换器
      comment_thread.rs          # 行内评论 CustomBlock 渲染器
      ai_review.rs               # "Review with AI" 按钮 + dispatch（不持有 AcpThread）
      types.rs                   # ChangeStackState 等核心数据结构
      events.rs                  # 面板内部事件 / 订阅
  ama10-i18n/                  # i18n 在本 panel 启动
    locales/zh-CN.json
    locales/en.json
    src/ama10_i18n.rs            # lookup API + 默认 zh-CN
```

> 不新建 crate，所有代码都进 `ama10-ui`。`ama10` 只新增几个针对 MR 的薄包装函数；`generated.rs` 不动（参见 [[ama10-+-wuling-openapi-client-conventions]] Trap 1）。

### 4.2 实体层次

```
Workspace
  ├─ Item (Box<dyn Item>):  MrChangeStackView
  │    ├─ Entity<MrChangeStackState>      // 数据 / fetch 状态机
  │    ├─ Entity<MultiBuffer>             // 跨文件 excerpt
  │    ├─ Entity<Editor>                  // 包 MultiBuffer
  │    └─ Entity<CommentStore>            // 评论缓存 + 锚点管理
  └─ AgentPanel (全局 dock，Left 或 Right 取决于用户布局)
       └─ AcpThread (按需开，由 "Review with AI" 触发；线程归 agent_panel 管，不归本 Item 管)
```

`MrChangeStackState` 持有：
- `mr_meta: Option<MergeRequest>` — `/merge-requests/{n}` 返回值
- `commits: Vec<MrCommit>` — `/merge-requests/{n}/commits`
- `diff: Option<MrDiff>` — `/merge-requests/{n}/diff?include=patch`
- `viewed: HashSet<PathKey>` — 客户端本地（后端无此 schema，第一版本地）
- `active_layer: LayerSelection { All | Commit(oid) }`
- 各 fetch 子状态（`FetchState::Idle | Loading | Loaded(_) | Error(String)`），照搬 `issues_panel.rs` 的模式
- 订阅 `WulingAccountState` 变更，账号切换时清空

`CommentStore` 持有：
- `BTreeMap<PathKey, Vec<CommentThread>>`，其中 `CommentThread` 含 `Vec<Comment>` + `resolved: bool` + 锚点 `(path, text::Anchor)`
- 监听 `MultiBuffer` 的 buffer edit 事件，让 `text::Anchor` 自然漂移（无需手写逻辑）

### 4.3 数据流：用户点 MR → 看到 diff

```
issues_panel / MR list click
  → workspace.open_or_focus_mr_view(mr_ref)
      → MrChangeStackView::open(workspace, mr_ref, cx)
          ├─ fetch_meta()       → MergeRequest
          ├─ fetch_commits()    → Vec<MrCommit>
          ├─ fetch_diff()       → MrDiff (含 patch)
          ├─ build_multibuffer():
          │     for each file in diff.files:
          │       buffer = Buffer::local(file.head_content, cx)
          │       buffer_diff = BufferDiff::from_patch(file.patch, cx)
          │       multibuffer.set_excerpts_for_path(
          │         PathKey::from(file.path), buffer, ranges, ctx_lines, cx);
          │       multibuffer.add_diff(buffer_diff, cx);
          └─ Item 装入 workspace.add_item_to_active_pane(...)
```

`build_multibuffer` 完全沿用 `crates/git_ui/src/multi_diff_view.rs` 的模式（参见 `register_entry` 调用 `set_excerpts_for_path` + `add_diff`）。**核心点**：buffer 是 `Buffer::local`，不挂载到 project，是合成的纯内存 buffer。

### 4.4 行内评论渲染

每条评论线程注册成一个 `CustomBlock`：

```
BlockProperties {
  placement: BlockPlacement::Below(thread.anchor),
  render: Arc::new(|ctx| comment_thread_view(thread.clone(), ctx)),
  ...
}
editor.insert_blocks([props], None, cx);
```

锚点 `thread.anchor: text::Anchor` 在 buffer 编辑时自动漂移（参见 `crates/text/src/anchor.rs` 的 Lamport timestamp 设计）。多个评论同一行用 `BlockPlacement::Below` 堆叠或合成一个聚合 block。

**第一版（后端 schema 未就绪前）**：comments pane 只读地展示后端 `/merge-requests/{n}/comments` 返回的 MR 级评论，渲染在右栏列表，不锚定到行。Inline CustomBlock 渲染保留代码路径但默认不出现 thread（schema 上线后切换）。

### 4.5 AI Review 入口

按钮在 **Stack 左栏顶部**（不在右栏，也不弹嵌入式 chat），点击后只做一件事：把 MR 上下文打包成 `AgentInitialContent::ContentBlock`，调用 `panel.external_thread(...)` 让**全局 `agent_panel`** 打开新线程：

```rust
fn on_review_with_ai(state: &MrChangeStackState, cx: &mut Context<...>) {
    let blocks = vec![
        acp::ContentBlock::Text { text: review_prompt_template(state) },
        acp::ContentBlock::Resource(TextResourceContents {
            uri: format!("wuling-mr://{}/{}/{}", org, project, mr_number),
            text: state.diff.as_ref().map(unified_patch_string).unwrap_or_default(),
        }),
    ];
    let initial = AgentInitialContent::ContentBlock { blocks, auto_submit: true };
    // 弹 agent picker（与 ReviewBranchDiff 一致），选完后 agent_panel 在自己 dock 位置打开线程
    AgentPanel::new_external_agent_thread_with_initial(
        workspace, /* picker = */ true, /* initial */ Some(initial), cx,
    );
}
```

参考实现：`crates/agent_ui/src/agent_panel.rs` 现有 `ReviewBranchDiff` 动作（约 359-402 行；动手前需 grep 重新校准位置）。两条路径几乎一致，差异只是 URI scheme 和 prompt 模板。

**关键点**：

- 本 workspace item **不持有** `Entity<AcpThread>`。线程生命周期完全归 `agent_panel`。这样切到别的 MR tab、关闭再开 MR tab，AI 对话都不丢。
- `agent_panel` 的 dock 位置遵从用户的 `agent.dock` 设置（`agent_settings::PanelLayout::AGENT` 时在左，`EDITOR` 时在右），workspace item 不干预。
- 第一版只支持一键起新线程；"把当前评论丢给 AI" 这种 inline 行为留到后端 schema 落地后再设计（见 §9 开放问题）。

新增 telemetry 源 `AgentThreadSource::WulingMr`（或新变体 `ExternalMrReview`）以便观测。

## 5. 关键 Zed 原语映射（复用 vs. 新建）

| 需求 | 现有原语 | 是否需要新建 |
|------|--------|-----------|
| 多文件 diff 一个 editor | `MultiBuffer` + `set_excerpts_for_path` + `BufferDiff` | 复用，包一层 fetch 适配 |
| 跨文件 patch 文本 → BufferDiff | `BufferDiff` API（需查 API surface） | 可能需薄构造器 `from_unified_patch(&str)` |
| 行评论锚点漂移 | `text::Anchor`（Lamport 时间戳） | 复用，零代码 |
| 评论卡片 UI | `BlockMap::CustomBlock` + `BlockPlacement::Below(Anchor)` | 复用，写 render closure |
| Workspace tab item | `Item` trait + `workspace.add_item_to_active_pane` | 复用 |
| 一键 AI Review | `AgentInitialContent::ContentBlock` + `external_thread()` | 复用，仿 `ReviewBranchDiff` |
| Native + ACP 统一 | `AcpThread` 已是统一抽象 | 复用 |
| Wuling HTTP 调用 | `crates/ama10/src/wuling_api/generated.rs` + 手写 shim | 在 `wuling_api.rs` 加包装 |
| 账号 / 重定向 | `WulingAccountState` global | 复用，订阅 `WulingAccountChanged` |
| i18n | （目前为空） | **新建** `ama10-i18n` lookup + zh-CN/en 文件 |
| viewed 持久化 | 无 | 第一版本地，长期推后端 |
| 右栏多 pane 切换（Context / Comments） | 无 | 自写 `right_pane.rs`（小组件） |
| AI 线程容器 | Zed 全局 `agent_panel`（Left/Right 由用户布局决定） | 复用，不在 workspace item 内重造 |

**重要**：上表所有"复用"在动手前都要 grep 一次确认 API 没漂（参考 [[ama10-+-wuling-openapi-client-conventions]] 关于点位失稳的提醒）。

## 6. 后端 schema 提案（Wuling-DevOps）

提案分两阶段。一阶段（第一版面板上线必须）补行内评论 / 线程 / resolved；二阶段（增值能力）补 viewed / 批量 review / AI 标记 / suggested edits。

### 6.1 一阶段 — 行内评论 schema

#### 新表：`mr_diff_comments`

```sql
-- 0010_mr_diff_comments.up.sql (假设迁移编号)
CREATE TABLE mr_diff_comments (
    id              BIGSERIAL PRIMARY KEY,
    mr_id           BIGINT NOT NULL REFERENCES merge_requests(id) ON DELETE CASCADE,
    parent_id       BIGINT REFERENCES mr_diff_comments(id) ON DELETE CASCADE, -- NULL = 线程根
    author_id       UUID NOT NULL REFERENCES users(id),

    -- 锚点：MR 视角下的文件 + 行 + 侧（OLD/NEW）
    file_path       TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('LEFT','RIGHT')),
    line_number     INTEGER NOT NULL CHECK (line_number > 0),

    -- 锚点漂移基线：评论创建时所参考的 OID，用于后续判定 outdated
    anchor_oid      TEXT NOT NULL,           -- side=LEFT 时记 base oid；RIGHT 时记 head oid

    body            TEXT NOT NULL CHECK (octet_length(body) <= 65536),
    resolved        BOOLEAN NOT NULL DEFAULT FALSE,
    resolved_by     UUID REFERENCES users(id),
    resolved_at     TIMESTAMPTZ,

    -- outdated 含义：评论锚点所在的 oid 已不在当前 MR 的 base..head 范围内
    -- 由服务端在读取时 / 后台任务里推断；不写入这里

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_mr_diff_comments_mr ON mr_diff_comments(mr_id);
CREATE INDEX idx_mr_diff_comments_thread ON mr_diff_comments(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_mr_diff_comments_file ON mr_diff_comments(mr_id, file_path);
```

设计点：

- **锚点三元组** `(file_path, side, line_number)` + `anchor_oid` 是足够的最小集；rebase / force-push 后服务端可以重算 outdated 而无需删评论。
- **线程**用自引用 `parent_id`（PostgreSQL 标准做法），不引入额外 thread 表，查询简单。
- **resolved** 推荐放在每条评论上（线程根的 resolved 状态由 UI 取根决定），便于后续支持"一个线程多人独立标记"等扩展。
- **suggested edits** 留到二阶段，需要的话再加 `suggested_old_lines TEXT[], suggested_new_lines TEXT[]` 两列或单独建表。

#### 端点

| 方法 | 路径 | 行为 |
|---|---|---|
| `GET` | `/api/v1/orgs/{org}/projects/{proj}/repos/{repo}/merge-requests/{n}/diff-comments` | 列出该 MR 的所有行评论，按 `file_path, line_number, created_at` 排序。可选 `?file_path=X` 过滤。**包含 server-computed `outdated: bool` 字段**。 |
| `POST` | `/api/v1/.../merge-requests/{n}/diff-comments` | 新建评论。Body: `{ file_path, side, line_number, anchor_oid, body, parent_id? }`。返回新建对象（含 `outdated` 计算值）。 |
| `PATCH` | `/api/v1/.../diff-comments/{id}` | 修改 body 或 resolved 状态。Body: `{ body?: string, resolved?: bool }`。Author 改 body，Author / org member 改 resolved。 |
| `DELETE` | `/api/v1/.../diff-comments/{id}` | Author 或 owner 删。删除根会级联（外键 ON DELETE CASCADE）。 |
| `POST` | `/api/v1/.../merge-requests/{n}/reviews:submit` | **新的批量端点**：原子提交一个 review（approve/request-changes/comment）+ 多条行评论 + 一条总评。Body: `{ state, body, diff_comments: [...], top_level_comment?: string }`。后端在事务里写。 |

> 路径风格沿用现有 `mrhttp/handler.go` 的 RESTful 习惯。`reviews:submit` 借鉴 Google AIP 子动作语法，避免和 `POST /reviews` 撞语义。

#### OpenAPI 片段（草稿）

```yaml
components:
  schemas:
    MRDiffComment:
      type: object
      required: [id, mr_id, author, file_path, side, line_number, anchor_oid, body,
                 resolved, outdated, created_at, updated_at]
      properties:
        id:            { type: integer, format: int64 }
        mr_id:         { type: integer, format: int64 }
        parent_id:     { type: integer, format: int64, nullable: true }
        author:        { $ref: '#/components/schemas/UserRef' }
        file_path:     { type: string }
        side:          { type: string, enum: [LEFT, RIGHT] }
        line_number:   { type: integer, minimum: 1 }
        anchor_oid:    { type: string }
        body:          { type: string, maxLength: 65536 }
        resolved:      { type: boolean }
        resolved_by:   { $ref: '#/components/schemas/UserRef', nullable: true }
        resolved_at:   { type: string, format: date-time, nullable: true }
        outdated:      { type: boolean, description: "服务端推断；anchor_oid 不在 base..head 范围时为 true" }
        created_at:    { type: string, format: date-time }
        updated_at:    { type: string, format: date-time }

    MRDiffCommentCreate:
      type: object
      required: [file_path, side, line_number, anchor_oid, body]
      properties:
        file_path:   { type: string }
        side:        { type: string, enum: [LEFT, RIGHT] }
        line_number: { type: integer, minimum: 1 }
        anchor_oid:  { type: string }
        body:        { type: string, maxLength: 65536 }
        parent_id:   { type: integer, format: int64, nullable: true }

    MRDiffCommentUpdate:
      type: object
      properties:
        body:     { type: string, maxLength: 65536 }
        resolved: { type: boolean }

    MRReviewSubmit:
      type: object
      required: [state]
      properties:
        state:               { type: string, enum: [approved, changes_requested, commented] }
        body:                { type: string, maxLength: 65536 }
        top_level_comment:   { type: string, maxLength: 65536, nullable: true }
        diff_comments:
          type: array
          items: { $ref: '#/components/schemas/MRDiffCommentCreate' }
```

`script/regen-wuling-api.sh` 会把上面这套自动转成 `ama10/src/wuling_api/generated.rs` 里的 Rust 类型（前提是后端先合并）。**注意**：`include=patch` 这种现有的 query-driven 行为不要照搬到 diff-comments 上，保持 RESTful。

#### 后端工作量预估（仅供会议参考）

- `mr_diff_comments` 表 + 迁移：~半天
- 5 个端点 + 测试：~2 天
- `outdated` 推断逻辑（重算需要走 git rev-list）：~1 天
- OpenAPI 更新 + 客户端 regen：~半天

合计 4–5 个工作日。如果一阶段先不上 `reviews:submit`，可拆到 3 天。

### 6.2 二阶段 — 增值能力（不阻塞一阶段上线）

- **viewed 状态**：新表 `mr_file_viewed (mr_id, user_id, file_path, viewed_at)`，仅记录 viewed 文件（unviewed = 缺行）。端点 `PUT /merge-requests/{n}/viewed/{file_path}` / `DELETE` 切换。
- **AI 标记**：`mr_diff_comments` 加 `kind TEXT DEFAULT 'human' CHECK (kind IN ('human','ai_review'))` 列 + 可选 `ai_agent TEXT` 标识哪个 agent 留的。
- **Suggested edits**：扩展 `mr_diff_comments` 加 `suggestion_old TEXT, suggestion_new TEXT`，UI 出 "Apply suggestion" 按钮（最终落地需要写 commit）。
- **Outdated 后台任务**：MR 收到 force-push 时主动重算 outdated 状态，避免 GET 时即时算。

二阶段任何一项独立可拆。

## 7. 实施阶段（建议）

> 所有阶段在 2026-06-15 对齐会议拿到 GO 之前不动代码。

**阶段 0 — 设计冻结**（本提案）
本文档 + 探索 issue 上线，等待会议。

**阶段 1 — 只读骨架**（客户端 only，~1 周）
- `ama10-i18n` 启动 + zh-CN/en 基础翻译
- `MrChangeStackView` Workspace Item + 三栏布局
- Stack 左栏 commit 列表，diff 中栏 MultiBuffer 渲染
- 右栏 Context tab 只读，Comments tab 仅显示 MR 级评论
- 不动后端，不接行内评论

**阶段 2 — 后端 schema 落地**（后端 owner，~1 周）
- 一阶段 schema + 端点 + 迁移上线
- `script/regen-wuling-api.sh` 跑一遍，`generated.rs` 更新
- 后端发布灰度环境给客户端联调

**阶段 3 — 行内评论 + 批量 review**（客户端 ~1 周）
- `CommentStore` 接入 `/diff-comments` 端点
- `CustomBlock` 渲染 + 创建评论 UX
- `reviews:submit` 批量提交对话框
- Outdated 提示

**阶段 4 — AI Review 入口**（客户端 ~2 天）
- Stack 左栏顶部 `[Review with AI]` 按钮 + agent picker 复用
- prompt 模板调优 + telemetry 源接入
- 不嵌入 AcpThread；线程开在全局 agent_panel 自然 dock 位置

**阶段 5 — 二阶段增值**（按需）
- viewed 状态服务端化
- AI 评论标记
- Suggested edits

## 8. 风险与缓解

| 风险 | 缓解 |
|---|---|
| `MultiBuffer` 装 50+ 文件大 diff 时性能未知 | 阶段 1 先做一个 ≤30 文件的 MR 压力测试；超过阈值时左栏 stack 改为 lazy 加载，editor 改为只装 active layer 的文件 |
| `text::Anchor` 在 buffer 内容完全替换时（如 force-push 后整文件改写）会指向旧 buffer，需要重新锚 | 服务端 `outdated` flag + 客户端遇到 outdated 评论用旧锚定 LineNumber 转 NEW 锚定（fallback 算法） |
| ACP 外部 agent 收到的 patch 文本太大被截断 | 大 MR 在 prompt 模板里只放文件清单 + 摘要，让 agent 通过工具调用按需拉文件 |
| 后端先 schema 后客户端的时间错位（schema 改了客户端没跟） | `wuling_api/generated.rs` 是 codegen 来源，schema 改后客户端必须 regen；CI 加一个 `check_wuling_api_up_to_date` job（在 [[cicd-rewrite-decisions-for-esperanta-fork]] 范围内补一下） |
| 阶段 1 上线时没有行评论，用户体验断层 | UI 文案明确 "行评论即将上线"，并把入口禁用而不是隐藏，给用户预期 |

## 9. 开放问题（留给 2026-06-15 会议）

1. **Suggested edits 是否一阶段就含**？如果是，schema 需要扩；如果否，二阶段独立做。
2. **批量 review 接口 `reviews:submit` 是否一阶段就要**？或者第一版只支持一条一条 POST，留批量到后面？
3. **AI 评论是否要服务端持久化**（AI agent 留的评论存到 `mr_diff_comments` 表带 `kind='ai_review'`）？还是仅客户端展示，刷新就没？影响 schema 是否要 `kind` 列。
4. **Outdated 计算时机**：每次 GET 实时算（简单但 query 慢）还是 force-push 异步更新（复杂但快）？影响后端实现路径。
5. **viewed 是否一阶段就服务端化**？还是接受第一版本地（多设备不同步）？
6. **i18n 是只翻这个 panel 还是顺带把 `issues_panel.rs` 也翻了**？前者最小化变更，后者一致性更好。
7. **本 panel 是否要单独 feature flag**（`wuling.json` 加 `mr_change_stack.enabled`）？还是直接默认启用？
8. **agent picker 弹出时是否记忆上次选择**？还是每次都让用户选？后者用户感觉受控，前者更顺手。

## 10. 引用

- Zed 原语
  - `crates/multi_buffer/src/multi_buffer.rs` — MultiBuffer 构造 + `set_excerpts_for_path`
  - `crates/buffer_diff/src/buffer_diff.rs` — BufferDiff
  - `crates/git_ui/src/multi_diff_view.rs` — 多文件 diff 入口先例（`load_entries` / `register_entry`）
  - `crates/git_ui/src/project_diff.rs` — workspace Item 形态 diff 先例
  - `crates/editor/src/display_map/block_map.rs` — CustomBlock / BlockPlacement
  - `crates/editor/src/git.rs` — `StoredReviewComment` / `DiffReviewOverlay` 本地评论先例
  - `crates/text/src/anchor.rs` — Lamport timestamp 锚点漂移
  - `crates/acp_thread/src/acp_thread.rs` — AcpThread 统一抽象
  - `crates/acp_thread/src/mention.rs` — MentionUri 枚举（含 `GitDiff` 变体可参考）
  - `crates/agent_ui/src/agent_ui.rs` — `AgentInitialContent::ContentBlock`
  - `crates/agent_ui/src/agent_panel.rs` — `ReviewBranchDiff` + `external_thread()` 先例
  - `crates/agent_settings/src/agent_settings.rs` — `PanelLayout::AGENT` / `EDITOR` 两种 dock 预设（agent_dock 在 Left 或 Right）
  - `crates/ama10-ui/src/issues_panel.rs` — fork panel 骨架范本
  - `crates/zed/src/zed.rs` — `initialize_panels()` 注册点
- Wuling-DevOps
  - `internal/mrhttp/handler.go` — 现有 MR/comments/reviews 处理器
  - `0003_merge_requests.up.sql` — 现有 MR 表
- 记忆引用
  - [[esperanta-is-an-independent-fork-release]]
  - [[architect-hwllochen-return-2026-06-15]]
  - [[ama10-+-wuling-openapi-client-conventions]]
  - [[cicd-rewrite-decisions-for-esperanta-fork]]
  - [[internal-discussions-stay-on-fork]]

---

> **审核者注意**：本文件是设计提案而非实施清单。所有 file:line 引用在动手前必须 grep 一次确认未漂；所有原语调用签名以 `cargo doc --open -p multi_buffer` 等命令的实时输出为准。
