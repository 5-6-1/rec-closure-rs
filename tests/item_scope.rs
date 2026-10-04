//! Block items are visible before their declaration, across expansion paths.
// Lowercase items intentionally collide with recursive closure bindings.
#![allow(non_upper_case_globals)]
// Explicit return types require braces, even when the expansion flags them.
#![allow(unused_braces)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn nonrecursive_closure_stays_untouched() {
    let base = 1;
    let f = |n: i32| -> i32 {
        if n == 0 {
            base
        } else {
            let value = f(n - 1);
            fn f(_: i32) -> i32 {
                100
            }
            value
        }
    };
    assert_eq!(f(1), 100);
}

#[rec_closure]
#[test]
fn fn_path_respects_whole_block_items() {
    let f = |n: i32| -> i32 {
        if n == 0 {
            1
        } else {
            let child = f(n - 1);
            let value = {
                let v = f(n);
                fn f(n: i32) -> i32 {
                    n * 10
                }
                v
            };
            child + value
        }
    };
    let callable: fn(i32) -> i32 = f;
    assert_eq!(callable(2), 31);
}

#[rec_closure]
#[test]
fn typed_capture_respects_whole_block_items() {
    let base = 1;
    let f = |n: i32| -> i32 {
        if n == 0 {
            base
        } else {
            let child = f(n - 1);
            let value = {
                let v = f(n);
                fn f(n: i32) -> i32 {
                    n * 10
                }
                v
            };
            child + value
        }
    };
    assert_eq!(f(2), 31);
}

#[rec_closure]
#[test]
fn inferred_path_respects_whole_block_items() {
    let f = |n| {
        if n == 0 {
            1
        } else {
            let child = f(n - 1);
            let value = {
                let v = f(n);
                fn f(n: i32) -> i32 {
                    n * 10
                }
                v
            };
            child + value
        }
    };
    assert_eq!(f(2), 31);
}

#[rec_closure]
#[test]
fn forward_items_keep_fn_lowering() {
    let f = |n: i32| -> i32 { if n == 0 { BASE } else { helper(n) + f(n - 1) } };
    fn helper(n: i32) -> i32 {
        n * 10
    }
    const BASE: i32 = 1;
    let callable: fn(i32) -> i32 = f;
    assert_eq!(callable(2), 31);
}

#[rec_closure]
#[test]
fn local_binding_shadows_outer_item_for_capture() {
    fn helper() -> i32 {
        100
    }
    assert_eq!(helper(), 100);
    let base = 7;
    let helper = || base;
    let f = |n: i32| -> i32 { if n == 0 { helper() } else { f(n - 1) + 1 } };
    assert_eq!(f(2), 9);
}

#[rec_closure]
#[test]
fn nested_closure_parameter_shadows_outer_item() {
    fn helper() -> i32 {
        100
    }
    assert_eq!(helper(), 100);
    let run = |helper: i32| {
        let f = |n: i32| -> i32 { if n == 0 { helper } else { f(n - 1) + 1 } };
        f(2)
    };
    assert_eq!(run(7), 9);
}

#[rec_closure]
#[test]
fn const_and_static_shadow_before_declaration() {
    let base = 1;
    let f = |n: i32| -> i32 {
        if n == 0 {
            base
        } else {
            let child = f(n - 1);
            let a = {
                let value = f;
                const f: i32 = 3;
                value
            };
            let b = {
                let value = f;
                static f: i32 = 4;
                value
            };
            child + a + b
        }
    };
    assert_eq!(f(2), 15);
}
