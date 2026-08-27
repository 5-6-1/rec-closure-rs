//! Scope-aware rewriting of the recursive self reference inside a closure
//! body. Bindings that shadow the recursive name (locally or through
//! control-flow patterns, items, and nested closures) are tracked, and every
//! reference that still resolves to the recursive binding is replaced.

use proc_macro2::Ident;
use syn::{
    BinOp, Expr, ExprClosure, Item, Stmt,
    visit_mut::{self, VisitMut},
};

use crate::analysis::{item_binds_name, pat_binds_name, single_ident};

struct SelfReplacer<'a> {
    from: &'a Ident,
    to: Ident,
    /// An inner binding of `from` is currently in scope: references seen in
    /// this state resolve to the inner binding and must be left alone.
    shadowed: bool,
}

impl<'a> SelfReplacer<'a> {
    fn new(from: &'a Ident, to: &Ident) -> Self {
        Self { from, to: to.clone(), shadowed: false }
    }

    /// Statements in sequence: a `let` shadows from its own statement to the
    /// end of the enclosing block. Nested `item`s are skipped entirely —
    /// they cannot capture the recursive binding, so every occurrence of the
    /// name inside them resolves to something else.
    fn visit_seq(&mut self, stmts: &mut [Stmt]) {
        let saved = self.shadowed;
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
    fn visit_expr_path_mut(&mut self, p: &mut syn::ExprPath) {
        if !self.shadowed && single_ident(&p.path).is_some_and(|id| id == self.from) {
            p.path = syn::Path::from(self.to.clone());
            return;
        }
        visit_mut::visit_expr_path_mut(self, p);
    }

    fn visit_block_mut(&mut self, node: &mut syn::Block) {
        self.visit_seq(&mut node.stmts);
    }

    fn visit_expr_block_mut(&mut self, node: &mut syn::ExprBlock) {
        self.visit_seq(&mut node.block.stmts);
    }

    fn visit_item_mut(&mut self, node: &mut Item) {
        // Never rewrite inside nested items (fn/static/...): they cannot
        // capture the recursive binding. A `const`/`static`/`fn` with the
        // recursive name shadows it from this point on.
        if item_binds_name(node, self.from) {
            self.shadowed = true;
        }
    }

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
    SelfReplacer::new(from, to).visit_expr_mut(body);
}
