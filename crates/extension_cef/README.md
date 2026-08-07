# CEF bindings (`extension_cef`)

Dynamically links Chromium Embedded Framework (`libcef`) for the embedded
browser / frontend-enhancement stack. The shared library is **not** compiled
into the editor.

## Loading model

- `AsyncCefHost` opens `libcef` on a dedicated background thread via
  `async_host_runtime` so GPUI never blocks on `dlopen`.
- Production installs use a managed download under `data_dir/cef/<version>/`
  (menu: **Install Browser Runtime**). After the first install, component-only
  auto-update (`auto_update::init_component_updates`) can refresh the pin —
  full-application auto-update stays off.
- Override the artifact URL with `ZETA_CEF_DOWNLOAD_URL`.
- When the library is missing, the host falls back to stub mode (fail-soft).
  Tests use `AsyncCefHost::spawn_stub` or the `force-stub` feature.

## Required libraries

Probe order: managed install → platform defaults:

- macOS: `Chromium Embedded Framework`
- Linux: `libcef.so`
- Windows: `libcef.dll`
