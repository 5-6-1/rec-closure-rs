# Contributing

Development conventions for this repository. Rules marked **hard** are
enforced: a change that violates them is rejected in review.

## Toolchain

- Latest stable Rust (currently 1.98+), edition **2024** (`rustfmt.toml` and
  `Cargo.toml` must agree).
- `cargo fix --edition` when upgrading.

## File size

- **Hard:** no source file (under `src/` or `tests/`, Rust only) may exceed
  **350 lines**. Split into modules / test files instead.
- Check with PowerShell from the repository root:

  ```powershell
  Get-ChildItem src,tests -Recurse -Filter *.rs |
    Where-Object { (Get-Content $_.FullName).Count -gt 350 }
  ```

  Output must be empty.

## Dependencies

- **Hard:** never add a dependency without explaining its purpose first and
  getting approval. Prefer `std`; prefer mature, lightweight crates.
- This crate is deliberately small: `proc-macro2`, `quote`, `syn`
  (features `full`, `visit`, `visit-mut`).

## Error handling

- Structured errors in the macro use `syn::Error`; diagnostics are collected
  (`Ctx::errors`) so one bad closure does not hide the others.
- Do not add `anyhow`.

## Tests

- Integration tests live under `tests/`, one file per concern
  (`basic`, `shadowing`, `nesting`, `references`); shared fixtures in
  `tests/common/`.
- **Hard:** every new behavior gets a test; every fixed bug gets a regression
  test.
- Keep each test file under the 350-line limit.

## Quality gate

Before committing, all of these must pass with **zero warnings**:

```powershell
cargo fmt --all --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --example optimized
cargo run --example sync
cargo run --example nested
cargo run --example typed
```

Do not silence lints with `#[allow]` unless there is a concrete reason, and
always comment it.

## Commits

- **Hard:** Conventional Commits, imperative mood, subject ≤ 50 chars:
  `<type>: <subject>`
- Allowed types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`,
  `build`. No scope for a single-crate project.
- Comments and commit messages are written in English.

## Comments

- Code comments and doc comments in English; explain *why* over *what*.
