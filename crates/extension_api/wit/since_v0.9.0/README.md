# Extension API 0.9.0 (proposed)

This WIT tree proposes pull-request provider exports for the shared review
surface. The host still loads `0.8.0` until the guest ABI and wasmtime bindings
are cut over.

## Manifest

```toml
[pull_request_providers.github]
label = "GitHub"

[[capabilities]]
kind = "http:fetch"
host = "api.github.com"
path = ["**"]
```

## Guest exports

- `pull-request-provider-metadata`
- `list-pull-requests`
- `get-pull-request`
- `post-review-comments`
- `resolve-review-thread`

Host-side registration already exists through
`ExtensionHostProxy::register_pull_request_provider`. Guest WASM wiring lands
when the published `zed_extension_api` version advances to `0.9.0`.

V8 / Deno extensions should mirror the same ABI; they must not gain a separate
“execute arbitrary WASM” mouthpiece. TypeScript definitions live in
`crates/extension_api/typescript/index.d.ts`.
