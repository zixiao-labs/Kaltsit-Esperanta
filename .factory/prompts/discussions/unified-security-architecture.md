# 统一安全架构：Agent 沙箱、扩展签名、嵌入式 JS 与 Workflow

> 状态：**设计提案 / 非实现**。日期：2026-08-06。
> 范围：Kaltsit-Esperanta（ZetaCode）安全策略面与强制层；不改代码、不改用户手册。
> 依赖与承接：
> - [Zed Sandboxing](https://zed.dev/blog/sandboxing)
> - [`crates/sandbox/README.md`](../../../crates/sandbox/README.md)
> - [`docs/src/ai/sandboxing.md`](../../../docs/src/ai/sandboxing.md)
> - [`docs/src/ai/tool-permissions.md`](../../../docs/src/ai/tool-permissions.md)
> - [`docs/src/worktree-trust.md`](../../../docs/src/worktree-trust.md)
> - [四大特性前置调研](./extension-runtimes-and-agent-workflows.md)
> - PerlicaScript JIT 安全契约：[`perlicascript/docs/jit-security.md`](../../../../perlicascript/docs/jit-security.md)（假定与本仓同级检出）
>
> 核心约束：**安全性第一，绝不重蹈 VS Code 覆辙**（同进程、满权限、无能力系统）。指令与细粒度命令规则不可作为唯一防线；不可信代码按最大化敌意建模。

---

## 0. 一句话结论

用单一用户选择 `security.level`（`none` → `ultra` / `custom`）绑定四组策略包——**OS 沙箱、扩展签名、嵌入式 JS/Bun/Deno 能力、Agent Workflow 限制**——叠在现有 tool permissions / profiles / worktree trust / WASM 能力闸门之上。现状已有可用的 Agent 终端 OS 沙箱；缺口在统一等级面、扩展验签、脚本化 Workflow 控制面，以及嵌入式 JS / Perlica JIT 的能力与类型安全契约（后两者见分期与兄弟仓文档）。

---

## 1. 威胁模型

### 1.1 资产

| 资产 | 为何重要 |
|------|----------|
| 用户文件系统与密钥 | Agent/扩展可读写项目外路径或 `~/.ssh`、凭据文件 |
| `.git` 与 hooks | 写入可在沙箱外触发执行（commit hook、`$EDITOR`） |
| 网络与内网服务 | 数据外泄、攻击可达主机 |
| 模型调用预算 | Workflow 扇出可造成费用失控 |
| 扩展供应链 | 恶意/篡改扩展获得 `process:exec` 等能力 |
| 编辑器进程完整性 | 进程内 V8/WASM escape、类型混淆导致宿主内存破坏 |

### 1.2 攻击者与场景

1. **提示注入**：不可信 PR / `AGENTS.md` / 网页内容诱导 Agent 外泄密钥或扩大权限（Zed 博客已说明「只靠指令不够」）。
2. **命令规则绕过**：`git .*` 类 allow/deny 可被 `bash -c`、解释器、自写包装脚本绕过；**不能替代 OS 沙箱**。
3. **Symlink TOCTOU**：用户批准路径后、挂载进沙箱前，路径被换成指向敏感目录的 symlink（`crates/sandbox` 以 fail-closed 为目标；设计要求任何新路径绑定保持同等强度）。
4. **沙箱外副作用**：`edit` 工具、LSP/proc macro、普通终端、MCP、git submodule hooks——沙箱不覆盖；`ultra` 档规划额外收紧（见 §8）。
5. **恶意扩展 / 未签名包**：无验签时 marketplace 或本地 tar 可携带任意能力声明。
6. **恶意 Workflow 脚本**：模型生成的编排脚本若有 FS/shell，等同于在用户会话里跑不可信代码；且可放大子代理数量与费用。
7. **JIT / 类型混淆**：优化码与 deopt 路径若信任错误类型标签，可破坏 VM 不变量并绕过宿主检查（契约见 `perlicascript/docs/jit-security.md`）。

### 1.3 非目标（本设计不承诺）

- 解决「Agent 总能正确判断用户意图」。
- 在 DeltaDB 未开源前自研 CRDT worktree。
- 用提示词替代 OS 强制隔离。
- 对 External Agents（ACP）提供与内置 Agent 完全相同的 OS 沙箱（除非后续单独立项）。

---

## 2. 分层防御

```text
┌─────────────────────────────────────────────────────────────┐
│  security.level  （单一选择 → 四组策略包 / custom 旋钮）      │
├─────────────────────────────────────────────────────────────┤
│  策略层  tool_permissions · Agent Profiles · Worktree Trust │
├─────────────────────────────────────────────────────────────┤
│  强制层  Seatbelt / bwrap+seccomp / WSL · http_proxy        │
├─────────────────────────────────────────────────────────────┤
│  扩展层  WASM+WASI · CapabilityGranter · 签名与发布者信任   │
│         （未来：Deno/Bun/Perlica 同一闸门，零 ambient I/O）   │
├─────────────────────────────────────────────────────────────┤
│  编排层  Workflow 脚本仅编排；agent()/pipeline() 受限 ops    │
└─────────────────────────────────────────────────────────────┘
```

| 层 | 现状（2026-08） | 目标 |
|----|-----------------|------|
| 策略 | `agent.tool_permissions`、profiles、worktree trust | 由 `security.level` 设默认，允许 custom 覆盖 |
| 强制 | `terminal`/`fetch` OS 沙箱；`.git` 保护；升级需确认 | 保持；补强子代理升权隔离与等级绑定 |
| 扩展 | WASM 能力双闸门；**无签名信任** | 按等级验签；JS/Perlica 复用 granter |
| 编排 | 子代理/并行线程有；**无可恢复脚本 Workflow** | 脚本无 I/O；预算/审批/journal |

### 2.1 明确不覆盖（残留风险）

与 [`docs/src/ai/sandboxing.md`](../../../docs/src/ai/sandboxing.md) 一致，默认沙箱**不**保护：

- 文件 `edit` 等非 terminal/fetch 工具
- LSP、MCP、Tasks、普通终端标签页
- External Agents、Terminal Threads
- Windows 非 WSL shell

设计要求：文档与 UI 在每一安全等级下如实展示这些缺口；`ultra` 用额外策略收紧侧信道，而不是假装沙箱已覆盖。

---

## 3. 统一设置：`security.level`

### 3.1 Schema（设计）

```json
{
  "security": {
    "level": "medium",
    "custom": {
      "sandbox": {},
      "extension_signing": {},
      "embedded_js": {},
      "workflow": {}
    }
  }
}
```

- `level`：`none` | `low` | `medium` | `high` | `extreme` | `ultra` | `custom`
- 非 `custom` 时忽略或只读展示 `custom.*`（实现期再定是否持久化覆盖）
- `medium` 为**建议默认**（平衡自动化与供应链/沙箱基线）
- 与现有 `agent.sandbox_permissions` / `agent.tool_permissions` 关系：等级写入**默认包**；用户在设置 UI 中的显式覆盖在 `custom` 或「从当前等级钉住覆盖」流程中生效（实现期：避免静默互相打架，优先显式 `custom`）

### 3.2 等级 → 四组策略包

| 等级 | OS 沙箱 | 扩展签名 / 加载 | Agent Workflow | 嵌入式 JS（Deno/Bun） |
|------|---------|-----------------|----------------|------------------------|
| **none** | 关（`allow_unsandboxed` 等价） | 不验签；允许 dev | 不限（仅有硬上限防失控，见 §7） | 宽松；仍建议禁远程 import |
| **low** | 开；升级提示宽松 | 宽松（未签名 + dev 可装，警告） | 小预算；可选审批 | 禁远程 import；能力闸门开 |
| **medium** | Zed 默认：项目可写、`.git` 只读、默认无网；升级需理由 | **要求 Wuling DevOps 代码签名**；dev 需额外确认 | 中等预算；**运行前审批** | 零 ambient I/O；ops ∩ granter |
| **high** | 严格：少升级路径；网络/写路径默认更窄 | **仅** Zixiao Palace Laboratory Group 与 Zed Industries Inc 签名 | 严格预算；**强制审批**；子代理不可自行 unsandboxed | 最小 ops 集；堆/时间硬上限 |
| **extreme** | 几乎不可升级；**禁 unsandboxed** | 同 high + **禁 dev 扩展** | 极小 fan-out；强制审批 | 几乎无 I/O ops（只读宿主 API） |
| **ultra** | 同 extreme | 白名单签名 + **能力默认全拒**（逐项授予） | **禁用**或只读编排（不可 spawn 写工具） | **禁用**嵌入式 JS 扩展 |
| **custom** | 旋钮独立 | 旋钮独立 | 旋钮独立 | 旋钮独立 |

### 3.3 `custom` 旋钮清单（设计）

**sandbox**

- `enabled: bool`
- `allow_unsandboxed: bool`
- `allow_network_escalation: bool`
- `allow_write_escalation: bool`
- `protected_paths: string[]`（默认含 Git 元数据）
- `default_network: deny | allowlist`
- `host_allowlist: string[]`

**extension_signing**

- `mode: off | warn_unsigned | require_wuling | allowlist_publishers`
- `allowed_publishers: string[]`（默认建议含 `Zixiao Palace Laboratory Group`、`Zed Industries Inc`）
- `allow_dev_extensions: bool`
- `require_capability_prompt_on_install: bool`

**embedded_js**

- `runtime: disabled | deno_core | bun_embed`（Bun 仅当实现满足零 ambient）
- `allow_remote_import: bool`（默认 `false`）
- `max_heap_bytes` / `max_wall_ms` / `ops_allowlist`

**workflow**

- `enabled: bool`
- `require_plan_approval: bool`
- `max_concurrent_agents` / `max_total_agents`
- `max_depth` / `max_tokens` / `max_wall_ms`
- `size_guideline: unrestricted | small | medium | large`
- `subagent_may_request_unsandboxed: bool`（`high+` 预设为 `false`）

### 3.4 与现有 UI 的关系

- Settings 增加「安全性」页：主控件为等级枚举；展开显示四组摘要；`custom` 进入细项。
- Agent 线程 padlock / 沙箱 tooltip：显示有效等级与是否低于项目要求（若未来有项目级最低等级）。
- **不**删除现有 sandbox / tool permission 页；中长期可标注「由安全等级驱动」。

---

## 4. OS 沙箱：全套建议

对齐 [Zed Sandboxing](https://zed.dev/blog/sandboxing) 与 `crates/sandbox` 实现。

### 4.1 必须保留的行为

| 规则 | 约束 |
|------|------|
| 默认 FS | 全盘只读倾向 + 项目目录可写 + 隔离 temp |
| `.git` | 即使落在可写子树内也保持受保护；Agent **不得**请求写 `.git`（防 hooks 逃逸） |
| 网络 | 默认拒绝；经宿主 `http_proxy` + host allowlist |
| 升级 | 展示权限与理由；一次 / 本线程 / 永久；fail-closed |
| TOCTOU | 路径检查与进入沙箱之间的 symlink 交换必须失败关闭，不得挂载攻击者目标 |
| 平台 | macOS Seatbelt；Linux bwrap +（已有）FD/身份校验 + seccomp；Windows 经 WSL |

### 4.2 设计补强（相对现状）

1. **子代理升权隔离**：子线程默认继承父线程沙箱上限，**不得**高于父；父未授予的 host/write 子不得静默获得。Workflow 调度的子代理额外禁止自行请求 `unsandboxed`（由等级控制，见 §3.3）。
2. **升级不可暗含 `.git`**：任何「写任意路径」批准必须在策略层剥离 Git 元数据写权限。
3. **`create_directory` 与沙箱权限流对齐**：临时创建失败清理的语义保持；批准前不得留下可被 TOCTOU 利用的可写跳板。
4. **并行线程**：同一用户批准窗口内，路径身份绑定（Linux FD 校验等）必须覆盖所有并发构造 sandbox 的调用方。
5. **可观测性**：每次拒绝/升级记录结构化原因，便于用户审计（不依赖模型自述）。

### 4.3 侧信道缓解路线图（单靠沙箱不够）

| 侧信道 | 缓解方向 | 建议档位 |
|--------|----------|----------|
| proc macro / `build.rs` | 不信任工作区时禁用会执行代码的 LSP 能力；Restricted Mode 联动 | `ultra` 默认收紧 |
| 恶意 Tasks / 普通终端 | 不自动运行不可信任务；UI 区分 sandboxed agent 终端 vs 用户终端 | `extreme+` 提示，`ultra` 限制自动任务 |
| git hooks / submodule | 保持 `.git` 只读；提交前钩子警告 | 全档文档化；`ultra` 可提示禁用 hooks |
| MCP / 外部 Agent | 独立信任与工具允许列表；不假装已沙箱 | `high+` 默认更严的 MCP 安装 |
| Windows 非 WSL | 无沙箱则 UI 强制降级提示或拒绝 `high+` 的「已保护」宣称 | 全档诚实展示 |

---

## 5. 嵌入式 Bun / Deno JS 能力限制（设计预留）

> 实现尚未落地。本节是未来扩展运行时的安全契约；细节前置见 [四大特性调研 §2](./extension-runtimes-and-agent-workflows.md)。

### 5.1 选型硬约束

| 项 | 约束 |
|----|------|
| Deno | 嵌入 **`deno_core::JsRuntime`**，**不用**完整 `deno_runtime`（避免 ambient `Deno.*` I/O） |
| Bun | 若嵌入：同样 **默认零能力 + 显式 ops**；禁止「整包 Bun 权限模型」直通 |
| 模块加载 | 默认 **禁止远程 import**；`medium+` 仅允许签名/锁定源（若将来开放） |
| 隔离 | 一扩展一 isolate + 堆上限 + 执行时间/epoch 中断；`high+` **可要求**进程级隔离（分期） |
| 能力 | 一切危险宿主调用经现有 **`CapabilityGranter`**（manifest 声明 ∩ 宿主授予） |
| 注册 | 经 `ExtensionHostProxy`；按 `ExtensionLibraryKind` dispatch，不另造旁路加载器 |

### 5.2 按 `security.level` 裁剪 ops

| 等级 | JS 扩展 | 典型允许 |
|------|---------|----------|
| none / low | 可启用 | 较宽 ops；仍无 ambient FS/net |
| medium | 可启用 | LSP/主题等只读注册 + 经授权的 exec/download |
| high | 可启用 | 最小 ops；默认无 `process:exec` |
| extreme | 可启用 | 几乎无 I/O；无 exec/download |
| ultra | **禁用** | — |

### 5.3 明确非边界

V8 isolate **不是**抗恶意代码的硬安全边界。市场审核扩展可暂用 isolate；完全不可信第三方在 `high+` 路线图中走向进程隔离。JIT/优化若引入，不得绕过 tag/capability 检查（Perlica 见兄弟仓文档；JS 侧同理：宿主边界强制校验）。

---

## 6. 插件 / 扩展签名与加载

### 6.1 信任锚

1. **Wuling DevOps 代码签名**：发行/分发流水线签名；编辑器安装时验签（fail-closed）。
2. **发布者白名单**（`high` / `extreme` / `ultra`）：
   - Zixiao Palace Laboratory Group
   - Zed Industries Inc
3. **能力闸门**：签名只解决「谁发布」；「能做什么」仍由 `ExtensionCapability` + 用户/等级授予决定。

### 6.2 按等级的加载策略

| 等级 | 签名策略 | Dev Extension（Install Dev Extension） |
|------|----------|----------------------------------------|
| none | 不验签 | 允许 |
| low | 未签名可装，UI 警告 | 允许 |
| medium | **必须**通过 Wuling DevOps 验签 | 允许但每次确认 |
| high | 验签 + 发布者 ∈ 白名单 | **禁止**（除非日后 `custom`） |
| extreme | 同 high | **禁止** |
| ultra | 同 high + 能力默认全拒 | **禁止** |
| custom | `extension_signing.mode` 等 | `allow_dev_extensions` |

### 6.3 安装时强制流程

1. 解析 manifest → 验签 → 校验发布者（若需要）→ 展示能力声明 →（按等级）用户确认 → 写入授予集。
2. 验签失败、发布者不在白名单、或能力未被授予：**拒绝加载**，不得半加载执行。
3. WASM / 未来 Deno / Bun / Perlica **同一信任与能力管线**；禁止「JS 扩展跳过验签」。

### 6.4 与 WASM 现状对齐

现有：work_dir 限定、`CapabilityGranter`、epoch 中断。缺口（应在实现签名时一并考虑）：无内存上限、能力粒度粗、无安装时逐项 UI——见调研文档 §1.3；`ultra` 的「能力默认全拒」依赖安装时知情同意 UI。

---

## 7. Agent Workflow 能力限制

形态对齐 Claude Code Dynamic Workflows（脚本编排子代理、可恢复、会话内进度），约束按本仓库威胁模型收紧。控制面设计承接调研 §3：**不自研 DeltaDB/CRDT**。

### 7.1 脚本与运行时边界

| 约束 | 为什么 |
|------|--------|
| 脚本**无**直接文件系统 / shell / 网络 | 脚本只协调；副作用只经子代理工具 |
| 仅暴露编排 ops（如 `agent`、`pipeline`、预算/取消） | 缩小攻击面 |
| 子代理权限 ≤ 父会话，且受 `security.level` 裁剪 | 防止脚本放大 YOLO |
| 可恢复 effect journal（控制面状态，非代码 CRDT） | 崩溃恢复；in-flight 标 `unknown`，不盲目重跑 |
| 用户取消传播到脚本与全部子任务 | 费用与失控控制 |

### 7.2 运行时上限（设计默认；等级可下调）

参考 Claude Code 量级（约 16 并发 / 1000 总代理），本设计按等级给出**上限包**（实现可微调数值，不可取消硬上限）：

| 等级 | 并发代理 | 总代理/次 | size guideline | 运行前审批 | 子代理 `unsandboxed` |
|------|----------|-----------|----------------|------------|----------------------|
| none | 16 | 1000 | unrestricted | 否 | 允许（仍受工具权限） |
| low | 8 | 200 | small | 可选 | 允许请求 |
| medium | 8 | 100 | medium | **是** | 允许请求，需用户批 |
| high | 4 | 50 | small | **是** | **否** |
| extreme | 2 | 15 | small | **是** | **否** |
| ultra | 0 或只读 | 0 | — | — | —（Workflow 禁用或只读） |
| custom | 旋钮 | 旋钮 | 旋钮 | 旋钮 | 旋钮 |

另限：深度、token/费用、wall-clock、单代理输出大小；超出 fail-closed 并停止调度。

### 7.3 审批与权限模式

- 运行前展示：阶段列表、预估代理数/token、workspace 目标、写作用域、模型。
- 选项语义对齐业界：运行一次 / 本项目不再询问该 Workflow / 查看脚本 / 拒绝。
- Workflow 产生的子代理：固定受限模式（建议 `acceptEdits` + 继承工具允许列表 + **有效沙箱策略**）；shell/网络/MCP 仍可按工具权限弹窗。
- `claude -p` / 无头等价路径：无交互时严格按预配置允许列表，不得隐式放宽等级。

### 7.4 Large workflow 警告

- 超过指南代理数或预计 token 阈值时标记 `Large workflow`（建议性，不自动停，除非等级为 `extreme+` 且策略设为硬拒绝）。
- `size_guideline` 与 §7.2 映射；`none` 不展示警告亦可。

### 7.5 与并行 Agent / worktree

- 复用现有 spawn / create_thread / Git linked worktree；Workflow 只加协调与预算。
- 写集不相交由调度策略 + 审批声明约束；不把「无文本冲突」当成正确性。

---

## 8. PerlicaScript / JIT（交叉引用）

扩展或 Workflow 若以 PerlicaScript 为脚本前端：

- 宿主 ABI 必须 capability-token 化；异步经 channel broker，不持有 GPUI `App`。
- JIT（FE 基线 + SuperCharger）安全契约见兄弟仓：  
  [`perlicascript/docs/jit-security.md`](../../../../perlicascript/docs/jit-security.md)
- 按等级：`high+` 可禁 SuperCharger；`ultra` 可禁 JIT，仅解释执行。

---

## 9. 现状 vs 目标差距

| 能力 | 现状 | 目标 |
|------|------|------|
| Agent terminal/fetch OS 沙箱 | 有 | 保持 + 等级绑定 + 子代理升权隔离 |
| 统一 `security.level` | **无** | 有 |
| 扩展代码签名 | **无** | Wuling / 白名单按等级 |
| 嵌入式 Deno/Bun | **无** | 零 ambient + granter |
| 脚本 Workflow 控制面 | **无** | 受限 ops + 预算 + 审批 + journal |
| Perlica JIT | **无**（解释器） | 见 jit-security 契约 |
| ultra 侧信道收紧 | 部分（worktree trust） | 设计分期落地 |

---

## 10. 分期路线图（仅规划，本任务不实现）

| 阶段 | 内容 |
|------|------|
| **P0** | `security.level` settings schema + UI 摘要；映射到现有 sandbox/tool_permissions 默认值；扩展签名策略面（验签钩子 + 发布者白名单配置） |
| **P1** | Workflow 控制面：coordinator、预算、审批、journal、无 I/O 脚本宿主 |
| **P2** | `deno_core`（及可选 Bun）扩展运行时 + 按等级 ops；Perlica host ABI |
| **P3** | `ultra` 侧信道：LSP/任务/MCP 更严默认；可选 JS 进程隔离；Perlica JIT 按 jit-security 落地 |

---

## 11. 验收标准（设计级）

实现某一阶段时，至少满足：

1. 将 `security.level` 从 `none` 调到 `high`，无需手改多处 JSON 即可收紧沙箱升级、签名与 Workflow 预算。
2. `medium` 下未签名扩展 fail-closed；`high` 下非白名单发布者 fail-closed。
3. Workflow 脚本无法直接 `open`/`fetch`/`exec`；超预算停止调度。
4. 沙箱 TOCTOU 与 `.git` 保护回归测试保持 fail-closed。
5. 文档/UI 不声称沙箱覆盖 edit/LSP/普通终端。

---

## 12. 相关路径速查

| 路径 | 角色 |
|------|------|
| `crates/sandbox/` | OS 沙箱实现 |
| `crates/agent/src/sandboxing.rs` | Agent 胶水 |
| `crates/agent/src/tool_permissions.rs` | 工具权限 |
| `crates/http_proxy/` | 出站代理与 host 过滤 |
| `crates/extension/src/capabilities.rs` | 扩展能力枚举 |
| `crates/extension_host/src/capability_granter.rs` | 能力双闸门 |
| `crates/settings_ui/src/pages/sandbox_settings.rs` | 沙箱设置 UI |
| `crates/ama10/` | Wuling DevOps 客户端（签名/发行信任锚候选） |
