//! Captures stay captures when they shadow an item or occur in opaque macros.
// Explicit return types require braces, even when the expansion flags them.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn macros_may_capture() {
    let base = 7;
    let f = |n: i32| -> String { if n == 0 { format!("{base}") } else { f(n - 1) } };
    let g = |n| if n == 0 { format!("{base}") } else { g(n - 1) };
    assert_eq!(f(2), "7");
    assert_eq!(g(2), "7");
}

#[rec_closure]
#[test]
#[allow(non_snake_case)] // An item can hide the prelude name, then be shadowed.
fn constructor_spelling_does_not_hide_a_capture() {
    fn Some() -> i32 {
        100
    }
    assert_eq!(Some(), 100);
    let base = 7;
    let Some = || base;
    let f = |n: i32| -> i32 { if n == 0 { Some() } else { f(n - 1) } };
    assert_eq!(f(2), 7);
}

#[rec_closure]
#[test]
fn for_if_and_match_bindings_mask_items() {
    fn helper() -> i32 {
        100
    }
    assert_eq!(helper(), 100);
    for helper in [7, 8] {
        let f = |n: i32| -> i32 { if n == 0 { helper } else { f(n - 1) } };
        assert_eq!(f(2), helper);
    }
    if let Some(helper) = Some(8) {
        let f = |n: i32| -> i32 { if n == 0 { helper } else { f(n - 1) } };
        assert_eq!(f(2), 8);
    }
    let actual = match Some(9) {
        Some(helper) => {
            let f = |n: i32| -> i32 { if n == 0 { helper } else { f(n - 1) } };
            f(2)
        }
        None => 0,
    };
    assert_eq!(actual, 9);
    // Earlier control-flow bindings must not leak into this closure.
    let f = |n: i32| -> i32 { if n == 0 { helper() } else { f(n - 1) } };
    let callable: fn(i32) -> i32 = f;
    assert_eq!(callable(2), 100);
}

#[rec_closure]
#[test]
fn let_chain_bindings_are_visible_in_later_operands() {
    fn helper() -> i32 {
        100
    }
    assert_eq!(helper(), 100);
    if let Some(helper) = Some(7)
        && {
            let f = |n: i32| -> i32 { if n == 0 { helper } else { f(n - 1) } };
            f(2) == 7
        }
    {
        assert_eq!(helper, 7);
    } else {
        panic!("the let-chain must use its local binding");
    }
}

#[rec_closure]
#[test]
fn let_else_reads_the_outer_binding() {
    let base = 7;
    let f = |n: i32| -> i32 {
        let Some(value) = n.checked_sub(1) else {
            return base;
        };
        if value < 0 { 0 } else { f(value) }
    };
    assert_eq!(f(2), 0);
    assert_eq!(f(i32::MIN), 7);
}
