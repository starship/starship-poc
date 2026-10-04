<h3 align="center">Starship Rewrite (proof of concept)</h3>
<p align="center">A rewrite of Starship in more maintainable, idiomatic Rust.</p>

---

This repo is meant to serve as a proof of concept for a new architecture for Starship. Nothing here is set in stone. It will serve as a way to share what I envision a future rewrite of Starship will look like.

> 👉 Comments and feedback are appreciated in Issues.

## Todos

- [x] Create daemon for plugin and config loading
- [x] Use lua for programmatic configuration
  - [ ] Have lua values use metamethods to query WASM plugin
- [ ] Use WASM for plugins
  - [ ] Create plugin SDK for opinionated:
    - [ ] Authoring 
    - [ ] Testing
  - [ ] Have wasm bytecode compile to native and cached on disk
- [ ] Have modules generate lua types on load
- [ ] Budget: 16.67ms (60fps) or 8.33ms (120fps)
- [x] Daemon responds to `nc` for other shell prompts to use:
      `echo '{"pwd":"'$PWD'","user":"'$USER'"}' | nc -U ~/.config/starship/starship.sock`
- [x] Cache binary output keyed by resolved path, size, and mtime
- [ ] Have modules enable based on repo root


## Example config
```lua
-- Conditionally show "root" when running as root
rootUsername = ctx.user == "root" and red("root") or nil

-- git.branch calls the git plugin lazily; nil outside a repo.
-- compact() drops nils and joins what's left with a space.
return compact(rootUsername, ctx.pwd, git.branch, "❯")
```

## Contributing

1. Run the daemon. Building the runtime also compiles the workspace plugins, and `STARSHIP_PLUGIN_DIR` points the daemon at them (it defaults to `~/.config/starship/plugins`):

```
STARSHIP_PLUGIN_DIR=target/wasm-plugins/wasm32-unknown-unknown/release cargo run --release -p starship-daemon
```

2. Then run the prompt:

```
cargo run --release -p starship
```

Run them both with `STARSHIP_PROFILE=1` to get runtime profiling metrics.
