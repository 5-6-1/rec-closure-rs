//! Reference parameters and reference returns: higher-ranked lifetime
//! lifting in the inferred store, and the routing of elided reference
//! returns away from the zero-allocation path.
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
fn mutable_reference_parameter() {
    // A `Fn` closure cannot mutate captures, but a `&mut` *parameter* is
    // borrowed from the caller, so the body may mutate through it. Each
    // node's value is incremented; the sum of incremented values is returned.
    let count = |node: &mut Tree| -> i32 {
        node.value += 1;
        let mut total = node.value;
        for child in &mut node.children {
            total += count(child);
        }
        total
    };
    let mut tree = Tree {
        value: 1,
        children: vec![Tree { value: 2, children: vec![] }, Tree { value: 3, children: vec![] }],
    };
    assert_eq!(count(&mut tree), 9); // 2 + 3 + 4
    assert_eq!(tree.value, 2); // the root was mutated
}

#[rec_closure]
#[test]
fn shared_reference_parameter() {
    // Sums the tree through a shared reference.
    let total = |node: &Tree| -> i32 {
        let sum: i32 = node.children.iter().map(&total).sum();
        node.value + sum
    };
    let tree = Tree {
        value: 10,
        children: vec![Tree { value: 20, children: vec![] }, Tree { value: 30, children: vec![] }],
    };
    assert_eq!(total(&tree), 60);
}

#[rec_closure]
#[test]
fn reference_param_partial_annotation() {
    // Reference parameter annotated, return type inferred: the inferred
    // store erases `&mut Tree` through a higher-ranked lifetime, so the
    // borrow is decided per call and the escape check passes.
    let count = |node: &mut Tree| {
        node.value += 1;
        let mut total = node.value;
        for child in &mut node.children {
            total += count(child);
        }
        total
    };
    let mut tree = Tree {
        value: 1,
        children: vec![Tree { value: 2, children: vec![] }, Tree { value: 3, children: vec![] }],
    };
    assert_eq!(count(&mut tree), 9);
    assert_eq!(tree.value, 2);
}

#[rec_closure]
#[test]
fn mixed_reference_and_inferred_params() {
    // Reference parameter annotated, plain parameter inferred: the erased
    // signature becomes `for<'a0> Fn(&'a0 mut Tree, _) -> _`.
    let count = |node: &mut Tree, n| {
        node.value += n;
        let mut total = node.value;
        for child in &mut node.children {
            total += count(child, n);
        }
        total
    };
    let mut tree = Tree {
        value: 1,
        children: vec![Tree { value: 2, children: vec![] }, Tree { value: 3, children: vec![] }],
    };
    assert_eq!(count(&mut tree, 2), 12); // 3 + 4 + 5
    assert_eq!(tree.value, 3);
}

#[rec_closure]
#[test]
fn partially_typed_reference_param() {
    // `&mut _` is only inferable when the body does not project into the
    // referenced type (field/method access needs a concrete type in Rust);
    // here it is merely passed along, so `_` resolves at the call site.
    let pass = |node: &mut _, n: i32| {
        if n <= 0 { 0 } else { pass(node, n - 1) }
    };
    let mut tree = Tree { value: 1, children: vec![] };
    assert_eq!(pass(&mut tree, 3), 0);
    assert_eq!(tree.value, 1);
}

#[rec_closure]
#[test]
fn container_of_mut_refs_param() {
    // References nested inside containers (`Vec<&mut Tree>`) are lifted to
    // higher-ranked lifetimes just like a top-level `&mut Tree`.
    let sum = |nodes: Vec<&mut Tree>| {
        let mut total = 0;
        for node in nodes {
            node.value += 1;
            total += node.value + sum(node.children.iter_mut().collect());
        }
        total
    };
    let mut a = Tree {
        value: 1,
        children: vec![Tree { value: 2, children: vec![] }, Tree { value: 3, children: vec![] }],
    };
    let mut b = Tree { value: 2, children: vec![] };
    assert_eq!(sum(vec![&mut a, &mut b]), 12); // (2+3+4) + 3
    assert_eq!(a.value, 2);
    assert_eq!(b.value, 3);
}

#[rec_closure]
#[test]
fn deep_nested_container_refs() {
    // `Vec<Vec<&mut Tree>>`: references nested two levels deep are lifted to
    // higher-ranked lifetimes.
    let f = |groups: Vec<Vec<&mut Tree>>| {
        if groups.is_empty() {
            0
        } else {
            let mut total = 0;
            for group in groups {
                for node in group {
                    node.value += 1;
                    total += node.value;
                }
            }
            total + f(vec![])
        }
    };
    let mut a = Tree { value: 1, children: vec![] };
    let mut b = Tree { value: 2, children: vec![] };
    assert_eq!(f(vec![vec![&mut a], vec![&mut b]]), 5); // 2 + 3
}

#[rec_closure]
#[test]
fn elided_ref_return() {
    // An elided reference return (`&str -> &str`) cannot be expressed in the
    // zero-allocation `Fn` bound (the elision rule picks `&dyn HideFn`), so
    // it routes to the inferred path where the return is erased as `_`.
    let marker = "!";
    let f = |s: &str| -> &str { if s.is_empty() { marker } else { f(&s[1..]) } };
    assert_eq!(f("ab"), "!");
    assert_eq!(f(""), "!");
}

#[rec_closure]
#[test]
fn explicit_lifetime_ref_return() {
    // The escape hatch for elided reference returns: explicit lifetimes
    // (`-> &'static str`) compile fine through the zero-allocation path.
    let marker = "!";
    let f = |n: i32| -> &'static str { if n <= 0 { marker } else { f(n - 1) } };
    assert_eq!(f(2), "!");
}

#[rec_closure]
#[test]
fn fn_path_borrow_return() {
    // Capture-free + one reference parameter + elided reference return
    // borrowing from the parameter: the `fn` path's own elision rule handles
    // it (the inferred `_` return cannot express a borrow).
    let f = |s: &str| -> &str { if s.is_empty() { s } else { f(&s[1..]) } };
    assert_eq!(f("ab"), "");
    assert_eq!(f(""), "");
}

#[rec_closure]
#[test]
fn underscore_lifetime_ref_return() {
    // `-> &'_ str` is semantically identical to `-> &str` and must be
    // treated as elided, not as an explicit lifetime.
    let marker = "!";
    let f = |s: &str| -> &'_ str { if s.is_empty() { marker } else { f(&s[1..]) } };
    assert_eq!(f("ab"), "!");
}

#[rec_closure]
#[test]
fn fn_ptr_param_static_return() {
    // A `fn`-pointer parameter contains a reference (`fn(&str)`) that is not
    // a borrowable input for elision; with no borrowable inputs the inferred
    // `_` return resolves the `'static` return, like a native closure.
    let f = |cb: fn(&str) -> i32| -> &str { if cb("x") > 0 { "a" } else { f(cb) } };
    let g = |cb: fn(&str) -> i32| f(cb);
    assert_eq!(g(|_| 1), "a");
}

#[rec_closure]
#[test]
fn fn_ptr_param_not_miscounted() {
    // One real reference parameter plus a `fn`-pointer parameter: the fn
    // pointer's inner `&str` must not push the count past one, so the `fn`
    // path's elision binds the return to `a`.
    let f = |a: &str, cb: fn(&str) -> i32| -> &str {
        if a.is_empty() || cb(a) > 0 { a } else { f(&a[1..], cb) }
    };
    let is_hi = |s: &str| if s == "hi" { 1 } else { 0 };
    assert_eq!(f("hi", is_hi), "hi");
    assert_eq!(f("no", is_hi), "");
}

#[rec_closure]
#[test]
fn zero_input_elided_return() {
    // Zero parameters with an elided reference return: like a native
    // closure, the `'static` return resolves through the inferred store.
    let c = std::cell::Cell::new(2);
    let f = || -> &str {
        if c.get() == 0 {
            "done"
        } else {
            c.set(c.get() - 1);
            f()
        }
    };
    assert_eq!(f(), "done");
}
