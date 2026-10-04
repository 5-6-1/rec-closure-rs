//! Replacing a recursive value must preserve surrounding syntax.
// Explicit closure return types require braces before expansion.
#![allow(unused_braces)]

use rec_closure::rec_closure;

struct Holder<F> {
    f: F,
}

#[rec_closure]
#[test]
fn inferred_self_in_shorthand_field() {
    let f = |n| {
        if n == 0 {
            7
        } else {
            let holder = Holder { f };
            (holder.f)(n - 1)
        }
    };
    assert_eq!(f(2), 7);
}

#[rec_closure]
#[test]
fn captured_self_in_shorthand_field() {
    let base = 7;
    let f = |n: i32| -> i32 {
        if n == 0 {
            base
        } else {
            let holder = Holder { f };
            (holder.f)(n - 1)
        }
    };
    assert_eq!(f(2), 7);
}

#[rec_closure]
#[test]
fn shadowed_shorthand_stays_unchanged() {
    let f = |n| {
        if n == 0 {
            let f = 7;
            Holder { f }.f
        } else {
            f(n - 1)
        }
    };
    assert_eq!(f(2), 7);
}
