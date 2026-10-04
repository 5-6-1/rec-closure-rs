//! Generated implementation names must not resolve to user declarations.
// Explicit closure return types require braces before expansion.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn local_names_do_not_collide() {
    let __rec_0_slot_ref = 7;
    let __rec_0_self = 2;
    let f = |n| {
        if n == 0 { __rec_0_slot_ref + __rec_0_self } else { f(n - 1) }
    };
    assert_eq!(f(2), 9);
}

#[rec_closure]
#[test]
fn parameter_names_do_not_collide() {
    let f = |__rec_0_rec| {
        if __rec_0_rec == 0 { 7 } else { f(__rec_0_rec - 1) }
    };
    assert_eq!(f(2), 7);
}

struct HideFn(i32);
struct HideFnImpl(i32);
struct F(i32);
struct __Rec0HideFn(i32);
struct __Rec1Callable(i32);

#[rec_closure]
#[test]
fn type_names_do_not_collide() {
    let base = 1;
    let f = |n: i32, a: HideFn, b: HideFnImpl, c: F| -> i32 {
        if n == 0 { base + a.0 + b.0 + c.0 } else { f(n - 1, a, b, c) }
    };
    assert_eq!(f(2, HideFn(2), HideFnImpl(3), F(4)), 10);
}

#[rec_closure]
#[test]
fn generated_name_families_are_reserved() {
    let base = 1;
    let f = |n: i32, a: __Rec0HideFn, b: __Rec1Callable| -> i32 {
        if n == 0 { base + a.0 + b.0 } else { f(n - 1, a, b) }
    };
    assert_eq!(f(2, __Rec0HideFn(2), __Rec1Callable(3)), 6);
}

#[rec_closure]
#[test]
fn fn_trait_uses_its_absolute_path() {
    struct Fn;
    let marker = Fn;
    let f = |n| if n == 0 { 7 } else { f(n - 1) };
    assert_eq!(f(2), 7);
    let base = 2;
    let g = |n: i32| -> i32 { if n == 0 { base } else { g(n - 1) } };
    assert_eq!(g(2), 2);
    let _ = marker;
}

#[rec_closure]
fn named_lifetime<'__rec_0_lt0>(left: &'__rec_0_lt0 i32) -> i32 {
    let f = |a: &'__rec_0_lt0 i32, b: &i32, n| {
        if n == 0 { *a + *b } else { f(a, b, n - 1) }
    };
    f(left, &2, 2)
}

#[test]
fn generated_lifetimes_do_not_shadow_explicit_ones() {
    assert_eq!(named_lifetime(&7), 9);
}
