//! Recursion detection: does the closure body reference the recursive
//! binding, ignoring references shadowed by inner bindings?

use proc_macro2::Ident;
use syn::{
    BinOp, Expr, ExprClosure, Item, Stmt,
    visit::{self, Visit},
};

use crate::analysis::{item_binds_name, pat_binds_name, single_ident};

/// Scope-aware scan for references resolving to the recursive binding. The
/// shadowing rules mirror `SelfReplacer`, so detection and rewriting agree:
/// a reference shadowed by an inner binding (a nested closure parameter, a
/// `let`, a match arm, ...) is not recursion.
struct RefFinder<'a> {
    name: &'a Ident,
    found: bool,
    /// An inner binding of `name` is in scope; references resolve to it.
    shadowed: bool,
}

impl<'a> RefFinder<'a> {
    /// A `let` shadows from its own statement on; nested items never
    /// reference the recursive binding.
    fn visit_seq(&mut self, stmts: &'a [Stmt]) {
        let saved = self.shadowed;
        for stmt in stmts {
            self.visit_stmt(stmt);
            if let Stmt::Local(local) = stmt
                && pat_binds_name(&local.pat, self.name)
            {
                self.shadowed = true;
            }
        }
        self.shadowed = saved;
    }

    /// `if let` / `while let` conditions (possibly let chains): whether any
    /// pattern shadows `name` (visible to the right and in the branch).
    fn cond_binds(&mut self, cond: &'a Expr) -> bool {
        match cond {
            Expr::Let(l) => {
                self.visit_expr(&l.expr);
                self.visit_pat(&l.pat);
                pat_binds_name(&l.pat, self.name)
            }
            Expr::Binary(b) if matches!(b.op, BinOp::And(..)) => {
                let left = self.cond_binds(&b.left);
                let saved = self.shadowed;
                if left {
                    self.shadowed = true;
                }
                let right = self.cond_binds(&b.right);
                self.shadowed = saved;
                left || right
            }
            other => {
                self.visit_expr(other);
                false
            }
        }
    }
}

impl<'a> Visit<'a> for RefFinder<'a> {
    fn visit_expr_path(&mut self, p: &'a syn::ExprPath) {
        if !self.shadowed && single_ident(&p.path).is_some_and(|id| id == self.name) {
            self.found = true;
            return;
        }
        visit::visit_expr_path(self, p);
    }

    fn visit_block(&mut self, block: &'a syn::Block) {
        self.visit_seq(&block.stmts);
    }

    fn visit_item(&mut self, node: &'a Item) {
        // Never descend into nested items; one binding the name shadows it.
        if item_binds_name(node, self.name) {
            self.shadowed = true;
        }
    }

    fn visit_expr_closure(&mut self, node: &'a ExprClosure) {
        // Nested closures inherit ambient shadowing; params shadow in-body.
        let saved = self.shadowed;
        for input in &node.inputs {
            self.visit_pat(input);
            if pat_binds_name(input, self.name) {
                self.shadowed = true;
            }
        }
        self.visit_expr(&node.body);
        self.shadowed = saved;
    }

    fn visit_expr_for_loop(&mut self, node: &'a syn::ExprForLoop) {
        // The iterator is evaluated before the loop variable binds.
        self.visit_expr(&node.expr);
        let saved = self.shadowed;
        self.visit_pat(&node.pat);
        if pat_binds_name(&node.pat, self.name) {
            self.shadowed = true;
        }
        self.visit_block(&node.body);
        self.shadowed = saved;
    }

    fn visit_expr_if(&mut self, node: &'a syn::ExprIf) {
        let saved = self.shadowed;
        let cond_binds = self.cond_binds(&node.cond);
        if cond_binds {
            self.shadowed = true;
        }
        self.visit_seq(&node.then_branch.stmts);
        self.shadowed = saved;
        if let Some((_, els)) = &node.else_branch {
            self.visit_expr(els);
        }
    }

    fn visit_expr_while(&mut self, node: &'a syn::ExprWhile) {
        let saved = self.shadowed;
        let cond_binds = self.cond_binds(&node.cond);
        if cond_binds {
            self.shadowed = true;
        }
        self.visit_block(&node.body);
        self.shadowed = saved;
    }

    fn visit_expr_match(&mut self, node: &'a syn::ExprMatch) {
        self.visit_expr(&node.expr);
        for arm in &node.arms {
            // Arm-pattern bindings are scoped to the arm only.
            let saved = self.shadowed;
            if pat_binds_name(&arm.pat, self.name) {
                self.shadowed = true;
            }
            self.visit_pat(&arm.pat);
            self.visit_expr(&arm.body);
            self.shadowed = saved;
        }
    }
}

/// Whether the closure body references `name` in a position that resolves to
/// the recursive binding (shadowed references do not count).
pub(crate) fn body_refers_to(closure: &ExprClosure, name: &Ident) -> bool {
    let mut f = RefFinder { name, found: false, shadowed: false };
    // The closure's own parameters shadow the name inside the body.
    for input in &closure.inputs {
        f.visit_pat(input);
        if pat_binds_name(input, name) {
            f.shadowed = true;
        }
    }
    f.visit_expr(&closure.body);
    f.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn refers(body: &str) -> bool {
        let name = Ident::new("f", proc_macro2::Span::call_site());
        body_refers_to(&parse_str::<ExprClosure>(body).unwrap(), &name)
    }

    #[test]
    fn shadowed_params_are_not_recursion() {
        assert!(!refers("|f| f + 1")); // own parameter
        assert!(!refers("|n| { let g = |f| f * 2; g(n) }")); // nested param
        assert!(!refers("|n| { let f = 2; n + f }")); // `let` shadow
    }

    #[test]
    fn unshadowed_calls_are_recursion() {
        assert!(refers("|n| if n <= 1 { 1 } else { n * f(n - 1) }"));
        // Recursion *after* a shadowed nested closure still counts.
        assert!(refers("|n| { let g = |f| f * 2; g(f(n - 1)) }"));
    }
}
