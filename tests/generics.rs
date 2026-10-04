//! Type annotations must not move outer generics into independent items.
// Explicit return types require braces, even when the expansion flags them.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
fn copy_rec<T: Copy>(x: T) -> T {
    let f = |n: u32, value: T| -> T { if n == 0 { value } else { f(n - 1, value) } };
    f(2, x)
}

#[rec_closure]
fn captured<T: Copy>(x: T) -> T {
    let f = |n: u32| -> T { if n == 0 { x } else { f(n - 1) } };
    f(2)
}

#[rec_closure]
fn sized<const N: usize>(x: [u8; N]) -> [u8; N] {
    let f = |n: u32, value: [u8; N]| -> [u8; N] { if n == 0 { value } else { f(n - 1, value) } };
    f(2, x)
}

#[rec_closure]
fn borrowed<'a>(value: &'a str) -> &'a str {
    let f = |n: u32, s: &'a str| -> &'a str { if n == 0 { s } else { f(n - 1, s) } };
    f(2, value)
}

#[rec_closure]
fn body_generic<T>() -> usize {
    let f = |n: u32| -> usize { if n == 0 { std::mem::size_of::<T>() } else { f(n - 1) } };
    f(2)
}

#[test]
fn enclosing_generics_remain_available() {
    assert_eq!(copy_rec(7), 7);
    assert_eq!(captured(8), 8);
    assert_eq!(sized([1, 2]), [1, 2]);
    let text = String::from("borrowed");
    assert_eq!(borrowed(&text), "borrowed");
    assert_eq!(body_generic::<u32>(), 4);
}

#[rec_closure]
#[test]
fn underscores_remain_inference_holes() {
    let f = |n: _| -> i32 { if n == 0 { 1 } else { n * f(n - 1) } };
    let g = |n: i32| -> _ { if n == 0 { 1 } else { n * g(n - 1) } };
    assert_eq!(f(4), 24);
    assert_eq!(g(4), 24);
}

#[rec_closure]
#[test]
fn explicit_static_input_is_not_generalized() {
    let f = |n, s: &'static str| {
        if n == 0 { s } else { f(n - 1, s) }
    };
    assert_eq!(f(2, "static"), "static");
}

#[rec_closure]
#[test]
fn binding_type_and_mutability_survive() {
    let mut f: fn(i32) -> i32 = |n: i32| -> i32 { if n == 0 { 1 } else { n * f(n - 1) } };
    assert_eq!(f(4), 24);
    f = |n| n + 1;
    assert_eq!(f(4), 5);
}

#[rec_closure]
#[test]
fn partial_signature_keeps_its_return_constraint() {
    let f = |n| -> u8 { if n == 0 { 7 } else { f(n - 1) } };
    assert_eq!(std::mem::size_of_val(&f(2)), 1);
}
