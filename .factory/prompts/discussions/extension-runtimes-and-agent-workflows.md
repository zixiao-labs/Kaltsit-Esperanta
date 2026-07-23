# 四大特性前置需求调研：JS/TS 扩展、CEF 绑定、前端增强模式、Agent 编排工作流

> 状态：调研（前置需求盘点），非实现计划。快照日期：2026-07-11。基于对 `Kaltsit-Esperanta`、`perlicascript` 的一手代码勘察及 Zed 的公开资料；其他兄弟项目仍待单独补充。
> 核心约束（用户明确）：**安全性第一，绝不重蹈 VS Code 覆辙**（所有扩展同进程、满权限、无能力系统）；PerlicaScript 定位为**通用语言**（类比 Node.js 里的 JS），而非仅 DSL。

---

## 0. 一句话结论

四个特性中，**特性 4（Agent 编排）已有可用的子代理、并行线程和 Git linked worktree 原语，但“可恢复的脚本工作流”本身仍是早期设计**；尤其不能把 CRDT 工作树当作本项目应自行补齐的底座，因为这与 Zed 尚未开源的 DeltaDB 核心范围直接重叠。**特性 1（Deno JS/TS 扩展）路径较清晰但仍需解决进程隔离与 GPUI 跨线程桥接**，**特性 2/3（CEF + 前端增强）仅 macOS 有现成渲染入口、跨平台缺口最大**。当前未跟踪的两个骨架 crate（`extension_cef`、`extension-perlicascript`）仍未接入 workspace；其中 `extension-perlicascript` 还引用了主仓不存在的 `LibKind::PerlicaScript` 和 PerlicaScript 已删除的旧 API。PerlicaScript 在 2026-07-10 的首个可运行版本中已把 VM 改为 `Send + Sync`，因此“VM 非 `Send`”不再是当前事实，但语言/宿主接口与资源治理仍远未满足扩展或工作流要求。

---

## 1. 现有扩展系统架构（安全模型基线 —— 必须复刻）

Zed 现行 WASM 扩展系统**已经是**一个能力式（capability-based）沙箱，这正是「反 VS Code」的正确范式。新增的任何运行时都必须对齐它，而非绕过它。

### 1.1 关键类型与位置

| 类型 | 位置 | 职责 |
|---|---|---|
| `trait Extension` | `crates/extension/src/extension.rs:50` | 扩展能力契约（~25 个 async 方法：LSP、slash command、context server、DAP、docs 索引） |
| `ExtensionManifest` | `crates/extension/src/extension_manifest.rs:82` | `extension.toml` 反序列化结构 |
| `enum ExtensionLibraryKind` | `extension_manifest.rs:312` | **目前只有 `Rust` 一个变体** |
| `enum ExtensionCapability` | `crates/extension/src/capabilities.rs:14` | `process:exec` / `download_file` / `npm:install` 三类能力 |
| `ExtensionHostProxy` | `crates/extension/src/extension_host_proxy.rs:26` | **功能注册中枢 seam**（8 个 proxy trait） |
| `WasmHost` / `WasmExtension` | `crates/extension_host/src/wasm_host.rs:48,63` | wasmtime 实例化与调用桥 |
| `CapabilityGranter` | `crates/extension_host/src/capability_granter.rs:7` | 运行时能力校验双闸门 |

### 1.2 沙箱强制点（安全模型精髓）

WASM 运行时的隔离由 wasmtime + WASI + 能力校验三层构成：

1. **文件系统限制**：`build_wasi_ctx`（`wasm_host.rs:729`）只 `preopened_dir` 扩展自己的 work_dir。扩展看不到任意路径。（注意：目录内是 `FilePerms::all()`，即对自己的目录有全读写。）
2. **路径逃逸防御**：`writeable_path_from_extension`（`wasm_host.rs:753`）canonicalize 后校验 `starts_with(work_dir)`，防 `..` 与 symlink 逃逸。已有专门测试（`wasm_host.rs:1017`）。
3. **能力校验**：每个危险宿主函数入口强制调用 granter：
   - `process::run_command` → `grant_exec`（`since_v0_8_0.rs:896`）
   - `download_file` → `grant_download_file`（`since_v0_8_0.rs:1070`）
   - 双闸门：manifest 声明（`allow_exec`, `extension_manifest.rs:162`）**且** 宿主授予（`granted_capabilities`, `capability_granter.rs:28`）两者同时满足才放行。
4. **CPU 时间片**：`epoch_interruption(true)`（`wasm_host.rs:562`），后台每 100ms `increment_epoch`，防扩展在 `Future::poll` 里无限阻塞执行线程。
5. **执行线程模型**：编译在 GPUI background executor（`wasm_host.rs:649`，CPU 密集、不需 tokio）；实例化与消息循环在 tokio 线程（`wasm_host.rs:713-717`，因 wasmtime_wasi 的 I/O 依赖 tokio）；宿主回调需回主线程时走 `on_main_thread` channel（`wasm_host.rs:905`）。

### 1.3 已知安全缺口（新运行时不应继承）

- **无内存上限**：wasmtime `Store` 未设置 `StoreLimits`/`memory_size` 限制。恶意扩展可 OOM 整个编辑器。
- **能力粒度粗**：只有 exec/download/npm 三类；无「读某目录」「访问某网络域」等细粒度声明（`download_file` 的 URL 允许列表是唯一的网络粒度）。
- **无用户知情同意 UI**：`granted_capabilities` 来自 `ExtensionSettings` 全局配置（`wasm_host.rs:627`），非「安装时逐项授权」。

### 1.4 功能注册 seam（第二运行时的接入点）

`ExtensionHostProxy`（`extension_host_proxy.rs`）是所有功能向编辑器注册的唯一中枢，含 8 个 proxy：`theme` / `grammar` / `language` / `language_server` / `snippet` / `context_server` / `debug_adapter` / `language_model_provider`。**任何新运行时（Deno/Perlica）只要产出实现了 `trait Extension` 的对象，就能复用这套注册机制**，无需改动 UI/LSP/DAP 侧。

### 1.5 运行时 dispatch 点（关键改造位置）

`extension_host.rs:1496-1528`：加载时遍历扩展，`if extension.manifest.lib.kind.is_none() { continue }`，否则**无条件** `WasmExtension::load`。当前逻辑等价于「有 lib.kind ⟹ 必是 WASM」。第二运行时接入就在此处按 `lib.kind` 分派：

```
match manifest.lib.kind {
    Some(ExtensionLibraryKind::Rust)  => WasmExtension::load(...),   // 现状
    Some(ExtensionLibraryKind::Deno)  => DenoExtension::load(...),   // 待建
    Some(ExtensionLibraryKind::Perlica) => PerlicaExtension::load(...), // 待建（见 §4 风险）
    None => continue,
}
```

### 1.6 Dev Extension 安装流

`crates/extensions_ui/src/extensions_ui.rs` 提供 "Install Dev Extension" 入口；`extension_host.rs:1759` 起处理，默认为 `extension.wasm` 存在时 `get_or_insert(ExtensionLibraryKind::Rust)`（`:1764`）。新运行时需在此判定入口文件（如 `extension.js` / `extension.pscript`）并设定对应 kind。

---

## 2. 特性一：基于 Deno 的 JS/TS 扩展

### 2.1 现状

- **无任何 Deno/V8/QuickJS 依赖**：主仓库 tracked 代码中无 `deno_core`、`rusty_v8`、`quickjs` 引用。
- `LibKind` 无 JS 变体（`extension_manifest.rs:312` 只有 `Rust`）。
- 属于**从零新建**，但可完全复用 §1.4 注册 seam 与 §1.2 能力模型。

### 2.2 技术选型要点（deno_core，非 deno_runtime）

**版本与许可（2026-07 确证，crates.io）**：`deno_core` 最新 `0.407.0`（**MIT**，2026-07-08，约每周一版的高频节奏）；V8 绑定 crate 已从 `rusty_v8` 更名为 **`v8`**（最新 `149.4.0`，MIT；旧 `rusty_v8` 停更在 2021）；`deno_runtime` `0.262.0`、`deno_ast` `0.53.3`、`deno_permissions` `0.113.0` 均 MIT。全链路 MIT，无 copyleft 顾虑。注意：deno_core 已并入 `denoland/deno` monorepo 的 `core/` 目录维护。高频版本节奏意味着需锁定版本并定期跟进 breaking change。

- **嵌入层选 `deno_core::JsRuntime`，不用 `deno_runtime`**。`deno_core` 默认**无任何文件/网络能力**——嵌入者通过 `#[op2]` + `extension!` 宏定义**全部**能力面（已由 Context7 文档确证：`JsRuntime::new` 仅含你注入的 ops，无 ambient I/O）。这天然满足「无环境权限」的安全第一要求。`deno_runtime` 反而会带入完整 `Deno` 命名空间与进程级 `PermissionsContainer`，粒度偏进程级、不适合逐扩展隔离。
- **事件循环**：`run_event_loop` 需 tokio current-thread runtime 驱动。本仓库用 GPUI + smol，非 tokio——但 §1.2 已证明可行先例：WASM 扩展正是通过 `gpui_tokio::Tokio::spawn`（`wasm_host.rs:713`）在专用 tokio 线程上跑。Deno 运行时可套用同一模式（每扩展一个 isolate + 专用线程 + channel 桥）。
- **TS 转译**：`deno_core` 不原生转译 TS，需在 `ModuleLoader` 里用 `deno_ast`/swc 转译（Deno 自身即如此）。`ModuleLoader` 同时是**供应链控制点**——可在此禁止远程 import，杜绝「运行时拉取任意代码」。
- **资源上限**：V8 `create_params` 可设 isolate 堆上限；`terminate_execution` 可杀失控脚本。**这正好补上 §1.3 WASM 无内存上限的缺口**，新运行时应默认启用。
- **隔离模型**：一扩展一 isolate（v8 isolate 间强隔离）。但注意 —— V8 isolate **不是抗恶意代码的硬安全边界**（Chrome 靠站点隔离/进程隔离才达成，V8 sandbox escape 是现实威胁）。对完全不可信的第三方扩展，长期应考虑进程级隔离；对「审核过的市场扩展」，isolate 级足够。

### 2.3 前置需求清单（特性一）

1. `ExtensionLibraryKind` 增加 `Deno` 变体 + `extension.toml` schema 升版。
2. 新 crate `extension-deno`：`DenoExtension` 实现 `trait Extension`（把 25 个方法桥接到 JS 侧导出函数）。
3. 定义 `zed` 宿主 ops（`#[op2]`）：**必须走 CapabilityGranter**，复刻 §1.2 的 exec/download/fs 校验，不给 ambient 权限。
4. 每扩展一 isolate + tokio 线程 + epoch/堆上限。
5. `ModuleLoader` 禁远程 import（供应链）。
6. dispatch 点（§1.5）与 dev 安装（§1.6）分支。
7. 决策：isolate 级隔离 vs 进程级隔离（取决于扩展信任模型 —— 市场审核 or 完全开放）。

---

## 3. 特性四：Agent 编排工作流（主代理生成脚本编排子代理）

**已有执行原语，但尚不存在工作流控制面；工作树版本控制必须等待并适配 DeltaDB，而不是另造一套 CRDT。**

### 3.1 已有资产与真实边界

- `spawn_agent`（`crates/agent/src/tools/spawn_agent_tool.rs`）创建可等待、可凭 `session_id` 继续交互的子代理，最终只返回文本。`Thread::new_subagent` 复用父线程同一个 `Project`，因此并行写入发生在同一工作区；当前安全性主要依赖提示词要求“写集不重叠”，没有宿主级写集隔离。
- `create_thread`（`create_thread_tool.rs`，2026-06-01 合入上游）创建侧栏中独立持久线程，并支持 `use_new_worktree` 创建 Git linked worktree。它不返回 session ID 或最终结果，也不能由父工作流 join、取消或继续交互，因此不能直接充当 `agent()` worker。
- `SubagentHandle::send` 是一个可等待的 `Task<Result<String>>`，但句柄没有单分支 `cancel`、状态订阅、结构化结果或 workspace 选择接口；`MAX_SUBAGENT_DEPTH` 当前为 1。
- 父线程取消会传播到当前登记的运行中子代理，这是可复用的生命周期能力；尚无工作流级 fail-fast、部分取消或补偿语义。
- `AgentTool` 的 typed input/output 可用于设计工作流 op，但其 `replay` **只重建历史工具调用的 UI 事件，不重新执行工具，也不是 effect journal**（`Thread::replay_tool_call` 的注释明确说明 replay 不运行命令、不重新应用策略）。仓库里的 `journal` crate 是用户日记功能，与 workflow journaling 无关。
- 上游已经发布 Parallel Agents UI，故本项目不应重复实现线程侧栏、线程持久化或普通 Git worktree 管理；新工作仅应补可组合、可等待、可恢复的控制面。

### 3.2 DeltaDB 重叠边界

Zed 在 2026-06-11 的 [DeltaDB 公告](https://zed.dev/blog/introducing-deltadb) 中已经把以下能力划入上游核心：

- 以稳定身份记录每一个细粒度 delta，并版本化持续演化的 worktree；
- 将 agent 对话与其产生的编辑并排持久化和互相引用；
- 内嵌 conflict-free replicated worktrees，支持多人/多 agent 跨机器同时编辑；
- 将共享 worktree 挂载为真实文件供终端和外部工具使用；
- 与 Git 互操作，但不以 commit 作为协作边界。

因此本项目当前**不应实现** CRDT 文件树、delta 身份/操作日志、对话到代码的永久锚点、跨机器同步协议或虚拟文件系统挂载。这些并非可在以后轻易替换的“存储后端”，而是 DeltaDB 的领域模型本身；抢先实现会产生长期 rebase 冲突和数据迁移负担。上游代码尚未发布，只能把公告当作产品边界，不能假设内部 API、数据格式或开源时间表。

在 DeltaDB API 公开前，可以安全推进的是与工作树实现无关的控制面：

1. 定义 runtime-neutral 的 `WorkflowCoordinator`/状态机，管理节点依赖、并发、预算、取消与结果校验。
2. 让工作流只持有不透明的 `WorkspaceTarget`/`RevisionToken`，通过 adapter 请求工作区；不要让脚本或调度器认识 CRDT operation。
3. 初期 adapter 仅支持 `SharedCurrentProject`（要求静态不相交写集）和受限的 `GitLinkedWorktree`；DeltaDB 发布后新增官方 adapter，而非迁移自研 CRDT 数据。
4. effect journal 只记录 workflow 控制状态、调用 ID、输入/输出/错误、agent/model、预算和不透明 workspace revision；代码演化历史与对话—编辑锚点留给 DeltaDB。

### 3.3 不能由 CRDT 自动解决的问题

即使 DeltaDB 提供冲突自由的文本/文件树收敛，Agent Workflow 仍需处理语义冲突：两个 agent 可分别做出可合并但相互矛盾的 API 修改、数据库迁移、依赖锁文件或重命名；终端任务还会争用端口、构建目录、Git index 和外部服务。工作流需要显式的依赖 DAG、写作用域/ownership、验证关卡和人工批准，不能把“CRDT 无文本冲突”等同于“并行任务结果正确”。

当前 Git worktree fallback 也有明确限制：

- 新 worktree 基于 `HEAD`/指定 ref 的 clean checkout，不包含父工作区未提交修改；
- 非 Git roots 被原样加入新 workspace，仍与父线程共享，不是完整隔离；
- collab project 不支持创建 linked worktree；多个同仓 worktree 会被合并映射到一个新 worktree；
- 同一 workspace 的 worktree provisioning 有单一 in-flight guard，直接在 `parallel(...)` 中并发创建多个会失败，必须先串行 provision 再并发运行；
- 多仓库只做逐仓创建和失败回滚，没有跨仓库一致 revision。

### 3.4 脚本层仍缺的核心能力

1. **非确定性 effect journal**：禁用 `Date.now()`/随机并不能让 agent 调用可确定重放。agent 输出、工具副作用和工作区状态都非确定；恢复必须用稳定 invocation ID 返回已记录结果，并对崩溃时的 in-flight 调用标成 `unknown`/可人工决定，而不是盲目重跑。
2. **结构化结果协议**：`SubagentHandle` 只返回 `String`。需要 schema 校验、错误分类、重试策略和原始结果持久化，不能只借用 `AgentTool` 的 Rust 类型声明。
3. **层级预算**：同时限制 fan-out、深度、总 turns、模型 tokens/费用、wall-clock、终端资源和输出大小；当前只有深度上限，没有 workflow 总预算或子代理并发信号量。
4. **权限与审批**：`spawn_agent`/`create_thread` 本身不触发工具权限确认，权限由子线程内的每个工具调用处理。自动脚本会放大费用并产生并行授权弹窗，执行前应展示 agent/model、workspace、写作用域和预算，并支持一次性审批范围。
5. **生命周期**：需要 worker ID、状态流、单节点取消、fail-fast/collect-all、父线程关闭后的恢复以及 orphan 清理。现有 joinable 子代理和 isolated sibling thread 各只满足一半需求。
6. **线程桥**：`ThreadEnvironment` 以 `Rc<dyn ...>` 暴露并依赖 `App`/`AsyncApp`，不是可直接送进 Deno/后台 VM 的 `Send + Sync` 服务；`deno_core::OpState` 本身也为 `!Send + !Sync`。脚本运行时必须固定在线程上，并通过 request/response channel 回到 GPUI 前台 broker，不能把环境对象直接塞进 isolate。
7. **沙箱与停止**：除无 I/O 外，还要限制源码/模块大小、堆、同步执行时间、pending promises、日志和 agent 调用数，并确保用户取消可以终止脚本和所有子任务。

### 3.5 建议的最小顺序

1. 先扩充 agent 核心 seam：定义可等待且可取消的 worker handle、workspace target、结构化结果和持久化 workflow state。不能再假设“无需改动 agent 核心”。
2. 用 Rust 测试驱动状态机与少量 typed ops 验证 spawn/join/cancel/budget/recovery；暂不实现 CRDT 工作树，也不把调度语义绑定到 JS。
3. 再接脚本前端。Deno 与扩展运行时可复用低层 isolate/限额基础设施，但必须使用不同 capability profile 和生命周期；特性一不再是特性四的硬前置。
4. PerlicaScript 具备宿主调用、异步/取消和内存计量后，也可实现同一套 workflow op 协议。
5. DeltaDB 源码/API 发布后，只新增 workspace adapter 并验证 revision、mount、conversation linkage 与恢复语义。

---

## 4. PerlicaScript 运行时成熟度评估（影响「Perlica 作为扩展语言」可行性）

> 用户澄清：PerlicaScript 是**通用语言**（类比 Node 的 JS），不止 DSL。这抬高了对其运行时成熟度的要求。以下结论基于 `perlicascript` 2026-07-10 21:42 的 `56875d5 Make PerlicaScript runnable`；旧骨架所依赖的 GC、`VM`、`FunctionValue` 等 API 已不存在。

### 4.1 2026-07-10 版本已取得的进展

- 已打通「源码 → lexer/parser → bytecode `Program` → `Vm::run` → CLI/REPL」第一条垂直链路；文件、`--eval`、`--check` 和持久 globals 的 REPL 可运行。
- 当前 `Vm` 仅由 `Vec`/`HashMap`/owned `Value` 组成，测试明确断言 `Vm: Send + Sync`。此前“GC 使用 `Rc<RefCell<_>>` 导致 VM 非 Send”的阻塞已经消失。
- 已有每次 `run` 的 instruction 上限、value stack 上限和 globals 数量上限，并对非法常量索引、栈下溢、类型错误和除零返回 `RuntimeError`。
- 实测 `cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 README 示例均通过；现有测试为 compiler 4 个、runtime 6 个，共 10 个。

这证明它已是可运行的最小解释器，不再是占位工程；但“能运行表达式程序”和“能安全承载扩展/Agent Workflow”仍是两个相距很远的里程碑。

### 4.2 当前硬阻塞项

| 问题 | 当前证据 | 对扩展/Workflow 的影响 |
|---|---|---|
| **语言表达力不足** | 仅变量、赋值、标量、表达式和 `print`；README 明列控制流、函数、模块待实现，也没有对象/数组/异常 | 无法表达 `agent()`、`parallel()`、`pipeline()`、导出函数或真实扩展入口 |
| **无宿主调用 ABI** | runtime 不再有 `NativeFunction`/`define_global` 注入 API；`print` 是编译器 opcode | 无法把 Zed capability 或 agent op 注入脚本，未形成 capability boundary |
| **无 async/取消/yield** | `Vm::run(&Program)` 是同步 `for` 循环 | 无法 await 子代理或 I/O；未来加入循环后还需 cooperative cancellation，不能阻塞 GPUI 前台 |
| **资源限制不完整** | 只有 instruction/stack/globals 数量；无源码大小、总 heap/string、输出、wall-clock 或 session 累计预算 | 大字符串/超大源码仍可耗尽进程内存；多次 `run` 会重置 instruction 预算 |
| **失败非事务性** | `Vm` 跨 run 保留 globals；run 中途报错前已写入的 globals 不回滚 | REPL 尚可接受，工作流恢复会留下半应用状态；外部 agent/tool effect 更不可能靠 VM 回滚 |
| **无 effect 持久化** | `Program`/VM 状态没有稳定序列化格式或 invocation journal | 崩溃恢复、版本迁移、幂等重放尚无协议 |
| **外围仍是占位** | `languageserver` 与 `packagemanager` 仍只输出 `Hello, world!` | 通用语言所需诊断、模块解析、包供应链策略尚不存在 |
| **性能宣传未落地** | 无 Cranelift 依赖、JIT、GC 或 README 所称 RTTI | `extension-perlicascript/README.md` 描述的是愿景，不是当前实现；JIT 不是近期安全前置 |

### 4.3 `extension-perlicascript` 骨架已整体失配

- crate 未加入主 workspace；单独 `cargo check --manifest-path` 首先报“package believes it's in a workspace when it's not”。
- 两个 path dependency 写成 `../../perlicascript/...`，从 crate 目录解析到 `Kaltsit-Esperanta/perlicascript`，并非实际兄弟仓 `/Users/logos/WebstormProjects/perlicascript`。
- 源码仍引用已经删除的 `perlica_compiler::bytecode::BytecodeModule`、`types::TypeSystem`、`perlica_runtime::VM/gc/value`、native functions 和 `define_global`；当前 API 是根模块的 `Program`、`Vm`、`Value`。
- 它还引用不存在的 `extension::LibKind::PerlicaScript`/旧 manifest 形状，没有为当前约 25 个 `Extension` 方法提供实现，所谓 toast/open file/LSP 绑定目前也只是日志占位。
- 所以这不是修一个枚举即可恢复编译的 adapter，应在 Perlica host ABI 稳定后按当前 extension seam 重写；现在继续修骨架会造成双向追赶。

### 4.4 结论与建议（Perlica）

1. **短期**：Perlica 适合作为语言实现实验和受信任的同步表达式 PoC，不适合承载不可信第三方扩展，也尚不能执行 Agent Workflow。
2. 优先级应为：(a) 函数/控制流/集合/模块，(b) capability-token 化的宿主调用 ABI，(c) async host call + 取消，(d) heap/source/output/session 累计限额，(e) 模块加载与供应链策略，(f) parser/VM/CLI 集成、property/fuzz/adversarial 测试。JIT 应排在正确性和安全之后。
3. `Vm: Send + Sync` 只解决 VM 本体；GPUI 的 `ThreadEnvironment` 仍是 `Rc` 且依赖前台 `App`。Perlica 与 Deno 一样需要 channel broker，不能让后台 VM 直接持有 UI/agent handle。
4. Agent Workflow 应先稳定 runtime-neutral op 协议。届时 Perlica 和 Deno 都只是前端；无需为了等待 Perlica 阻塞控制面，也无需把 Deno 扩展系统设成 workflow 的硬前置。

---

## 5. 特性二 & 三：CEF 绑定 + 前端增强模式

### 5.1 GPUI 渲染入口现状

- **仅 macOS 有现成 surface 注入路径**：`gpui::surface(source)`（`crates/gpui/src/elements/surface.rs:33`）→ `SurfaceSource::Surface(CVPixelBuffer)`，仅 `#[cfg(target_os = "macos")]`。`window.paint_surface(bounds, CVPixelBuffer)`（`window.rs:4123`）走 Metal 合成。
- **最佳先例 = LiveKit 视频帧**：`remote_video_track_view.rs:81` 直接 `gpui::surface(latest_frame.clone())` 把解码帧作为元素渲染。**CEF 离屏渲染（OSR）产出的帧注入 GPUI 走的就是这条路** —— path A（OSR → CVPixelBuffer → surface 元素）在 macOS 上有完整可复制先例。
- **Windows/Linux 无对应 surface 路径**：`SurfaceSource` 无非 mac 变体。CEF path A 在这两平台需新建外部纹理导入（DXGI 共享句柄 / dmabuf → blade/DirectX 渲染器），是**最大的跨平台缺口**。

### 5.2 `extension_cef` 骨架现状

- `crates/extension_cef/src/lib.rs`：手写 C ABI FFI 绑定骨架（`CefBrowser`/`CefString`/`CefSettings` 等 opaque handle），README 明说走 `extern "C"` 动态链接、需系统预装 libcef。目前仅类型定义，**无实际渲染/事件桥接逻辑**。
- 未纳入 workspace members（主 `Cargo.toml` 无 `extension_cef`/`extension-perlicascript` 条目）。

### 5.3 前端增强模式（特性三）依赖链

「内嵌浏览器 + Agent 指点修改」需要三件事，均待建：

1. **渲染**：§5.1 的 CEF OSR → surface（macOS 可行，跨平台缺口）。
2. **输入合成（path A 必需）**：GPUI 元素事件（`MouseDownEvent`/key/scroll/IME）转发进 CEF 的 `SendMouseClickEvent`/`SendKeyEvent`。GPUI 事件数据是否足够仍待专门核实。
3. **元素→源码映射 + CDP**：CEF 暴露 Chrome DevTools Protocol（remote debugging port）；`Overlay.setInspectMode` / `DOM.getNodeForLocation` 让用户点选元素，配合 source annotation（类似 React DevTools `_debugSource` / vite-plugin-inspect）映射回 file:line 供 agent 编辑。这是特性三与 agent 系统的结合点，纯属新建。

### 5.4 前置需求清单（特性二/三）

1. 决策 CEF 集成路径：**path A（OSR 注入 surface，跨平台一致但需补 Win/Linux 纹理导入）** vs path B（原生子窗口叠加，各平台窗口句柄 + 事件路由，破坏 GPUI overlay 层叠）。macOS 先行建议 path A（有 LiveKit 先例）。
2. libcef 分发/打包策略（README 已定：动态链接、省编译时间与体积）。
3. GPUI → CEF 输入事件桥（需先确认 GPUI 事件字段完备度）。
4. 特性三额外需：CDP 客户端 + inspect 模式 + 元素定位 + source-map 注解 + 与 agent 工具链打通。
5. Windows/Linux 的 surface/纹理导入路径（GPUI 侧新增，工作量最大）。

---

## 6. 兄弟项目相关性（仍待单独调研）

- `opencode`：疑为 opencode AI coding agent，其插件系统（Bun/Node 运行时）与工具权限模型对特性一/四有借鉴价值 —— **待取回**。
- `logos`（zixiao-labs）：`closure-agent` README 点名的 TypeScript 版编排 agent 落地仓 —— 与特性四直接相关。
- `zhuque` / `Wuling-DevOps`：相关性待定。

---

## 7. 优先级与协同建议（供决策）

| 特性 | 底座成熟度 | 安全契合度 | 跨平台风险 | 建议次序 |
|---|---|---|---|---|
| 4a. Agent 执行原语 | ★★★★☆（spawn、并行线程、Git worktree 已有） | 中（权限在子线程逐项处理） | 低 | 直接复用，不重建 UI/线程系统 |
| 4b. Workflow 控制面/脚本层 | ★★☆☆☆（无 journal、worker/workspace 抽象不完整） | 待设计（费用、fan-out、恢复均是风险） | 低 | **先做 runtime-neutral PoC，禁止自研 CRDT worktree** |
| 1. Deno JS/TS 扩展 | ★★☆☆☆（从零但路径清晰） | 较高但非硬边界（无 ambient I/O，V8 仍在进程内） | 低 | 可独立做隔离/能力 PoC，不是 4 的硬前置 |
| 2. CEF 绑定 | ★☆☆☆☆（仅 FFI 骨架） | 中（浏览器进程隔离） | **高**（仅 mac 有 surface 入口） | 中期，macOS 先 PoC |
| 3. 前端增强 | ☆（纯新建，依赖 2） | 中 | 高 | 依赖 2 之后 |

**关键边界**：DeltaDB 已认领 operation history、conversation linkage、replicated worktree、跨机器同步和 mount。当前只定义 opaque workspace adapter 与 workflow control journal；不实现任何可演化成第二套 DeltaDB 的数据模型。

**关键协同**：特性 1 与 4 可以共享 isolate 启动、堆/时间限额、模块禁用和 GPUI channel broker 等低层设施，但不能共用运行时实例或 capability profile：扩展是长生命周期第三方代码，workflow 是短生命周期、由模型生成且能产生昂贵 agent effects 的程序。

**先决条件应分线处理**：

- 扩展运行时线：扩展 `ExtensionLibraryKind`、dispatch、dev install、能力授权与进程/isolate 隔离。
- Workflow 线：joinable/cancellable worker、workspace adapter、effect journal、结构化结果、层级预算与审批；不依赖 extension manifest。
- Perlica 线：先稳定语言与 host ABI，再重写 adapter；不要修补当前失配骨架。
- DeltaDB 线：等待公开源码/API 后做 adapter spike，并准备删除临时 Git fallback，而不是迁移自研 CRDT。

**建议近期只做两个可丢弃实验**：(1) Rust 状态机在共享只读/静态不相交写集下完成 spawn/join/cancel/recovery；(2) Deno 或 Perlica 的最小脚本前端只调用 fake host ops。两者通过明确协议连接，均不实现工作树存储。这样能验证 workflow 价值，又把与 DeltaDB 发生结构性冲突的部分留白。
