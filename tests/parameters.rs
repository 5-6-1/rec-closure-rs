//! Parameter patterns must retain ordinary callable behavior on every path.
// Explicit closure return types require braces before expansion.
#![allow(unused_braces)]

use rec_closure::rec_closure;

fn invoke(f: impl Fn((i32, i32)) -> i32) -> i32 {
    f((2, 3))
}

#[rec_closure]
fn inferred_tuple() -> impl Fn((i32, i32)) -> i32 {
    let f = move |(n, acc)| if n == 0 { acc } else { f((n - 1, acc + n)) };
    f
}

#[rec_closure]
fn typed_tuple(base: i32) -> impl Fn((i32, i32)) -> i32 {
    let f = move |(n, acc): (i32, i32)| -> i32 {
        if n == 0 { acc + base } else { f((n - 1, acc + n)) }
    };
    f
}

#[test]
fn destructured_closures_can_escape_and_satisfy_fn() {
    assert_eq!(invoke(inferred_tuple()), 6);
    assert_eq!(invoke(typed_tuple(1)), 7);
}

#[rec_closure]
#[test]
fn typed_destructuring_keeps_the_fn_path() {
    let f = |(n, acc): (i32, i32)| -> i32 { if n == 0 { acc } else { f((n - 1, acc + n)) } };
    let pointer: fn((i32, i32)) -> i32 = f;
    assert_eq!(invoke(pointer), 6);
}

#[rec_closure]
#[test]
fn wildcard_parameter_remains_callable() {
    let f = |_, n| if n == 0 { 7 } else { f((), n - 1) };
    let callable: &dyn Fn((), i32) -> i32 = &f;
    assert_eq!(callable((), 2), 7);
}
