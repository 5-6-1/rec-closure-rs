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

## Git workflow

- **Hard:** work on `main` with small, self-contained commits. One logical
  change per commit; do not mix refactors with unrelated fixes.
- Reverting: use `git revert` and commit with type `revert:` (the message
  references the reverted commit hash).
- Line endings are normalized to LF via `.gitattributes`; never commit
  CRLF-mixed files.
- Tags mark released versions (`v0.1.0`); the tag message summarizes the
  release.

## Commits

- **Hard:** Conventional Commits, imperative mood, subject ≤ 50 chars:
  `<type>: <subject>`
- Allowed types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`,
  `build`. No scope for a single-crate project.
- Use the subject line only for small changes. For larger ones, add a blank
  line and a body explaining *why* (not what) and any behavioral impact.
  `BREAKING CHANGE:` as a footer when the public macro behavior changes.
- Comments and commit messages are written in English.

## Continuous integration

- `.github/workflows/ci.yml` enforces `cargo fmt --check`, clippy with
  `-D warnings`, `cargo test`, and the 350-line file limit on every push /
  pull request. A change that breaks CI is **hard** rejected.

## Optional pre-commit hook

To run the fast checks locally before every commit:

```powershell
$hook = ".git/hooks/pre-commit"
@'
cargo fmt --all --check || exit 1
cargo clippy --all-targets -- -D warnings || exit 1
bad=$(find src tests -name "*.rs" -exec wc -l {} + | awk '$1 > 350 { print $2 }')
[ -z "$bad" ] || { echo "files over 350 lines: $bad"; exit 1; }
'@ | Set-Content -NoNewline $hook
```

## Comments

- Code comments and doc comments in English; explain *why* over *what*.
