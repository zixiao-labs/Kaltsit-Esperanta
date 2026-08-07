# CEF bindings (`extension_cef`)

Dynamically links Chromium Embedded Framework (`libcef`) for the embedded
browser / frontend-enhancement stack. The shared library is **not** compiled
into the editor; install CEF on the system (or point at a Framework path).

## Loading model

- `AsyncCefHost` opens `libcef` on a dedicated background thread via
  `async_host_runtime` so GPUI never blocks on `dlopen`.
- When the library is missing, the host lifecycle becomes `Failed` with a clear
  message (fail-soft). Tests use stub mode (`AsyncCefHost::spawn_stub` or the
  `force-stub` feature).

## Required system libraries

Ensure one of these is loadable:

- macOS: `libcef.dylib` / `Chromium Embedded Framework`
- Linux: `libcef.so`
- Windows: `libcef.dll`

Optional helpers such as `libcef_dll_wrapper` follow upstream CEF packaging.
