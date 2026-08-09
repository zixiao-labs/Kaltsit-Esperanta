---
title: Pull Request Providers
description: "Extension-provided pull request and merge request connectors for Zed's shared review surface."
---

# Pull Request Providers

Zed keeps the review surface provider-agnostic. Local Git diffs and remote
pull requests share the same inline comment model. Hosting providers such as
GitHub or GitLab are contributed by extensions; they are not hardcoded into
core UI.

> **Note:** The guest WASM ABI for these methods is proposed in Extension API
> `0.9.0`. Host registration and the `http:fetch` capability already exist.

## Manifest

```toml
[pull_request_providers.github]
label = "GitHub"

[[capabilities]]
kind = "http:fetch"
host = "api.github.com"
path = ["**"]
```

## Host behavior

1. On extension load, Zed registers each
   `[pull_request_providers.*]` entry through
   `ExtensionHostProxy::register_pull_request_provider`.
2. The shared review surface asks the provider for summaries, detail, and
   review threads.
3. Submit / resolve actions call back into the provider. Core never speaks
   GitHub- or GitLab-specific terms; the provider label is shown instead.

## HTTP capability

`http-client.fetch` is gated by the `http:fetch` capability. Users can restrict
hosts through `granted_extension_capabilities`, the same way they restrict
`download_file`.

## Deno / TypeScript

If a Deno extension path is enabled later, it must implement the same method
shape. TypeScript definitions live in
`crates/extension_api/typescript/index.d.ts`. There is no separate V8 pathway
for executing arbitrary WASM modules.
