# Kal'tsit·Esperanta

[![Zed](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/zed-industries/zed/main/assets/badge/v0.json)](https://zed.dev)

Kal'tsit·Esperanta is a high-performance, multiplayer code editor maintained by [Zixiao Labs](https://github.com/zixiao-labs) — **基于 [Zed](https://github.com/zed-industries/zed) 二次开发**.

It is a sister project to [Logos Editor Workstation](https://github.com/zixiao-labs/logos), Zixiao Labs' Electron-based editor; the two share design goals but target different stacks.

---

## What's different from Zed

- **Chinese-first localization** — UI, settings, and docs translated for native Chinese readers.
- **Kal'tsit theme** — a dedicated dark/light theme pair, inspired by *Arknights*' Kal'tsit.
- **Self-hosted auth** *(Production-Ready)* — swap Zed's hosted account for [Wuling DevOps](https://github.com/zixiao-labs/Wuling-DevOps) OAuth/SSO.
- **Self-hosted collab** *(planned)* — keep Zed's CRDT, but route auth, presence, and WebRTC signaling through your own Wuling DevOps instance.

See [dev-plan](./ROADMAP.md) for the full roadmap.

<<<<<<< HEAD
## Installation
=======
- Web ([tracking discussion](https://github.com/zed-industries/zed/discussions/26195))
>>>>>>> upstream/main

Esperanta does not yet ship pre-built binaries — please build from source:

- [Building for macOS](./docs/src/development/macos.md)
- [Building for Linux](./docs/src/development/linux.md)
- [Building for Windows](./docs/src/development/windows.md)

If you just want vanilla Zed, install it directly from [zed.dev/download](https://zed.dev/download).

## Contributing

This fork is maintained by Zixiao Labs — please file issues and PRs against this repo, not upstream Zed.

For changes that are not Esperanta-specific (general bug fixes, new editor features), please send them upstream first when possible; we periodically rebase on `zed-industries/zed` and will pick up your work that way.

The original [CONTRIBUTING.md](./CONTRIBUTING.md) (Zed's contribution conventions) applies here as well.

## Licensing

Esperanta inherits Zed's licensing — see [LICENSE-AGPL](./LICENSE-AGPL), [LICENSE-APACHE](./LICENSE-APACHE), and [LICENSE-GPL](./LICENSE-GPL). Original copyright remains with Zed Industries, Inc.

Zed source code is licensed primarily under GPL-3.0-or-later, with Apache-2.0 components where marked.

License information for third party dependencies must be correctly provided for CI to pass.

We use [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) to automatically comply with open source licenses. If CI is failing, check the following:

- Is it showing a `no license specified` error for a crate you've created? If so, add `publish = false` under `[package]` in your crate's Cargo.toml.
- Is the error `failed to satisfy license requirements` for a dependency? If so, first determine what license the project has and whether this system is sufficient to comply with this license's requirements. If you're unsure, ask a lawyer. Once you've verified that this system is acceptable add the license's SPDX identifier to the `accepted` array in `script/licenses/zed-licenses.toml`.
- Is `cargo-about` unable to find the license for a dependency? If so, add a clarification field at the end of `script/licenses/zed-licenses.toml`, as specified in the [cargo-about book](https://embarkstudios.github.io/cargo-about/cli/generate/config.html#crate-configuration).

## Sponsoring upstream Zed

Esperanta itself does not accept sponsorship. If you'd like to financially support the project this fork is built on, Zed Industries accepts sponsorship via GitHub Sponsors — funds go directly to Zed Industries as general company revenue, with no perks or entitlements.

## Friendly Links

- [Zed](https://github.com/zed-industries/zed) — the upstream this fork is based on.
- [Logos Editor Workstation](https://github.com/zixiao-labs/logos) — sister Electron-based editor from Zixiao Labs.
- [Wuling DevOps](https://github.com/zixiao-labs/Wuling-DevOps) — Zixiao Labs DevOps platform; auth & collab backend for the planned integration *(WIP)*.

## Acknowledgements

- **Zed Industries, Inc.** — Esperanta would not exist without their work; this is a derivative of Zed under the same licenses.
- **Kal'tsit** — character from *Arknights* by Hypergryph; theme inspiration only, no affiliation.

## FAQ

**Q: Why are there 1,848 Contributors?**

A: Because we're built on Zed, and upstream contributor information was preserved during history synchronization.

This is out of respect for the open-source community. (Actually, an accident occurred during synchronization.)

**Q: Who are the actual maintainers?**

A: Only @Amiya167 and closure-bot.

The other 1,846 people probably don't know they're here.
