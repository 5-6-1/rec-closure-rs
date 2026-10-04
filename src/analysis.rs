//! Conservative capture analysis. Binding predicates live in `scope.rs`;
//! recursive name detection and replacement share `replace.rs`.

use proc_macro2::Ident;
use std::collections::HashSet;
use syn::{
    ExprClosure, Item, Pat,
    visit::{self, Visit},
};

use crate::scope::{block_items, pat_names};

/// Only unqualified single-segment paths (`f`, `f::<T>`) can name a local.
/// Associated paths (`<T>::f`) and absolute paths are never recursive self.
pub(crate) fn single_ident(expr: &syn::ExprPath) -> Option<&Ident> {
    expr.path
        .segments
        .first()
        .filter(|_| {
            expr.qself.is_none()
                && expr.path.leading_colon.is_none()
                && expr.path.segments.len() == 1
        })
        .map(|segment| &segment.ident)
}

// ---------------------------------------------------------------------------
// capture scanning
// ---------------------------------------------------------------------------

/// Whether the body references any name not bound at its point of use.
/// Order-sensitive: a `let x = 2;` shadowing an earlier `let y = x;` does not
/// cancel the capture reported by `y = x`. `outer_items` holds the in-scope
/// `fn`/`const`/`static` names, which are never captures.
pub(crate) fn captures_external(
    closure: &ExprClosure, self_name: &Ident, outer_items: &HashSet<Ident>,
) -> bool {
    let mut bound: HashSet<Ident> = outer_items.clone();
    bound.extend(closure.inputs.iter().flat_map(pat_names));
    bound.insert(self_name.clone());
    let mut v = CaptureScanner { bound: &mut bound, found: false };
    v.visit_expr(&closure.body);
    v.found
}

/// Order-sensitive capture scan: flags references to names not bound so far
/// (parameters, `let`/`for`/`match` patterns, the recursive name). Block
/// bindings are scoped: restored when their block ends.
struct CaptureScanner<'a> {
    bound: &'a mut HashSet<Ident>,
    found: bool,
}

impl<'a> Visit<'a> for CaptureScanner<'a> {
    fn visit_expr_path(&mut self, p: &'a syn::ExprPath) {
        if let Some(id) = single_ident(p)
            && !self.bound.contains(id)
        {
            self.found = true;
        }
        visit::visit_expr_path(self, p);
    }

    fn visit_pat(&mut self, pat: &'a Pat) {
        self.bound.extend(pat_names(pat));
        visit::visit_pat(self, pat);
    }

    fn visit_macro(&mut self, _: &'a syn::Macro) {
        // Opaque tokens may capture local values. Never infer capture-free
        // merely because their expanded expressions are unavailable here.
        self.found = true;
    }

    fn visit_block(&mut self, block: &'a syn::Block) {
        let saved = self.bound.clone();
        self.bound.extend(block_items(&block.stmts));
        visit::visit_block(self, block);
        *self.bound = saved;
    }

    fn visit_local(&mut self, local: &'a syn::Local) {
        // `let x = x;` reads the *outer* `x` on the right.
        if let Some(init) = &local.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        self.visit_pat(&local.pat);
    }

    fn visit_expr_for_loop(&mut self, node: &'a syn::ExprForLoop) {
        // The iterator is evaluated before the loop variable binds.
        let saved = self.bound.clone();
        self.visit_expr(&node.expr);
        self.visit_pat(&node.pat);
        self.visit_block(&node.body);
        *self.bound = saved;
    }

    fn visit_expr_let(&mut self, node: &'a syn::ExprLet) {
        // `if let pat = expr` evaluates `expr` before binding `pat`; the
        // binding is scoped by the enclosing if/while (see those visitors).
        self.visit_expr(&node.expr);
        self.visit_pat(&node.pat);
    }

    fn visit_expr_if(&mut self, node: &'a syn::ExprIf) {
        // Condition bindings (plain or let-chain) are visible in the then
        // branch; the else branch sees the outer scope.
        let saved = self.bound.clone();
        self.visit_expr(&node.cond);
        self.visit_block(&node.then_branch);
        if let Some((_, els)) = &node.else_branch {
            *self.bound = saved.clone();
            self.visit_expr(els);
        }
        *self.bound = saved;
    }

    fn visit_expr_while(&mut self, node: &'a syn::ExprWhile) {
        let saved = self.bound.clone();
        self.visit_expr(&node.cond);
        self.visit_block(&node.body);
        *self.bound = saved;
    }

    fn visit_expr_match(&mut self, node: &'a syn::ExprMatch) {
        self.visit_expr(&node.expr);
        for arm in &node.arms {
            // Arm-pattern bindings are scoped to the arm only.
            let saved = self.bound.clone();
            self.visit_pat(&arm.pat);
            self.visit_expr(&arm.body);
            *self.bound = saved;
        }
    }

    fn visit_expr_closure(&mut self, node: &'a syn::ExprClosure) {
        // Closure parameters are scoped to the closure body.
        let saved = self.bound.clone();
        for input in &node.inputs {
            self.visit_pat(input);
        }
        self.visit_expr(&node.body);
        *self.bound = saved;
    }

    // Items cannot capture their enclosing environment.
    fn visit_item(&mut self, _: &'a Item) {}
}
