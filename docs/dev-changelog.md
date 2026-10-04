# Development Log

Design decisions, implementation notes, and validation for maintainers.
User-facing changes are summarized in `CHANGELOG.md`; usage stays in `README.md`.

## [0.1.2] - 2026-10-04

### 2026-10-04 — Independent review and release preparation

#### Review

- Re-ran the complete gate on the assembled tree: formatting, all-target
  checking, Clippy with warnings denied, 12 unit tests, 95 integration tests,
  five compile-fail fixtures, one doctest, warning-denied documentation, and
  four examples. Every Rust file remains below 350 lines.
- Checked the recorded counts, the version baseline (crates.io `0.1.1`,
  working `0.1.2`), and changelog coverage against the implementation.
- Probed two previously uncovered shapes: the attribute on `impl` and trait
  default methods, and `self` capture inside a nested `impl`. Both work: a
  method signature parses as an ordinary `fn` item, and the user body lands
  in a closure rather than in the generated trait method, so `self` still
  resolves to the method receiver.
- Confirmed the conservative capture rule is required, not merely cautious:
  an outer local named `Some` is genuinely captured, and the scanner cannot
  see outer locals, so an unresolved constructor name must count as one.

#### Polish

- Documented `fn`-item coverage (free, associated, and method bodies) and the
  reference-only status of the hand-unrolled examples in the README.
- Recorded the path-selection consequence of conservative capture analysis
  under `Changed` in `CHANGELOG.md`.
- Translated the remaining non-English comments in the hand-unrolled examples.

#### Release

- Re-checked crates.io (`0.1.1` newest) and the remote tags (`v0.1.0`,
  `v0.1.1`) on 2026-10-04: `0.1.2` is the next free version. The published
  `0.1.2` is exactly the tagged commit.
- Release steps: tag `v0.1.2`, push to GitHub, wait for CI on Ubuntu,
  Windows, and the 1.88 MSRV job, then publish to crates.io.

### 2026-09-16 — Completeness review and expansion boundaries

#### Findings and fixes

- Reproduced collisions with user locals, parameters, `HideFn`/`HideFnImpl`
  types, an `F` type, and a named lifetime. Generate numbered type and local
  names with mixed-site spans, reserve families present in the source
  (including macro tokens and lifetimes), and use absolute `Fn` paths.
- Destructured and wildcard parameters previously exposed an `Rc` directly
  on the inferred path, which is callable by dereference but does not
  implement `Fn`. Always generate a forwarding closure with fresh arguments;
  keep user patterns on the implementation closure. Fully typed patterns
  can also take the function or captured zero-allocation path.
- A disabled binding left implementation statements enabled, and disabled
  parent statements still produced nested macro errors. Project `cfg` and
  nested `cfg_attr` onto generated statements and diagnostics. Keep original
  attributes on the user binding; do not duplicate lint expectations.
  Ordinary successful expansions avoid the extra attribute reparse.
- Rewriting `Holder { f }` changed its expression but left shorthand output
  referring to `f`. Insert the colon when rewriting the value, preserving
  the user's field name and leaving shadowed shorthand unchanged.

#### Validation

- Added `tests/hygiene.rs`, `tests/parameters.rs`, `tests/attributes.rs`, and
  `tests/self_value.rs` with regressions for the reproduced failures.
- Added `tests/lifecycle.rs`: cloned inferred closures remain callable after
  one owner drops, captures are released exactly once by the last owner,
  and the synchronized form is exercised by four threads.
- Added all-target checking and warning-denied documentation to CI, alongside
  existing formatting, Clippy, tests, examples, and file-size checks.
- Local stable validation passed: formatting, all-target checking, Clippy
  with warnings denied, 12 unit tests, 95 integration tests, five compile-fail
  fixtures, one doctest, warning-denied documentation, and four examples.
  All Rust source and test files remain below 350 lines. MSRV execution
  remains covered by the existing Rust 1.88 CI job.
- Recognition remains limited to direct closure initializers. No dependency
  or public syntax was added; the working version remains `0.1.2`/`Unreleased`.

### 2026-09-16 — Closure recognition and name resolution

#### Decisions

- Keep recognition limited to a bare closure initializer:
  `let f = |...| ...;` or `let f = move |...| ...;`. Do not unwrap
  parentheses, blocks, or calls. This boundary prevents an open-ended
  requirement to discover closures inside arbitrary expressions.
- Within a recognized closure, an unshadowed reference to its binding name
  denotes recursive self, including when an older binding has that name.
  Inner parameters, local bindings, and block items retain their lexical scope.
- Type annotations constrain types and enable optimization. They must not
  select different name-resolution rules.
- Keep nested recursive closure names distinct. Reject a collision before
  rewriting so the result does not depend on the expansion path.

#### Implementation

- Consolidated recursive-name detection and replacement in `src/replace.rs`
  and shared binding predicates in `src/scope.rs`.
- Pre-scan block items so `fn`, `const`, and `static` names apply throughout
  their block. Fixed a case that returned `100` without the attribute but
  incorrectly returned `1` after expansion.
- Added lexical expansion context in `src/context.rs` to track local and
  control-flow bindings that shadow item names. Removed repeated traversal
  of generated statements.
- Treat opaque macro invocations and unresolved names as possible captures;
  this can select a more conservative expansion but avoids illegal captures
  by generated functions. Macro expansion and import resolution remain
  outside the analysis.
- Use the inferred store for signatures with `_` holes or references to
  enclosing generics. Preserve explicit lifetimes, return-type annotations,
  and binding type/mutability constraints.

#### Validation

- Added regression coverage in `tests/syntax.rs`, `tests/item_scope.rs`,
  `tests/capture_scope.rs`, and `tests/generics.rs`.
- Added compile-fail fixtures for same-name nesting on inferred and captured
  typed paths. Existing function-path diagnostics remain covered.
- Checked formatting, all targets, Clippy with warnings denied, the complete
  test suite, doctests, documentation, and the four main examples. Rust files
  remain within the 350-line limit.

#### Version baseline

- Verified crates.io's latest version and GitHub's latest tag as `0.1.1`
  and `v0.1.1` on 2026-09-16.
- Set the working package version to `0.1.2`. Keep both changelogs under
  `Unreleased` until release; no tag or publication has been made.
