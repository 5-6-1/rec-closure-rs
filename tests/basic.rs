//! Basic behavior of the `#[rec_closure]` attribute macro: plain recursion,
//! path selection, capture, returning, and multithreading.
//!
//! `unused_braces` is a false positive here: `|x| -> T { expr }` must use
//! braces (a brace-less `-> T expr` body is a syntax error), so the lint
//! cannot be satisfied by rewriting the code.
#![allow(unused_braces)]

use rec_closure::rec_closure;

mod common;
use common::Tree;

#[rec_closure]
#[test]
fn basic_recursion() {
    let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };
    assert_eq!(fact(5), 120);
}

#[rec_closure]
#[test]
fn multi_arg_with_mut_patterns() {
    let gcd = |mut a, mut b| {
        if a < b {
            std::mem::swap(&mut a, &mut b);
        }
        if b != 0 { gcd(b, a % b) } else { a }
    };
    assert_eq!(gcd(12, 32), 4);
}

#[rec_closure]
#[test]
fn non_move_borrows_outer_variable() {
    let offset = 1;
    let sum = |n| if n <= 0 { offset } else { n + sum(n - 1) };
    assert_eq!(sum(4), 11);
    assert_eq!(offset, 1); // the closure borrowed, not moved
}

#[rec_closure]
#[test]
fn zero_parameters_with_cell_state() {
    let remaining = std::cell::Cell::new(5);
    let countdown = || {
        let n = remaining.get();
        if n <= 0 {
            0
        } else {
            remaining.set(n - 1);
            countdown()
        }
    };
    assert_eq!(countdown(), 0);
}

#[rec_closure]
#[test]
fn typed_annotations_zero_alloc_path() {
    let fib = |n: u32| -> u32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } };
    assert_eq!(fib(20), 6765);
}

#[rec_closure]
#[test]
fn zero_alloc_captures_environment() {
    // The reason the typed path exists at all: environment capture with zero
    // per-call overhead — something a plain `fn` cannot do (it would have to
    // thread `base` through every call).
    let base = 10;
    let scaled = |n: i32| -> i32 { if n <= 1 { base } else { n + scaled(n - 1) } };
    assert_eq!(scaled(3), 15); // 3 + 2 + base
}

#[rec_closure(sync)]
#[test]
fn multithreaded_sync_path() {
    let fact = move |n: i64| -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } };
    let handle = std::thread::spawn(move || fact(10));
    assert_eq!(handle.join().unwrap(), 3_628_800);
}

#[rec_closure]
#[test]
fn capture_free_typed_becomes_fn() {
    // No captures + complete annotations lower to a local `fn`: the binding
    // coerces to a function pointer, which a closure cannot do. This proves
    // the fn path (and its zero overhead) was taken.
    let fact = |n: i32| -> i32 { if n <= 1 { 1 } else { n * fact(n - 1) } };
    let f: fn(i32) -> i32 = fact;
    assert_eq!(f(5), 120);
}

#[rec_closure]
#[test]
fn nested_item_call_is_not_a_capture() {
    // Calling an outer `fn` item *nested inside the annotated fn* is not a
    // capture (the item pre-scan recognizes it), so the closure still lowers
    // to a plain `fn` (provable via function-pointer coercion) even though
    // its body calls outside itself.
    fn helper(n: i32) -> i32 {
        n * 10
    }
    let f = |n: i32| -> i32 { if n <= 1 { 1 } else { helper(n) + f(n - 1) } };
    let f: fn(i32) -> i32 = f;
    assert_eq!(f(3), 51); // 30 + (20 + 1)
}

#[rec_closure]
#[test]
fn plain_closures_left_untouched() {
    let add = |a: i32, b: i32| a + b;
    assert_eq!(add(2, 3), 5);
    let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };
    assert_eq!(fact(4), 24);
}

#[rec_closure]
#[test]
fn multiple_recursive_closures_in_one_fn() {
    let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };
    let gcd = |mut a, mut b| {
        if a < b {
            std::mem::swap(&mut a, &mut b);
        }
        if b != 0 { gcd(b, a % b) } else { a }
    };
    assert_eq!(fact(5), 120);
    assert_eq!(gcd(12, 32), 4);
}

#[rec_closure]
#[test]
fn partial_annotation_falls_back_to_inferred() {
    // parameter annotated, return type inferred -> inferred (Rc) path
    let fact = |n: i32| if n <= 1 { 1 } else { n * fact(n - 1) };
    assert_eq!(fact(5), 120);
}

#[rec_closure]
fn make_fact() -> impl Fn(i32) -> i32 {
    // escaping the fn requires `move` (same rule as for a native closure:
    // the generated identifiers must not borrow block-local scaffolding)
    let fact = move |n| if n <= 1 { 1 } else { n * fact(n - 1) };
    fact
}

#[test]
fn returned_recursive_closure() {
    assert_eq!(make_fact()(5), 120);
}

#[rec_closure]
#[test]
fn struct_literal_is_not_a_capture() {
    // A struct literal names a *type*, not a captured variable, so the
    // closure still lowers to a local fn.
    let f = |n: i32| -> i32 {
        if n <= 0 { Tree { value: 0, children: vec![] }.value } else { n + f(n - 1) }
    };
    assert_eq!(f(3), 6);
}

#[rec_closure(sync)]
#[test]
fn sync_fn_path() {
    // No captures + complete annotations lower to a local fn, which is
    // trivially Send + Sync; the sync attribute is irrelevant.
    let f = |n: i64| -> i64 { if n <= 1 { 1 } else { n * f(n - 1) } };
    let handle = std::thread::spawn(move || f(10));
    assert_eq!(handle.join().unwrap(), 3_628_800);
}

#[rec_closure]
fn make_fact_fn() -> impl Fn(i32) -> i32 {
    // Capture-free + fully annotated → fn path; the fn item is `'static` and
    // can be returned.
    let fact = |n: i32| -> i32 { if n <= 1 { 1 } else { n * fact(n - 1) } };
    fact
}

#[test]
fn returned_fn_path_closure() {
    assert_eq!(make_fact_fn()(5), 120);
}
