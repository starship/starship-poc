# Plugins talk to the host through one message export defined by shared Rust types

A plugin exports a single function, `_plugin_handle`, and the host exports a single import, `_host_handle`. Each takes one JSON-encoded message and returns one: `Request`/`Response` from host to plugin, `HostRequest`/`HostResponse` from plugin to host. The four enums live in `plugin-core/src/protocol.rs`, which both the runtime and the SDK compile against, so the contract can't drift between them. The host reads the plugin's `Manifest` (name, `kind`, `shadows`, methods, `abi_version`) once at load and refuses plugins built against a different `ABI_VERSION`.

Request handling lives in ordinary SDK functions (`dispatch::handle_plugin`, `dispatch::handle_vcs_plugin`). The `#[export_plugin]` and `#[export_vcs_plugin]` macros only generate the method table and the export, which holds the plugin in a `thread_local!`.

## Consequences

- The manifest's `kind` (`General` or `Vcs`) says which contract a plugin implements. `shadows` is a separate, general list that every plugin carries (empty by default), so it can later express supersession among non-VCS plugins too, e.g. deno or bun over nodejs.
- Adding a host capability means adding a `HostRequest` variant and its match arm, not a new import with wrappers on both sides.
- Plugins built against the previous per-operation exports no longer load. There were none outside this repo.
- A plugin request that traps or returns the wrong variant is logged and treated as a failure: the plugin is inapplicable, methods return nil. The old ABI failed open (applicable by default).

## Considered Options

- **Collapse the per-operation exports in place.** Rejected: it keeps the contract as matching export-name strings in two crates, and every new capability still touches an import, a guest wrapper, and a host wrapper.
- **WebAssembly Component Model (WIT).** Rejected for now: plugins are Rust-only, so a language-neutral contract buys little; WIT can't express recursive JSON-like values, so plugin methods would stay JSON-encoded anyway; and it would add a 0.x `wit-bindgen` dependency and a target change. The enums map directly onto a WIT world if non-Rust plugins ever matter.
- **Extism.** Rejected: it adds its own memory kernel, pins its own wasmtime version, and brings a WASI dependency we don't want.
