//! Only bare closure initializers opt in to recursive name resolution.
// Explicit return types require braces, even when the expansion flags them.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn recursive_name_wins_over_previous_binding() {
    let f = |n| n + 100;
    assert_eq!(f(1), 101);
    let f = |n| if n == 0 { 1 } else { n * f(n - 1) };
    assert_eq!(f(4), 24);
}

#[rec_closure]
#[test]
fn typed_recursive_name_wins_over_previous_binding() {
    let f = |n| n + 100;
    assert_eq!(f(1), 101);
    let f = |n: i32| -> i32 { if n == 0 { 1 } else { n * f(n - 1) } };
    assert_eq!(f(4), 24);
}

#[rec_closure]
#[test]
fn captured_recursive_name_wins_over_previous_binding() {
    let base = 2;
    let f = |n| n + 100;
    assert_eq!(f(1), 101);
    let f = move |n: i32| -> i32 { if n == 0 { base } else { n * f(n - 1) } };
    assert_eq!(f(4), 48);
}

#[rec_closure]
#[test]
#[allow(unused_parens)] // Parentheses deliberately disable macro recognition.
fn wrapped_initializers_keep_ordinary_resolution() {
    fn wrap<F>(f: F) -> F {
        f
    }

    let f = |n| n + 1;
    let f = (|n| f(n) * 2);
    assert_eq!(f(2), 6);

    let g = |n| n + 1;
    let g = { |n| g(n) * 3 };
    assert_eq!(g(2), 9);

    let h = |n| n + 1;
    let h = wrap(|n| h(n) * 4);
    assert_eq!(h(2), 12);
}
