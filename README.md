<h3 align="center">Starship Rewrite (proof of concept)</h3>
<p align="center">A rewrite of Starship in more maintainable, idiomatic Rust.</p>

---

This repo is meant to serve as a proof of concept for a new architecture for Starship. Nothing here is set in stone. It will serve as a way to share what I envision a future rewrite of Starship will look like.

> 👉 Comments and feedback are appreciated in Issues.

## Goals

These are the main goals in mind while I work on this rewrite:

- Detect which modules should be enabled by only scanning the project root
- Emit errors for consuming applications and clearer error messaging
- Abstract the VCS system to allow for first-class support of various VCSs
- Use traits for more consistent and testable APIs
- Don't render the module output to allow for per-shell formatting
