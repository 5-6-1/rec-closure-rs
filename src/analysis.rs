//! Name analysis: recursion detection, order-sensitive capture scanning, and
//! shadowing predicates shared by the capture scan and the self-reference
//! rewriting.

use proc_macro2::Ident;
use std::collections::HashSet;
use syn::{
    ExprClosure, Item, Pat,
    visit::{self, Visit},
};

use crate::pat_ident;

/// The identifier of a single-segment path (`f`, `f::<T>`), or `None` for
/// multi-segment paths (`a::f`), which never denote a local binding.
pub(crate) fn single_ident(path: &syn::Path) -> Option<&Ident> {
    (path.segments.len() == 1).then(|| &path.segments[0].ident)
}

// ---------------------------------------------------------------------------
// recursion detection
// ---------------------------------------------------------------------------

struct RefFinder<'a> {
    name: &'a Ident,
    found: bool,
}

impl<'a> Visit<'a> for RefFinder<'a> {
    fn visit_expr_path(&mut self, p: &'a syn::ExprPath) {
        if single_ident(&p.path).is_some_and(|id| id == self.name) {
            self.found = true;
        }
        visit::visit_expr_path(self, p);
    }
}

pub(crate) fn body_refers_to(closure: &ExprClosure, name: &Ident) -> bool {
    let mut f = RefFinder { name, found: false };
    f.visit_expr(&closure.body);
    f.found
}

// ---------------------------------------------------------------------------
// capture scanning
// ---------------------------------------------------------------------------

/// Whether the closure body references any name that is not yet bound at the
/// point of use. Order-sensitive: a `let x = 2;` that shadows an earlier
/// external use of `x` does not cancel it out, because `let y = x;` is seen
/// first and reports the capture immediately.
pub(crate) fn captures_external(closure: &ExprClosure, self_name: &Ident) -> bool {
    let mut bound: HashSet<Ident> = closure.inputs.iter().filter_map(pat_ident).cloned().collect();
    bound.insert(self_name.clone());
    let mut v = CaptureScanner { bound: &mut bound, found: false };
    v.visit_expr(&closure.body);
    v.found
}

/// Order-sensitive capture scan: tracks the names bound so far (parameters,
/// `let`/`for`/`match` patterns, the recursive name) and flags any reference
/// to a name not in that set. Block-local bindings are scoped: they are
/// restored when the block ends.
struct CaptureScanner<'a> {
    bound: &'a mut HashSet<Ident>,
    found: bool,
}

impl<'a> Visit<'a> for CaptureScanner<'a> {
    fn visit_expr_path(&mut self, p: &'a syn::ExprPath) {
        if let Some(id) = single_ident(&p.path)
            && !self.bound.contains(id)
        {
            self.found = true;
        }
        visit::visit_expr_path(self, p);
    }

    fn visit_pat(&mut self, pat: &'a Pat) {
        if let Pat::Ident(pi) = pat {
            self.bound.insert(pi.ident.clone());
        }
        visit::visit_pat(self, pat);
    }

    fn visit_block(&mut self, block: &'a syn::Block) {
        let saved = self.bound.clone();
        visit::visit_block(self, block);
        *self.bound = saved;
    }

    fn visit_local(&mut self, local: &'a syn::Local) {
        // `let pat = init` evaluates `init` before binding `pat`, so
        // `let x = x;` reads the *outer* `x` on the right.
        if let Some(init) = &local.init {
            self.visit_expr(&init.expr);
        }
        self.visit_pat(&local.pat);
    }

    fn visit_expr_for_loop(&mut self, node: &'a syn::ExprForLoop) {
        // The iterator expression is evaluated before the loop variable is
        // bound; the variable is scoped to the loop body only.
        let saved = self.bound.clone();
        self.visit_expr(&node.expr);
        self.visit_pat(&node.pat);
        self.visit_block(&node.body);
        *self.bound = saved;
    }

    fn visit_expr_let(&mut self, node: &'a syn::ExprLet) {
        // `if let pat = expr` evaluates `expr` before binding `pat`. The
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
            // Arm-pattern bindings are scoped to the arm (pattern, guard,
            // body) and must not leak into later arms.
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

    fn visit_item_fn(&mut self, node: &'a syn::ItemFn) {
        // A nested `fn` cannot capture; its parameters and body bindings are
        // scoped inside it.
        let saved = self.bound.clone();
        visit::visit_item_fn(self, node);
        *self.bound = saved;
    }
}

// ---------------------------------------------------------------------------
// shadowing predicates
// ---------------------------------------------------------------------------

/// Whether `pat` introduces a binding of `name` anywhere inside it (`f`,
/// `mut f`, `Some(f)`, `(f, _)`, `[f, ..]`, `f @ _`, `ref f`, `f: T`, ...).
/// A syn 3 match guard (`Pat::Guard`) binds through its inner pattern.
pub(crate) fn pat_binds_name(pat: &Pat, name: &Ident) -> bool {
    let mut v = PatBinder { name, found: false };
    v.visit_pat(pat);
    v.found
}

struct PatBinder<'a> {
    name: &'a Ident,
    found: bool,
}

impl<'a> Visit<'a> for PatBinder<'a> {
    fn visit_pat(&mut self, pat: &'a Pat) {
        if let Pat::Ident(p) = pat
            && p.ident == *self.name
        {
            self.found = true;
        }
        visit::visit_pat(self, pat);
    }
}

/// Whether a nested item binds `name` in the value namespace (`const f`,
/// `static f`, `fn f`), shadowing the recursive name from that point on.
pub(crate) fn item_binds_name(item: &Item, name: &Ident) -> bool {
    match item {
        Item::Const(c) => c.ident == *name,
        Item::Static(s) => s.ident == *name,
        Item::Fn(f) => f.sig.ident == *name,
        _ => false,
    }
}
