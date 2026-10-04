# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-10-04

### Fixed

- Use one scope traversal for recursive-name detection and replacement.
  Block-local items now shadow throughout their block, including before
  declaration, consistently across all expansion paths.
- Track local and control-flow bindings that shadow outer items during
  capture analysis. Treat opaque macros and unresolved constructor names
  conservatively instead of incorrectly selecting a capture-free function.
- Reject same-name recursive nesting before rewriting on every path.
- Keep outer generics and `_` type holes in the inferred store; preserve
  explicit lifetimes, return types, and binding type/mutability constraints.
- Avoid collisions between generated names and user locals, parameters,
  types, and lifetimes; qualify generated `Fn` bounds explicitly.
- Keep destructured and wildcard parameters callable through `Fn`, including
  returned closures, and retain function lowering for typed destructuring.
- Preserve `cfg`/`cfg_attr` across generated statements and diagnostics,
  including disabled parent statements, without duplicating lint expectations.
- Preserve field names when rewriting recursive values in struct shorthand.

### Changed

- Capture analysis is more conservative: a macro invocation or an unresolved
  constructor name (`Some`, `None`, `Ok`, `Err`) in a body counts as a
  possible capture. A fully annotated closure that mentions one now takes the
  typed-capture or store path instead of a plain `fn`, which can add one
  dynamic dispatch; this keeps generated items correct when such a name
  actually denotes a captured local.

### Documentation

- Clarify bare-initializer recognition and recursive-name resolution in the
  README. Record design decisions, implementation, and validation in
  `docs/dev-changelog.md`.

## [0.1.1] - 2026-08-28

### Fixed

- Recursion detection now honors shadowing: a reference to a nested closure
  parameter (or any inner binding) named like the recursive binding is no
  longer miscounted as a self-call, so non-recursive closures stay
  unexpanded and the detection agrees with the rewrite.
- `fn`/`const`/`static` items nested inside the annotated `fn` are no longer
  miscounted as captures; calling such an item no longer forces the dyn path.
- `rust-version` is pinned to the real MSRV (1.88) and checked by a CI job.
- The compile-fail contracts (async, same-name nesting, container-nested
  elided reference returns) are covered by a trybuild matrix.

## [0.1.0] - 2026-08-28

Initial release.

### Added

- `#[rec_closure]` on a `fn`: `let name = |...| ...` bindings whose bodies
  reference `name` become recursive closures.
- Three expansion paths: a plain local `fn` (capture-free, fully annotated),
  a zero-allocation `&dyn HideFn` self-parameter (captures, fully
  annotated), and an `Rc + OnceCell + Weak` inferred store (any shape).
- `#[rec_closure(sync)]` switches to the multithreaded expansion
  (`Arc + OnceLock`, `Send + Sync`).
- Published to [crates.io](https://crates.io/crates/rec-closure) and
  [GitHub](https://github.com/5-6-1/rec-closure-rs).
