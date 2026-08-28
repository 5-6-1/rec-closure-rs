# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Recursion detection now honors shadowing: a reference to a nested closure
  parameter (or any inner binding) named like the recursive binding is no
  longer miscounted as a self-call, so non-recursive closures stay
  unexpanded and the detection agrees with the rewrite.
- Item names (`fn`/`const`/`static`) are no longer miscounted as captures.
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
