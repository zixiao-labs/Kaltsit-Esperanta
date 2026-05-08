# Kal'tsit·Esperanta Dev Plan

> Kal'tsit·Esperanta 基于 [Zed](https://github.com/zed-industries/zed) 二次开发,由 [Zixiao Labs](https://github.com/zixiao-labs) 维护。
> 本文件追踪在上游 Zed 之上 Zixiao Labs 计划做的几项分阶段改动。

Status: ✅ Done · 🚧 In progress · ⬜ Planned

## Stage 1 · Localization & Branding ⬜

**Goal:** 让 Esperanta 成为面向中文用户的可用编辑器,并完成 Kal'tsit 视觉识别。

- ⬜ 翻译 UI:命令面板、设置、各侧边面板(项目、大纲、终端、AI、Git)
- ⬜ 翻译文档站(`docs/src/**`)
- ⬜ 加入 **Kal'tsit** 主题(深色 + 浅色一对),灵感来自《明日方舟》凯尔希
- ⬜ 替换应用图标 / wordmark 为 Esperanta 新 logo

**Done when:** `zh-CN` 覆盖 ≥ 95% 的用户可见字符串,且 Kal'tsit 主题能从 `theme:` 设置中无警告选中。

## Stage 2 · Auth Migration ⬜

**Goal:** 用 [Wuling DevOps](https://github.com/zixiao-labs/Wuling-DevOps) (Zixiao Labs OAuth) 替换 Zed 自托管账号 / SSO。

- ⬜ 把 Zed 内置账号流程抽象成可替换的 Provider
- ⬜ 实现针对 Wuling DevOps 的 OAuth 2.0 / OIDC 客户端
- ⬜ 迁移 session 与 refresh-token 的存储与刷新
- ⬜ 在 Esperanta 中以侧边面板形式嵌入 Wuling DevOps 控制台

**Done when:** 一台全新安装的 Esperanta 能完整走通"用 Wuling DevOps 账号登录",嵌入面板正确反映已登录态。

**Depends on:** Wuling DevOps 的认证 API 达到可用状态(目前 WIP)。

## Stage 3 · Self-hosted Collab ⬜

**Goal:** 把实时协作切到 Wuling DevOps,~~**不动** Zed 的 CRDT。~~

~~- ⬜ **保留** 现有 CRDT 与冲突解决逻辑,不做改动~~
- ⬜ 用 Wuling DevOps 作为协作认证服务器(依赖 Stage 2)
- ⬜ 完整保留协作能力:共享编辑、对端终端、对端文件系统、对端 Debug、对端 Run、AI 助手
- ⬜ 把 WebRTC 信令重定向到自托管端点(音视频栈本身不动)

**Done when:** 两台不同网络下的 Esperanta 能通过自托管的 Wuling DevOps 实例加入同一个共享项目,链路上不再依赖 Zed Industries 的任何基础设施。

**Depends on:** Stage 2。

~~**Non-goals:** 重写 CRDT、改协作 UX、替换 WebRTC 实现。~~ 协作是AGPL，要重造一遍，出BUG再说

---

## What's not changing

- Rust 内核、GPUI、编辑器内部实现持续与上游 Zed 对齐;我们会定期 rebase
- CRDT 与音视频栈不在本 fork 的改造范围内
- 我们不计划对外运营公共协作服务 —— Stage 3 的预期是用户自行部署 Wuling DevOps（除非Wuling DevOps出SaaS版，但是更偏向非商业SaaS，仅有可选的赞助）
