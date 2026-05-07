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

### Stage 2 · Wuling DevOps Auth Migration ⬜

- ✅ 引入 Wuling DevOps OpenAPI 客户端到 `ama10::wuling_api`
  （由 `script/regen-wuling-api.sh` 维护，spec 在 `crates/ama10/api/wuling-openapi.yaml`）
- ⬜ 把 Zed 内置账号流程抽象成可替换的 `AuthProvider`
- ⬜ 在 Provider 之上实现 Wuling DevOps 的 OAuth 2.0 / OIDC 客户端
- ⬜ 迁移 session / refresh-token 的存储与刷新
- ⬜ 在编辑器中以侧边面板形式嵌入 Wuling DevOps 控制台
- ⬜ 把 ama10 的 `reqwest` 0.13 与编辑器主体的 `zed-reqwest` 之间做适配 shim（生成的客户端目前走独立 reqwest，见下方 _Notes_）

**阻塞依赖**：Wuling DevOps 认证 API 达到可用状态（目前 Stage 1 WIP，详见 [issue tracker](https://github.com/zixiao-labs/Wuling-DevOps/issues)）。

### Stage 3 · Self-hosted Collab ⬜

- ⬜ 保留现有 CRDT 与冲突解决逻辑（不动）
- ⬜ 用 Wuling DevOps 作为协作认证服务器（依赖 Stage 2）
- ⬜ 完整保留协作能力：共享编辑、对端终端、对端文件系统、对端 Debug、对端 Run、AI 助手
- ⬜ 把 WebRTC 信令重定向到自托管端点（音视频栈不动）

**Non-goals**：重写 CRDT、改协作 UX、替换 WebRTC 实现。

---

## Wuling 客户端再生流程

1. 保持 `Wuling-DevOps` 仓库与本仓 sibling，或 `export WULING_OPENAPI_PATH=/path/to/openapi.yaml`
2. 跑 `script/regen-wuling-api.sh`
3. review `api/wuling-openapi.yaml` 和 `src/wuling_api/generated.rs` 的 diff
4. `./script/clippy -p ama10`

脚本会在生成前自动：
- 把 spec 头从 `openapi: 3.1.0` 改成 `3.0.3`（progenitor 只吃 3.0；spec 实际只用 3.0 的 `nullable: true` 语法）
- 给所有 operation 注入缺失的 `operationId`（形如 `get_orgs_by_org_slug_projects`）
- 跳过 `*.git/info/refs`、`*.git/git-upload-pack`、`*.git/git-receive-pack` 三个 Git smart-HTTP 端点（走 libgit2/`git`，不进 REST 客户端）

## Notes

- ama10 故意没有继承工作区的 `reqwest`（那是 `zed-reqwest` fork）。`progenitor-client` 是针对上游 `reqwest 0.13` 编译的，两边的 `Request`/`Error`/`HeaderValue` 是不同类型，混用会编译失败。Stage 2 集成进编辑器时再补一层 shim。
- 生成出来的客户端整个 `#![allow(clippy::all, ...)]`，review 时关注 `wuling_api.rs`（手写 shim 入口）和 `api/wuling-openapi.yaml`（spec diff），不必逐行看 `generated.rs`。
