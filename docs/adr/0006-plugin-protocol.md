# Plugins are native processes exchanging shared Rust message types

Each plugin is a separately built native executable. The daemon spawns one long-lived process per plugin and exchanges newline-delimited JSON messages with it over stdin and stdout. The messages (`Request`/`Response` and the `Manifest`) live in `plugin-core`, which both the daemon and the SDK compile against, so the contract can't drift between them. Each request carries the render context (the shell's pwd and environment), and the plugin reads files and runs commands itself.

We chose processes over WASM because WASM bought less than it cost:

- The sandbox was mostly nominal: the host already let plugins run any command and read any environment variable.
- `gix` and many other crates don't build for `wasm32-unknown-unknown`, so the git plugin couldn't use gitoxide.
- Every call already went through serde, so shared Rust types survive a pipe unchanged.

## Consequences

- Plugins are trusted code. Nothing isolates a plugin from the user's files or network. OS-level sandboxing (Seatbelt, Landlock) is possible later.
- No host calls. Plugins never call back into the daemon in the middle of a request, so each exchange is one request and one response.
- Every message carries a request ID, so the daemon can send several requests to the same plugin without waiting (concurrent renders, prefetching) and match responses as they arrive.
- Plugins handle requests concurrently. The SDK requires plugins to be `Sync`. Every path comes from the request's render context, never the process cwd, because two concurrent requests can have different pwds.
- Processes start eagerly when the daemon starts, and their `Describe` requests are sent in parallel, because the Lua globals need every manifest. The manifest declares the plugin's name, `kind` (`General` or `Vcs`), `shadows`, methods and `abi_version`. A plugin that fails `Describe` or reports a different `abi_version` is logged and skipped; there's no compatibility range.
- `shadows` is a general list every plugin carries (empty by default), separate from `kind`, so it can later express supersession among non-VCS plugins too, e.g. deno or bun over nodejs.
- A plugin request that fails, times out or returns the wrong variant makes that plugin inapplicable for the render, and its methods return nil.
- Discovery is explicit: the daemon runs only the executables in `STARSHIP_PLUGIN_DIR`, never `starship-plugin-*` found on `PATH`.
- Distribution needs a build per platform instead of one `.wasm` file. A reusable release workflow and a plugin template are meant to keep that invisible to plugin authors.
- A message round trip over a pipe measured about 3.6 µs (p50), against about 1 µs for a WASM call. That's around 0.1 ms on a busy prompt, small next to the subprocess and git work the plugins do.

## Considered Options

- **WASM modules with custom host calls** (the previous design). Rejected: it rules out gix and most crates that touch the filesystem, and it isn't a real sandbox while `exec` is open.
- **WASM plus WASI filesystem.** Rejected: gix still doesn't build properly for WASM (memory-mapped files, missing pieces), and the newer WASI version needs the Component Model.
- **WASM with declared permissions.** Rejected for now: it's the only option that would be truly sandboxed, but it costs the most to build and still rules out gix.
- **gix inside the daemon, plugins stay WASM.** Rejected: it splits features between the daemon and plugins, and third-party VCS plugins couldn't get the same performance.
- **Native dynamic libraries.** Rejected: Rust has no stable ABI, so third-party plugins would break whenever their compiler or dependencies differed from the daemon's, and a crash or hang takes down the daemon.
- **A fresh process per render.** Rejected: 2–5 ms to spawn each plugin on every render, and gix couldn't keep its repository state warm.
- **WebAssembly Component Model (WIT) or Extism.** Rejected with WASM itself. WIT also can't express recursive JSON-like values, and Extism pins its own wasmtime version and brings WASI.
