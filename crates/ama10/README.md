# Ama10

凯尔希的代号

防止产生严重合并冲突，所以我们的大多数修改都将放在此处（包括武陵DevOps）

谁曾见过，钢铁与岩石堆积的躯壳
它喝令不知真相的人们
交出自己的恐惧，交出发抖的嘴唇
交出血肉构筑的、磨损的容器，沤制孕生怪物的腐壤
但是，绞索与谎言，如何能褫夺夜雾中的火光？
留下一缕卑微的呼吸
与晚风一同，细数残垣上的野草

---

## TODO

> 同步自仓库根 [`ROADMAP.md`](../../ROADMAP.md)。状态：✅ Done · 🚧 In progress · ⬜ Planned

### Stage 1 · Localization & Branding 🚧

- ⬜ 翻译 UI：命令面板、设置、各侧边面板（项目、大纲、终端、AI、Git）
- ⬜ 翻译文档站（`docs/src/**`）
- 🚧 加入 **Kal'tsit** 主题（深色 + 浅色一对）
  - ✅ 深色版 `assets/themes/kaltsit/kaltsit-dark.json`
  - ⬜ 浅色版 `kaltsit-light.json`
- 🚧 替换应用品牌资源
  - ⬜ 菜单栏首项 "Zed" → "Esperanta"（macOS app name、"About Zed"、"Quit Zed" 等用户可见文案）
  - ⬜ Abort 对话框 / 崩溃报告中残留的 "Zed" 字样
  - ⬜ App Icon（`crates/zed/resources/app-icon-*.png`、`.icns`）
  - ⬜ wordmark / logo 资源（启动 splash、关于页等）

**完成标准**：`zh-CN` 覆盖 ≥ 95% 的用户可见字符串；Kal'tsit 主题深/浅可用；资源全部换成 Esperanta 视觉。

### Stage 2 · Connector Accounts 🚧

- ✅ 以 `ConnectorId`、`ConnectorAccount` 和统一账号状态抽象 Wuling DevOps / GitHub
- ✅ 实现 Wuling DevOps 与 GitHub OAuth Device Flow，并通过平台 `CredentialsProvider` 存储令牌
- ✅ 把服务器 URL、GitHub Client ID 和标题栏头像来源收敛到 Connectors 设置页
- ✅ 从账号菜单连接、打开或断开各连接器
- ✅ 同步 Wuling OpenAPI，并只生成认证所需的轻量 serde 类型

**阻塞依赖**：Wuling DevOps 认证 API 达到可用状态（目前 Stage 1 WIP，详见 [issue tracker](https://github.com/zixiao-labs/Wuling-DevOps/issues)）。

### Stage 3 · Self-hosted Collab ⬜

- ⬜ 保留现有 CRDT 与冲突解决逻辑（不动）
- ⬜ 用 Wuling DevOps 作为协作认证服务器（依赖 Stage 2）
- ⬜ 完整保留协作能力：共享编辑、对端终端、对端文件系统、对端 Debug、对端 Run、AI 助手
- ⬜ 把 WebRTC 信令重定向到自托管端点（音视频栈不动）

**Non-goals**：重写 CRDT、改协作 UX、替换 WebRTC 实现。

---

## Wuling 客户端再生流程

1. 安装 PyYAML：`python3 -m pip install PyYAML`
2. 保持 `Wuling-DevOps` 仓库与本仓 sibling，或 `export WULING_OPENAPI_PATH=/path/to/openapi.yaml`
3. 跑 `script/regen-wuling-api.sh`
4. review `api/wuling-openapi.yaml` 和 `api/wuling-client-types.json` 的 diff
5. `./script/clippy -p ama10`

脚本会原样同步 OpenAPI 约定，并从认证相关 schema 投影出一个小型 JSON Schema。`typify::import_types!` 在编译时生成 serde 类型；HTTP 请求由手写的轻量 `reqwest` 客户端完成，不再检入大型生成代码。

## GitHub OAuth App

1. 在 GitHub 的 **Settings → Developer settings → OAuth Apps** 创建 OAuth App。
2. Homepage URL 填项目主页；表单要求的 callback URL 可填 `http://127.0.0.1`，设备流程不会使用回调。
3. 创建后在应用设置中启用 **Device Flow**。
4. 把 Client ID 填入 Esperanta 的 **Settings → Connectors → GitHub → OAuth App Client ID**，再从用户菜单的 **Connectors → Connect GitHub** 登录。

设备流程只使用公开的 Client ID，不应把 Client Secret 填入设置或提交到仓库。详见 [GitHub OAuth App 授权文档](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)。

## Notes

- `ama10::connector` 不依赖 GPUI，连接器账号模型可被上游或其他平台复用。
- Wuling 和 GitHub 客户端把平台能力限制在 `CredentialsProvider` 与 Tokio handle 的构造边界，网络协议和 serde 类型位于 `ama10`。


## 为啥不贡献回上游

因为上游已经明确[Out of Scope](https://github.com/zed-industries/zed/pull/53719)

暂时不尝试贡献，但是新的插件系统和脚本语言可以再试一次Feature Request（不指望能被上游Accepted Proposal以及可以开cherry pick pr，纯工作量证明，因为不证明工作量可能会导致我们失去工作并被Claude Code CLI取代），bug修复一直可以cherry pick

## 许可证

- 手写的 Rust 代码（`src/`、`Cargo.toml`、本 README 等）：**MIT**。
- `api/wuling-openapi.yaml` 同步自 [Wuling DevOps](https://github.com/zixiao-labs/Wuling-DevOps)；`api/wuling-client-types.json` 派生自该 spec，**Apache-2.0**。上游的 LICENSE / NOTICE 副本放在 `api/LICENSE-APACHE` 与 `api/NOTICE`，按 Apache License §4(d) 保留。
