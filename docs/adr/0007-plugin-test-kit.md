# Plugin tests use separate fixtures, a `render!` macro, and strictly mocked commands

Plugin authors test their plugins in-process with the SDK's `test` module (behind the `testing` feature), imported in one line through `test::prelude`. A test declares its fixtures first, renders the plugin with them, then asserts:

```rust
let dir = Dir::new().file("package.json", "{}");
let commands = Commands::new().ok("node --version", "v20.1.0\n");

let render = render!(NodejsPlugin { dir, commands });

assert_eq!(render.read("version"), "20.1.0");
```

Each fixture owns one concern and is built the same way, with `new()` and then one call per entry: `Dir` (`.file`, `.dir`, `.git_repo`), `Env` (`.var`) and `Commands` (`.ok`, `.fails`). `render!` binds the plugin to its fixtures once, so `applicable()`, `depth()` and `read()` take no plugin argument and compare with plain `assert_eq!`. While a render drives a plugin, `ctx.exec` answers from its `Commands`; nothing runs, and a command that isn't listed fails the test, naming the call to add. Each render's `Dir` is a fresh temporary directory, never the test process's cwd, so a plugin that reads a relative path instead of using `ctx.path` fails its own tests.

We optimized for the plugin author over the SDK. Tests should be trivial to write and read for contributors who don't know Rust well, even where that costs the SDK a macro and more code to maintain.

## Consequences

- Behavior that needs a real process (resolving commands on `PATH`, starting up, exiting on EOF, crashing) is covered by end-to-end tests in each plugin's package. They run the built binary through `CARGO_BIN_EXE_<name>`, using `Dir::executable` for fake commands and `test::context` for the render context.
- Mocked commands are held in a thread-local while one `read`, `applicable` or `depth` call runs. That works because the kit calls the plugin's handler on the test's own thread; it must keep doing so if plugins start handling requests concurrently.
- `#[export_plugin]` generates `main`, so a crate can hold only one exported plugin. Tests that need several plugins in one crate (the SDK's own) implement `Handler` by hand.
- Snapshot assertions of every method (e.g. with `insta`) are deferred, not rejected.

## Considered Options

- **One `TestCtx` factory for directory, env and commands.** Rejected: unrelated concerns behind one builder, and commands were configured in the middle of the render rather than with the other fixtures.
- **A `Render` struct literal with `..Default::default()`.** Rejected: rustfmt wraps it over several lines. Widening `struct_lit_width` also reflowed non-test code we wanted left alone.
- **A plain builder (`Render::new().dir(dir).commands(commands)`).** Rejected for the author's sake. It's plain Rust and easier for us to maintain, but `render!` is shorter, names the plugin once, and removes an assertion macro.
- **Starship's `ModuleRenderer` with a global table of default command mocks.** Rejected: expected values hidden in a shared table make tests harder to read. Every value a test depends on is visible in that test.
- **Running unlisted commands for real** (Starship's fallback). Rejected: tests would depend on what's installed on the machine.
- **Fake executables on `PATH` for in-process tests.** Rejected: slower, Unix-only, and unable to assert which arguments a plugin used. They remain for end-to-end tests only.
- **An `assert_plugin!` macro for several fields at once.** Removed once `render!` bound the plugin, since plain `assert_eq!` reads just as well.
- **A table-driven `plugin_tests!` macro.** Rejected: shorter still, but it hides Rust so much that tests stop reading as tests, and failures are harder to trace.
