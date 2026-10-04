//! Scope-aware rewriting of the recursive self reference inside a closure
//! body. Bindings that shadow the recursive name (locally or through
//! control-flow patterns, items, and nested closures) are tracked, and every
//! reference that still resolves to the recursive binding is replaced.

use proc_macro2::Ident;
use syn::{
    BinOp, Expr, ExprClosure, Item, Stmt,
    visit_mut::{self, VisitMut},
};

use crate::{
    analysis::single_ident,
    scope::{block_items, pat_binds_name},
};

struct SelfReplacer<'a> {
    from: &'a Ident,
    to: Option<&'a Ident>,
    found: bool,
    /// An inner binding of `from` is currently in scope: references seen in
    /// this state resolve to the inner binding and must be left alone.
    shadowed: bool,
}

impl<'a> SelfReplacer<'a> {
    fn new(from: &'a Ident, to: Option<&'a Ident>) -> Self {
        Self { from, to, found: false, shadowed: false }
    }

    /// Statements in sequence: a `let` shadows from its own statement to the
    /// end of the enclosing block. Nested `item`s are skipped entirely —
    /// they cannot capture the recursive binding, so every occurrence of the
    /// name inside them resolves to something else.
    fn visit_seq(&mut self, stmts: &mut [Stmt]) {
        let saved = self.shadowed;
        self.shadowed |= block_items(stmts).contains(self.from);
        for stmt in stmts {
            self.visit_stmt_mut(stmt);
            if let Stmt::Local(local) = stmt
                && pat_binds_name(&local.pat, self.from)
            {
                self.shadowed = true;
            }
        }
        self.shadowed = saved;
    }

    /// An `if let` / `while let` condition, possibly a let chain (`a && b`):
    /// evaluate it with the outer bindings, report whether any of its
    /// patterns shadows `name`. In a chain, a left binding is visible to the
    /// right and to the then-branch, matching Rust 2024 let-chain semantics.
    fn cond_binds(&mut self, cond: &mut Expr) -> bool {
        match cond {
            Expr::Let(l) => {
                self.visit_expr_mut(&mut l.expr);
                self.visit_pat_mut(&mut l.pat);
                pat_binds_name(&l.pat, self.from)
            }
            Expr::Binary(b) if matches!(b.op, BinOp::And(..)) => {
                let left = self.cond_binds(&mut b.left);
                let saved = self.shadowed;
                if left {
                    self.shadowed = true;
                }
                let right = self.cond_binds(&mut b.right);
                self.shadowed = saved;
                left || right
            }
            other => {
                self.visit_expr_mut(other);
                false
            }
        }
    }
}

impl VisitMut for SelfReplacer<'_> {
    fn visit_field_value_mut(&mut self, field: &mut syn::FieldValue) {
        let rewritten_shorthand = field.colon_token.is_none()
            && self.to.is_some()
            && !self.shadowed
            && matches!(&field.expr, Expr::Path(path) if single_ident(path) == Some(self.from));
        visit_mut::visit_field_value_mut(self, field);
        if rewritten_shorthand {
            // `Holder { f }` must become `Holder { f: generated_self }`:
            // changing only the expression leaves shorthand printing `f`.
            field.colon_token = Some(Default::default());
        }
    }

    fn visit_expr_path_mut(&mut self, p: &mut syn::ExprPath) {
        if !self.shadowed && single_ident(p).is_some_and(|id| id == self.from) {
            self.found = true;
            if let Some(to) = self.to {
                // Keep generic arguments so invalid turbofish syntax is
                // still rejected by rustc on every expansion path.
                if let Some(segment) = p.path.segments.first_mut() {
                    segment.ident = to.clone();
                }
            }
        }
        visit_mut::visit_expr_path_mut(self, p);
    }

    fn visit_block_mut(&mut self, node: &mut syn::Block) {
        self.visit_seq(&mut node.stmts);
    }

    fn visit_expr_block_mut(&mut self, node: &mut syn::ExprBlock) {
        self.visit_seq(&mut node.block.stmts);
    }

    // Item names were collected at block entry; their bodies cannot capture.
    fn visit_item_mut(&mut self, _: &mut Item) {}

    fn visit_expr_closure_mut(&mut self, node: &mut ExprClosure) {
        // A nested closure captures lexically, so it inherits the ambient
        // shadowing; its own parameters shadow within its body.
        let saved = self.shadowed;
        for input in &mut node.inputs {
            self.visit_pat_mut(input);
            if pat_binds_name(input, self.from) {
                self.shadowed = true;
            }
        }
        self.visit_expr_mut(&mut node.body);
        self.shadowed = saved;
    }

    fn visit_expr_for_loop_mut(&mut self, node: &mut syn::ExprForLoop) {
        // The iterator expression is evaluated before the loop variable binds.
        self.visit_expr_mut(&mut node.expr);
        let saved = self.shadowed;
        self.visit_pat_mut(&mut node.pat);
        if pat_binds_name(&node.pat, self.from) {
            self.shadowed = true;
        }
        self.visit_block_mut(&mut node.body);
        self.shadowed = saved;
    }

    fn visit_expr_if_mut(&mut self, node: &mut syn::ExprIf) {
        let saved = self.shadowed;
        let cond_binds = self.cond_binds(&mut node.cond);
        if cond_binds {
            self.shadowed = true;
        }
        self.visit_seq(&mut node.then_branch.stmts);
        self.shadowed = saved;
        if let Some((_, els)) = &mut node.else_branch {
            self.visit_expr_mut(els);
        }
    }

    fn visit_expr_while_mut(&mut self, node: &mut syn::ExprWhile) {
        let saved = self.shadowed;
        let cond_binds = self.cond_binds(&mut node.cond);
        if cond_binds {
            self.shadowed = true;
        }
        self.visit_block_mut(&mut node.body);
        self.shadowed = saved;
    }

    fn visit_expr_match_mut(&mut self, node: &mut syn::ExprMatch) {
        self.visit_expr_mut(&mut node.expr);
        for arm in &mut node.arms {
            let saved = self.shadowed;
            // In syn 3 the match guard is part of the arm pattern
            // (`Pat::Guard`); its bindings are active in guard and body
            // alike, so mark before visiting either.
            if pat_binds_name(&arm.pat, self.from) {
                self.shadowed = true;
            }
            self.visit_pat_mut(&mut arm.pat);
            self.visit_expr_mut(&mut arm.body);
            self.shadowed = saved;
        }
    }
}

pub(crate) fn replace_self_ref(body: &mut Expr, from: &Ident, to: &Ident) {
    SelfReplacer::new(from, Some(to)).visit_expr_mut(body);
}

/// Detection uses the same scope traversal as rewriting. A scratch AST lets
/// the read-only query share the visitor without maintaining a second set
/// of name-resolution rules.
pub(crate) fn body_refers_to(closure: &ExprClosure, name: &Ident) -> bool {
    let mut scanner = SelfReplacer::new(name, None);
    scanner.visit_expr_closure_mut(&mut closure.clone());
    scanner.found
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
        assert!(!refers("|f| f + 1"));
        assert!(!refers("|n| { let g = |f| f * 2; g(n) }"));
        assert!(!refers("|n| { let f = 2; n + f }"));
    }

    #[test]
    fn unshadowed_calls_are_recursion() {
        assert!(refers("|n| if n <= 1 { 1 } else { n * f(n - 1) }"));
        assert!(refers("|n| { let g = |f| f * 2; g(f(n - 1)) }"));
    }

    #[test]
    fn block_items_shadow_before_declaration() {
        assert!(!refers("|n| { let v = f(n); fn f(n: i32) -> i32 { n } v }"));
        assert!(!refers("|n| { let v = f; const f: i32 = 3; v + n }"));
        assert!(!refers("|n| { let v = f; static f: i32 = 3; v + n }"));
    }

    #[test]
    fn qualified_paths_are_not_self_references() {
        assert!(!refers("|n| module::f(n)"));
        assert!(!refers("|n| <T>::f(n)"));
        assert!(!refers("|n| ::f(n)"));
    }
}
