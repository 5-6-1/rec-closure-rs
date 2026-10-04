//! Conditional bindings must gate all generated statements and diagnostics.

use rec_closure::rec_closure;

#[rec_closure]
#[test]
fn disabled_inferred_closure_emits_no_scaffolding() {
    #[cfg(any())]
    let f = |n| if n == 0 { undefined_value } else { f(n - 1) };

    #[cfg_attr(all(), cfg(any()))]
    let g = |n| if n == 0 { also_undefined } else { g(n - 1) };
}

#[rec_closure]
#[test]
fn disabled_closure_emits_no_diagnostic() {
    #[cfg(any())]
    let f = async |n| f(n).await;
}

#[rec_closure]
#[test]
fn disabled_parent_gates_nested_diagnostics() {
    #[cfg(any())]
    let f = |n| {
        let g = async |n| g(n).await;
        if n == 0 { 1 } else { f(n - 1) }
    };
}

#[rec_closure]
#[test]
fn nested_cfg_attr_preserves_lint_expectations() {
    #[cfg_attr(all(), cfg_attr(all(), cfg(any())), expect(unused_variables))]
    let f = async |n| f(n).await;

    #[cfg_attr(all(), expect(unused_variables))]
    let g = |n: i32| if n == 0 { 1 } else { g(n - 1) };
}

#[rec_closure]
#[test]
fn disabled_items_and_blocks_gate_nested_diagnostics() {
    #[cfg(any())]
    fn disabled() {
        let f = async |n| f(n).await;
    }

    #[cfg(any())]
    {
        let f = async |n| f(n).await;
    }
}
