# rec_closure

[![crates.io](https://img.shields.io/crates/v/rec-closure.svg)](https://crates.io/crates/rec-closure)
[![docs.rs](https://img.shields.io/docsrs/rec-closure)](https://docs.rs/rec-closure)
[![CI](https://github.com/5-6-1/rec-closure-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/5-6-1/rec-closure-rs/actions/workflows/ci.yml)

A procedural macro that turns a closure into a recursive closure while
keeping the native-closure look: just refer to the binding by name inside
the body.

## Status

Implemented and tested: all three expansion paths below work end to end and
are covered by `cargo test` (`tests/basic.rs`, `tests/nesting.rs`,
`tests/references.rs`, `tests/shadowing.rs`, plus the compile-fail matrix in
`tests/fail.rs`). The snippets in this README are the actual surface syntax.

## Goal syntax

`#[rec_closure]` is an attribute macro on a `fn`. It scans the function for
`let name = <closure>;` bindings whose bodies reference `name`, and rewrites
them into recursive closures.

### Basic: zero annotations, no capture

```rust
#[rec_closure]
fn main() {
    let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };
    println!("{}", fact(5)); // 120
}
```

`move` is never required by the macro: a closure that captures nothing (or
only borrows) works without it, exactly like a native closure. Write `move`
only when the closure must own its captures — for example when it is moved
into another thread (see the `sync` example below) or returned from a
function.

### Borrow instead of move

```rust
#[rec_closure]
fn main() {
    let offset = 1;
    let sum = |n| if n <= 0 { offset } else { n + sum(n - 1) };
    println!("{}", sum(4)); // 11
    println!("{}", offset); // still usable after the closure
}
```

### Multi-arg with `mut` patterns

```rust
#[rec_closure]
fn main() {
    let gcd = |mut a, mut b| {
        if a < b {
            std::mem::swap(&mut a, &mut b);
        }
        if b != 0 { gcd(b, a % b) } else { a }
    };
    println!("{}", gcd(12, 32)); // 4
}
```

### Fully annotated: zero-allocation path

When every parameter and the return type are annotated, the macro expands to
a `fix_fn`-style scheme (no heap allocation, no reference counting):

```rust
#[rec_closure]
fn main() {
    let fib = |n: u32| -> u32 {
        if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
    };
    println!("{}", fib(20)); // 6765
}
```

### Zero parameters with interior-mutable state

```rust
#[rec_closure]
fn main() {
    let remaining = std::cell::Cell::new(5);
    let countdown = || {
        let n = remaining.get();
        if n <= 0 { 0 } else { remaining.set(n - 1); countdown() }
    };
    println!("{}", countdown()); // 0
}
```

### Multithreaded: `#[rec_closure(sync)]`

```rust
#[rec_closure(sync)]
fn main() {
    // `thread::spawn` requires a `'static` closure, so `move` is required
    // here (the closure owns everything it captures).
    let fact = move |n: i64| -> i64 {
        if n <= 1 { 1 } else { n * fact(n - 1) }
    };
    let handle = std::thread::spawn(move || fact(10));
    println!("{}", handle.join().unwrap()); // 3628800
}
```

## Path selection

| user annotations | expansion | per-recursion cost |
|---|---|---|
| complete, no captures | plain local `fn` | none (static call) |
| complete, captures, non-reference return | `&dyn HideFn` self-param (see `examples/typed.rs`) | one dyn dispatch, zero alloc |
| reference return (or none/partial) | `Rc + OnceCell + Weak` (see `examples/optimized.rs`) | `get` + `upgrade` + dyn dispatch |

"Captures" means uses of ordinary local variables. `fn`/`const`/`static`
items *nested inside the annotated `fn`* are pre-scanned and never counted
as captures: a fully annotated closure that calls such an item (or reads
its `const`) takes the capture-free path, so the plain-`fn` lowering stays
available even when the closure's body calls a helper defined beside it.

## Verified expansion shapes

- `examples/optimized.rs` — single-threaded inferred path
- `examples/sync.rs` — multithreaded inferred path
- `examples/typed.rs` — zero-allocation typed path

## Limitations

- `Fn` only (no `FnMut`/`FnOnce`): recursion needs a re-borrowable self.
- `sync` requires the closure and everything it captures to be `Send + Sync`.
- A reference return (`|s: &str| -> &str`) always routes away from the
  zero-allocation path: the `Fn(&dyn HideFn, ...) -> ...` bound cannot
  express an elided reference return (the elision rule would pick
  `&dyn HideFn`). With exactly one reference parameter and no captures, the
  closure lowers to a `fn` (whose own elision rule resolves the borrow).
  Every other shape — zero inputs, multiple inputs, or captures — goes
  through the inferred store, which resolves a `'static` return just like a
  native closure; a *borrow* return in those shapes has no way to be
  expressed and fails with rustc's raw `E0623` (the macro cannot tell a
  `'static` return from a borrow one ahead of time), so use explicit
  lifetimes (`-> &'static str`) or make the closure capture-free with a
  single reference parameter. `-> &'_ str` counts as elided.
- Mutually recursive closures are not supported: `let f = |n| g(n); let g =
  |n| f(n);` — neither body references its own name, so neither is
  recognized as recursive and the forward reference fails to resolve.
- References to `fn`/`const`/`static` items *nested inside the annotated `fn`*
  are treated as non-captures (see Path selection), so they do not force the
  dyn path. Items declared at module level are outside the macro's view and
  are conservatively counted as captures — use a nested item to keep the
  capture-free lowering. Shadowing such an item's name *inside* the closure
  body is still honored by the rewrite's scope tracking.
- The recursive closure must be a bare initializer: `let f = |n| ...;`.
  Parenthesized forms (`let f = (|n| ...);`) are not recognized — the
  supported syntax is deliberately just the native closure binding.
- The rewrite of the recursive name is scope-aware but syntactic: references
  that resolve to an inner binding shadowing the recursive name (including
  `let` chains, `const`/`static`/`fn` items, match arms, and nested closure
  parameters) are left untouched, and nested `fn` items are never rewritten.
  Names produced by other macros inside the body are invisible to the
  analysis (syn treats macro tokens as opaque), so a recursive call inside
  `vec![f(x)]` is not detected.
- A turbofish self-reference (`fact::<i32>(...)`) is recognized as recursion,
  but cannot compile: the generated fn/closure has no generic parameters, and
  a native closure would fail the same way.
- `async` recursive closures and a nested recursive closure reusing an
  enclosing one's name are rejected with explicit errors.
- A parameter whose body projects into it (`node.value`, method calls) must
  name its type, including ref-ness and pointee (`node: &mut Tree`). This is
  a rustc limitation shared with native closures: closure signatures are
  inferred once from an expectation context, and field access on an
  unresolved type variable cannot be deferred until a later call site.
  Annotating only ref-ness + pointee suffices (the return type can always
  stay inferred), and `&mut _` works when the body merely passes the
  parameter along.
- Returning/escaping a non-`move` recursive closure from the enclosing fn
  requires the user's `move`, as for a native closure: stable Rust has no
  per-variable capture modes, so the generated self-reference handle cannot
  be owned while the environment stays borrowed.
