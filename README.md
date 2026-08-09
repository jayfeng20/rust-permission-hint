# rust-permission-hint

Hover over a variable in a Rust file and see the **permissions** it holds in the
eyes of the borrow checker.

The permission model comes from the ["Fixing Ownership Errors" chapter of _The
Rust Programming Language_, Interactive Edition](https://rust-book.cs.brown.edu/),
which describes access as three permissions:

- **R** — Read: the value can be read.
- **W** — Write: the value can be mutated.
- **O** — Own: the value can be moved or dropped.

Held permissions render highlighted, missing ones struck through, e.g. a shared
reference binding `let y = &x;` shows `R ~~W~~ O` on `y`.

## How it works

The actual permission analysis is done by
[Aquascope](https://github.com/cognitive-engineering-lab/aquascope) (the tool
behind the interactive book). We don't reimplement any borrow-checker logic — we
shell out to it and render the result:

```
Zed ──hover──▶ permission-lsp ──runs──▶ cargo aquascope ──JSON──▶ R/W/O tooltip
```

- `crates/permission-lsp` — a language server (stable Rust) that runs
  `cargo aquascope permissions`, maps the cursor to a permission boundary, and
  returns the R/W/O tooltip.
- `zed-extension` — a thin Zed extension (Rust → WASM) that launches the server
  as an additional language server for Rust files.

Because the server only *invokes* Aquascope as a subprocess, it stays on stable
Rust; the pinned-nightly requirement lives entirely inside Aquascope.

## Prerequisites

### 1. Install Aquascope

Aquascope uses compiler internals and pins an exact nightly in its
[`rust-toolchain.toml`](https://github.com/cognitive-engineering-lab/aquascope/blob/main/rust-toolchain.toml).
**That pin drifts over time — check the current value** and install that
toolchain. As of writing it is `nightly-2026-05-01`:

```sh
rustup toolchain install nightly-2026-05-01 \
  -c rust-src -c rustc-dev -c llvm-tools-preview -c miri

# Note: force the pinned toolchain — `cargo install --git` does NOT read the
# cloned repo's rust-toolchain.toml.
cargo +nightly-2026-05-01 install aquascope_front \
  --git https://github.com/cognitive-engineering-lab/aquascope --locked
```

This installs `cargo-aquascope` and `aquascope-driver` into `~/.cargo/bin`.

### 2. Install the language server

```sh
cargo install --path crates/permission-lsp
```

This puts `permission-lsp` on your `$PATH`, where the Zed extension looks for it.

## Install the Zed extension

You do **not** need a source build of Zed — the regular app installs local dev
extensions:

1. Open Zed → command palette (`cmd-shift-p`) → **`zed: install dev extension`**.
2. Select the `zed-extension/` directory in this repo.

Zed builds it to WASM and registers it as an extra language server for Rust
(alongside rust-analyzer). If prompted for a wasm target:
`rustup target add wasm32-wasip2`.

## Usage

Open a `.rs` file in a Cargo project and hover a variable — the project itself
needs no setup. When it isn't pinned to Aquascope's nightly, the server detects
the installed toolchain that provides Aquascope's driver and re-runs against it
automatically; if that toolchain isn't installed, the hover tells you to add it.

The first hover runs Aquascope (a few seconds); results are cached until you save.

## Limitations

- **Single-file crates.** Aquascope's boundaries don't carry a file name, so a
  hover resolves by line/column across the crate. Ideal for TRPL-style exercises;
  multi-file mapping is future work.
- **Reflects the last save.** Aquascope reads source from disk, so hovers show
  the last *saved* state, not unsaved edits.
- **Non-ASCII lines.** Aquascope uses char columns; LSP uses UTF-16 — positions
  can differ on lines with non-ASCII characters.

## Roadmap

- **F (Flow) permission.** Aquascope also tracks a *Flow* permission for
  references crossing function boundaries (lifetime errors), opt-in via
  `cargo aquascope permissions --show-flows`. Additive to the R/W/O stack; a
  planned follow-up.
- Multi-file crates.

## License

MIT. Aquascope is a separate MIT-licensed project by the Cognitive Engineering
Lab at Brown University.
