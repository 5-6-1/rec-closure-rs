//! Nested recursive closures (a recursive closure inside another) and the
//! order-sensitivity of the capture scan (initializer expressions are
//! evaluated before their bindings).
//!
//! `unused_braces` is a false positive here: `|x| -> T { expr }` must use
//! braces (a brace-less `-> T expr` body is a syntax error), so the lint
//! cannot be satisfied by rewriting the code.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn nested_recursive_closures() {
    let outer = |n: i32| -> i32 {
        let inner = |m: i32| -> i32 { if m <= 1 { outer(m) } else { m * inner(m - 1) + outer(0) } };
        if n <= 1 { 1 } else { n * outer(n - 1) + inner(n) }
    };
    // inner(1)=outer(0)=1, inner(2)=3, inner(3)=10; outer(3)=3*5+10=25
    assert_eq!(outer(3), 25);
}

#[rec_closure]
#[test]
fn three_level_nesting() {
    let l0 = |n: i32| -> i32 {
        let l1 = |m: i32| -> i32 {
            let l2 = |k: i32| -> i32 { if k <= 0 { l1(0) } else { k + l2(k - 1) } };
            if m <= 0 { l0(0) } else { m + l1(m - 1) + l2(0) }
        };
        if n <= 0 { 0 } else { n + l0(n - 1) + l1(0) }
    };
    // L1(0)=L0(0)=0, L0(1)=1, L0(2)=3
    assert_eq!(l0(2), 3);
}

#[rec_closure]
#[test]
fn mixed_paths_nesting() {
    // outer typed (zero-alloc block), inner inferred (Rc inline)
    let outer = |n: i32| -> i32 {
        let inner = |m| if m <= 1 { 1 } else { m * inner(m - 1) };
        if n <= 1 { 1 } else { n * outer(n - 1) + inner(n) }
    };
    // inner(3)=6, outer(2)=4, outer(3)=3*4+6=18
    assert_eq!(outer(3), 18);
}

#[rec_closure]
#[test]
fn mixed_paths_nesting_reversed() {
    // outer inferred (Rc inline), inner typed (zero-alloc block)
    let outer = |n| {
        let inner = |m: i32| -> i32 { if m <= 1 { 1 } else { m * inner(m - 1) } };
        if n <= 1 { 1 } else { n * outer(n - 1) + inner(n) }
    };
    assert_eq!(outer(3), 18);
}

#[rec_closure]
#[test]
fn for_loop_and_if_let_capture_order() {
    // Iterators and match scrutinees are evaluated before their bindings,
    // so the outer `v` used there is a capture.
    let v = [1, 2, 3];
    let f = |start: i32| -> i32 {
        let mut total = start;
        for x in v.iter() {
            total += x;
        }
        if let Some(y) = v.first() {
            total += y;
        }
        total
    };
    assert_eq!(f(0), 1 + 2 + 3 + 1);
}
