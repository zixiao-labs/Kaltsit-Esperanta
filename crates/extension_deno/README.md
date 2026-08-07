# extension_deno

Secure embedded JS/TS extension runtime.

## Security contracts

- Zero ambient filesystem / network access
- Remote imports (`http(s)`, `npm:`, `jsr:`, `file:`, …) are rejected
- Dangerous ops are capability-gated (manifest ∩ host granter)
- Host load runs on a dedicated background thread via `async_host_runtime`

## Features

- Default: secure stub host (CI-friendly, no V8 download)
- `deno-core`: embed `deno_core::JsRuntime` for real evaluation

## Entry files

Dev install detects `extension.js` / `extension.ts` / `extension.mjs` and sets
`ExtensionLibraryKind::Deno`.
