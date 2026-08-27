//! Shadowing of the recursive name inside the closure body, across all
//! binding forms: `let`, match arms, `if let`/`while let` (including 2024
//! let chains), for-loop variables, nested closure parameters, nested
//! blocks, and `const`/`static`/`fn` items.
//!
//! `unused_braces` is a false positive here: `|x| -> T { expr }` must use
//! braces (a brace-less `-> T expr` body is a syntax error), so the lint
//! cannot be satisfied by rewriting the code.
#![allow(unused_braces)]
// `non_upper_case_globals`: shadowing the recursive name requires a
// same-named lowercase `const`/`static`, which is the point of those tests.
#![allow(non_upper_case_globals)]

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn fn_path_shadowing_is_natural() {
    // The fn path rewrites nothing, so shadowing follows ordinary lexical
    // rules: the inner `let f = 10` shadows the fn inside its branch, and
    // the recursive call elsewhere is unaffected.
    let f = |n: i32| -> i32 {
        if n == 1 {
            let f = 10;
            f + n
        } else {
            n * f(n - 1)
        }
    };
    assert_eq!(f(1), 11); // shadowed binding: 10 + 1
    assert_eq!(f(3), 66); // 3 * 2 * 11
}

#[rec_closure]
#[test]
fn capture_before_local_shadow() {
    // The capture scan is order-sensitive: `let y = x` uses the *outer* `x`
    // before the inner `let x = 2` shadows it, so this closure captures and
    // must not lower to a `fn`.
    let x = 10;
    let f = |n: i32| -> i32 {
        let y = x;
        let x = 2;
        y + n + x
    };
    assert_eq!(f(1), 13); // 10 + 1 + 2
    assert_eq!(x, 10);
}

#[rec_closure]
#[test]
fn local_shadow_without_capture() {
    // Here the inner `let x = 2` comes first, so nothing external is used
    // and the closure lowers to a `fn`; the shadow still works inside it.
    let f = |n: i32| -> i32 {
        let x = 2;
        x + n
    };
    assert_eq!(f(1), 3);
}

#[rec_closure]
#[test]
#[allow(clippy::redundant_locals)] // the exact `let x = x;` shape is the point
fn let_self_init_captures() {
    // `let x = x;` reads the *outer* `x` on the right before the inner `x`
    // is bound, so the closure captures and must not lower to a `fn`.
    let x = 10;
    let f = |n: i32| -> i32 {
        let x = x;
        x + n
    };
    assert_eq!(f(1), 11);
    assert_eq!(x, 10);
}

#[rec_closure]
#[test]
fn self_reference_before_shadow_zero_alloc() {
    // The reference to `f` happens *before* `let f = ...` shadows it, so
    // every use still denotes the recursive binding and the expansion is
    // legal. Earlier revisions rejected any binding of the name outright.
    let base = 1;
    let f = |n: i32| -> i32 {
        let y = if n <= 1 { base } else { f(n - 1) };
        let f = base + 2; // shadows only from here on
        y + f
    };
    assert_eq!(f(3), 10); // f(1)=1+3=4, f(2)=4+3=7, f(3)=7+3=10
}

#[rec_closure]
#[test]
fn self_reference_before_shadow_inferred() {
    // Same shape as `self_reference_before_shadow_zero_alloc`, but routed to
    // the inferred (Rc) path by the unannotated parameter.
    let base = 2;
    let g = |n| {
        let y = if n <= 0 { base } else { g(n - 1) };
        let g = 5;
        y + g
    };
    assert_eq!(g(2), 17); // g(0)=2+5=7, g(1)=7+5=12, g(2)=12+5=17
}

#[rec_closure]
#[test]
fn match_arm_shadow_after_use() {
    // Scope-aware rewrite: `f(k - 1)` occurs before any shadow and denotes
    // the recursive binding; the arm pattern `k` does not collide, and the
    // later `let f = 999` shadows from its statement onward.
    let base = 1;
    let f = |n: i32| -> i32 {
        let y = match n {
            0 => base,
            k => f(k - 1) + k,
        };
        let f = 999; // shadows only from here on; never referenced after
        let _ = f;
        y
    };
    assert_eq!(f(2), 4); // f(0)=base=1, f(1)=f(0)+1=2, f(2)=f(1)+2=4
}

#[rec_closure]
#[test]
fn inner_fn_item_with_colliding_param() {
    // The recursive call passes its result through a nested `fn` whose
    // parameter collides with the binding name; the body of that `fn` must
    // stay untouched.
    let base = 1;
    let f = |n: i32| -> i32 {
        fn helper(f: i32) -> i32 {
            f * 10
        }
        if n <= 0 { base } else { helper(f(n - 1)) }
    };
    assert_eq!(f(2), 100); // f(0)=1, f(1)=helper(1)=10, f(2)=helper(10)=100
}

#[rec_closure]
#[test]
fn for_loop_var_shadows_self() {
    // The loop variable `f` shadows the recursive name inside the loop body
    // only; the recursive call after the loop is unaffected.
    let base = 1;
    let f = |n: i32| -> i32 {
        if n <= 1 {
            base
        } else {
            let mut acc = 0;
            for f in 0..n {
                acc += f;
            }
            acc + f(n - 1)
        }
    };
    // f(2) = (0+1) + f(1) = 2; f(3) = (0+1+2) + 2 = 5
    assert_eq!(f(3), 5);
}

#[rec_closure]
#[test]
fn if_let_binding_shadows_self() {
    // The `if let` pattern shadows `f` in the then-branch; the else branch
    // still recurses.
    let base = 1;
    let f = |n: i32| -> i32 { if let Some(f) = Some(n) { f + base } else { f(n - 1) } };
    assert_eq!(f(1), 2); // 1 + base
}

#[rec_closure]
#[test]
fn while_let_binding_shadows_self() {
    // `while let` shadows `f` inside the loop body.
    let base = 1;
    let f = |n: i32| -> i32 {
        if n <= 0 {
            base
        } else {
            let child = f(n - 1);
            let mut count = 0;
            let mut v = vec![child, 0];
            while let Some(f) = v.pop() {
                count += f;
            }
            count
        }
    };
    assert_eq!(f(2), 1); // f(1) = base, then 1 + 0
}

#[rec_closure]
#[test]
fn nested_closure_param_shadows_self() {
    // A nested closure whose parameter collides with the recursive name:
    // its body uses the parameter, the outer body still recurses.
    let base = 1;
    let f = |n: i32| -> i32 {
        let g = |f: i32| f * 2;
        if n <= 1 { base } else { g(f(n - 1)) }
    };
    assert_eq!(f(2), 2); // g(f(1)) = g(1) = 2
}

#[rec_closure]
#[test]
fn block_local_shadow_stays_scoped() {
    // The shadow inside a nested block does not leak out; the recursive call
    // after the block still denotes the fn.
    let f = |n: i32| -> i32 {
        if n <= 0 {
            0
        } else {
            let y = {
                let f = 5;
                f * n
            };
            y + f(n - 1)
        }
    };
    // f(1) = 5 + 0 = 5; f(2) = 10 + 5 = 15
    assert_eq!(f(2), 15);
}

#[rec_closure]
#[test]
fn match_arm_pattern_does_not_leak() {
    // A match-arm pattern must not leak its bindings into later arms: the
    // `k` in the `_` arm is the *outer* `k` (a capture), not the arm `k`.
    let k = 100;
    let f = |n: i32| -> i32 {
        match n {
            0 => 1,
            k if k < 0 => k,
            _ => f(n - 1) + k,
        }
    };
    // f(0)=1, f(1)=1+100=101, f(2)=101+100=201
    assert_eq!(f(2), 201);
    assert_eq!(k, 100);
}

#[rec_closure]
#[test]
fn nested_closure_param_does_not_leak() {
    // A nested closure's parameter must not leak into the outer body: the
    // `k` used after the closure is the outer capture.
    let k = 100;
    let f = |n: i32| -> i32 {
        let g = |k: i32| k * 2;
        if n <= 0 { k } else { g(f(n - 1)) }
    };
    // f(0)=100, f(1)=g(100)=200, f(2)=g(200)=400
    assert_eq!(f(2), 400);
}

#[rec_closure]
#[test]
fn let_chain_shadowing() {
    // Rust 2024 let chains: the `f` bound in the chain is a local value,
    // visible in the right-hand condition and the then-branch.
    let base = 1;
    let f = |n: i32| -> i32 {
        if let Some(f) = Some(n)
            && f > 0
        {
            f + base
        } else {
            f(n - 1)
        }
    };
    assert_eq!(f(1), 2); // 1 + base
}

#[rec_closure]
#[test]
fn while_let_chain_shadowing() {
    let base = 1;
    let f = |n: i32| -> i32 {
        if n <= 0 {
            base
        } else {
            let child = f(n - 1);
            let mut count = 0;
            let mut v = vec![child];
            while let Some(f) = v.pop()
                && f > 0
            {
                count += f;
            }
            count
        }
    };
    assert_eq!(f(1), 1); // f(0) = base = 1, popped and counted
}

#[rec_closure]
#[test]
fn match_guard_binding_shadow() {
    // The arm pattern `f if f > 0` binds `f`; syn 3 visits the guard
    // expression *after* the pattern, and the shadow flag is set before
    // `visit_pat_mut` runs, so the guard's `f` and the arm body's `f` refer
    // to the arm binding, while the `_` arm still recurses.
    let f = |n: i32| -> i32 {
        match n {
            f if f > 0 => f,
            _ => f(n - 1),
        }
    };
    assert_eq!(f(3), 3); // arm binding: f = n = 3
}

#[rec_closure]
#[test]
fn const_item_shadows_recursive_name() {
    // A `const f` shadows the recursive name from its statement onward.
    let base = 1;
    let f = |n: i32| -> i32 {
        if n <= 0 {
            base
        } else {
            let y = f(n - 1) + n;
            const f: i32 = 3;
            y + f
        }
    };
    // f(0)=1, f(1)=(1+1)+3=5
    assert_eq!(f(1), 5);
}

#[rec_closure]
#[test]
fn static_item_shadows_recursive_name() {
    let base = 1;
    let f = |n: i32| -> i32 {
        if n <= 0 {
            base
        } else {
            let y = f(n - 1) + n;
            static f: i32 = 4;
            y + f
        }
    };
    assert_eq!(f(1), 6); // (1+1)+4
}
