# Shekyl GUI Wallet — Claude Code Context

**Repo:** https://github.com/Shekyl-Foundation/shekyl-gui-wallet (public)
**Sibling:** https://github.com/Shekyl-Foundation/shekyl-core — the node, the
Rust wallet stack this app embeds, and the canonical copies of the shared rules.

A Tauri 2 desktop wallet: a React + TypeScript + Tailwind front end (`src/`) and
Rust Tauri commands (`src-tauri/src/`) around the pure-Rust
`shekyl-engine-core::Engine`, embedded in-process (`src-tauri/src/engine_session.rs`),
with `shekyl-scanner`. `CONTRIBUTING.md` is the architecture and setup reference.

---

## The rules live in `.cursor/rules/`, and they are canonical

This file is orientation only. **`.cursor/rules/*.mdc` is how this repository is
developed**; where this file and a rule disagree, the rule wins, and
`00-mission` wins between rules. Several rules are copies of `shekyl-core`'s. A
copy that says so names its canonical file, and the canonical one wins.

Rules that apply to everything:

- [`00-mission`](.cursor/rules/00-mission.mdc) — the priority hierarchy
- [`06-branching`](.cursor/rules/06-branching.mdc) — `main` stable, `dev`
  integration; work branches off `dev`; release flow in `CONTRIBUTING.md`
- [`38-shared-estate-coordination`](.cursor/rules/38-shared-estate-coordination.mdc)
  — **before using any Foundation host, read and claim in the private
  `shekyl-dev` repository's `infrastructure/USAGE.md`**; a long-running test on
  a host is a `quiet` claim nobody else disturbs
- [`90-commits`](.cursor/rules/90-commits.mdc) — commit and PR discipline

Wallet-specific: [`27-composition-decomposition`](.cursor/rules/27-composition-decomposition.mdc),
[`28-command-surface`](.cursor/rules/28-command-surface.mdc),
[`80-usability`](.cursor/rules/80-usability.mdc),
[`81-no-protocol-knowledge`](.cursor/rules/81-no-protocol-knowledge.mdc),
[`82-failure-mode-ux`](.cursor/rules/82-failure-mode-ux.mdc). Security:
[`30-cryptography`](.cursor/rules/30-cryptography.mdc),
[`35-secure-memory`](.cursor/rules/35-secure-memory.mdc), and
`docs/GUI_SECURITY.md` — every Tauri command validates user input through
`src-tauri/src/validate.rs`.

**This repository is public.** Foundation host names, addresses and
operational state never go in it — not in docs, code, commit messages or PR
bodies. Name the role (`the floor device`, `an internal node`), never the host;
the details live in `shekyl-dev`.

## Build and test (what CI runs, `.github/workflows/ci.yml`)

```bash
npm ci
npm run lint && npm run typecheck && npm run build && npm test
scripts/ci/check_file_size_ratchet.sh
scripts/ci/check_command_surface.sh
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Run the Rust gates from inside `src-tauri/`: its `rust-toolchain.toml` pins the
toolchain, and the same command run elsewhere uses a different one. Run them
before claiming a change is done.
