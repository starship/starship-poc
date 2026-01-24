<h3 align="center">Starship Rewrite (proof of concept)</h3>
<p align="center">A rewrite of Starship in more maintainable, idiomatic Rust.</p>

---

This repo is meant to serve as a proof of concept for a new architecture for Starship. Nothing here is set in stone. It will serve as a way to share what I envision a future rewrite of Starship will look like.

> 👉 Comments and feedback are appreciated in Issues.

## Todos

- [x] Create daemon for plugin and config loading
- [x] Use lua for programmatic configuration
- [ ] Use wasm for plugins
  - [ ] Create plugin SDK for opinionated authoring and testing
  - [ ] Have wasm bytecode compile to native and cached on disk
- [ ] Have modules return structs rather than strings
- [ ] Budget: 16.67ms (60fps) or 8.33ms (120fps)
- [x] Daemon responds to `nc` for other shell prompts to use:
      `echo '{"pwd":"'$PWD'","user":"'$USER'"}' | nc -U ~/.config/starship/starship.sock`
- [ ] Have modules enable based on repo root

## Contributing

1. Run the daemon:

```
cargo run --release -p starship-daemon
```

2. Then run the prompt:

```
cargo run --release -p starship
```

Run them both with `STARSHIP_PROFILE=1` to get runtime profiling metrics.
