---
title: Pull Request Providers
description: >-
  Extension-provided pull request and merge request connectors for Zed's
  shared review surface.
---

# Pull Request Providers

Zed keeps the review surface provider-agnostic. Local Git diffs and remote
pull requests share the same inline comment model. Hosting providers such as
GitHub or GitLab are contributed by extensions; they are not hardcoded into
core UI.

> **Note:** The guest WASM ABI for these methods is proposed in Extension API
> `0.9.0`. Host registration and the `http:fetch` capability already exist.

## Getting Started {#getting-started}

Use an extension when you want remote pull-request review threads to feed the
same Diff Review surface as local comments.

1. Install or develop an extension that declares
   `[pull_request_providers.*]` in `extension.toml`.
2. Grant the provider's API hosts through `http:fetch` in
   `granted_extension_capabilities` (Settings Editor or JSON).
3. Open a pull request from the shared review entry points once the provider
   is registered.

Or add this to your settings.json:

```json [settings]
{
  "granted_extension_capabilities": [
    {
      "kind": "http:fetch",
      "host": "api.github.com",
      "path": ["**"]
    }
  ]
}
```

### Manifest {#manifest}

```toml
[pull_request_providers.github]
label = "GitHub"

[[capabilities]]
kind = "http:fetch"
host = "api.github.com"
path = ["**"]
```

### Host behavior {#host-behavior}

1. On extension load, Zed registers each
   `[pull_request_providers.*]` entry through
   `ExtensionHostProxy::register_pull_request_provider` when the extension
   implements the provider ABI.
2. The shared review surface asks the provider for summaries, detail, and
   review threads.
3. Submit / resolve actions call back into the provider. Core never speaks
   GitHub- or GitLab-specific terms; the provider label is shown instead.

### HTTP capability {#http-capability}

`http-client.fetch` is gated by the `http:fetch` capability. Users can
restrict hosts through `granted_extension_capabilities`, the same way they
restrict `download_file`. Each redirect hop is authorized with the same
host/path rules as the initial URL.

### Deno / TypeScript {#deno-typescript}

If a Deno extension path is enabled later, it must implement the same method
shape. TypeScript definitions live in
`crates/extension_api/typescript/index.d.ts`. There is no separate V8 pathway
for executing arbitrary WASM modules.
