# VCS plugins use a parallel trait, not a sub-trait of `Plugin`

`VcsPlugin` and `Plugin` are sibling traits, not parent/child. Each declares its own `NAME` const and a kind-appropriate gate (`Plugin::is_applicable` returning `bool`; `VcsPlugin::detect_depth` returning `Option<u32>`). The macro layer (`#[export_vcs_plugin]`) synthesizes `_plugin_is_applicable` at the WASM ABI as `detect_depth().is_some()`, so the daemon's gate logic stays uniform across plugin kinds without forcing VCS plugin authors through a generic predicate that doesn't fit them naturally.

## Considered Options

- **`Plugin::KIND` enum const** as a runtime-checked discriminant on a unified trait. Rejected because we wanted compile-time enforcement of the VCS-specific contract (`root`, `branch`, `SHADOWS`), not a string-typed convention validated at plugin load.
- **`VcsPlugin: Plugin`** with the supertrait pulling in `NAME` and `is_applicable`. Rejected because the supertrait carried no benefit beyond a shared `NAME` const, and forced authors to write a one-line `fn is_applicable(&self) -> bool { self.detect_depth().is_some() }` body that was always identical and always derived. Forcing authors through a generic gate concept obscured rather than expressed the VCS plugin's natural shape.
