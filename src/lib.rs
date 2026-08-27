//! Procedural macro crate generating recursive closures.
//!
//! # Goal syntax
//!
//! ```
//! use rec_closure::rec_closure;
//!
//! #[rec_closure]
//! fn main() {
//!     let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };
//!     println!("{}", fact(5)); // 120
//! }
//! ```
//!
//! A closure bound with `let name = ...` whose body references `name` is
//! rewritten into a recursive closure. With complete type annotations the
//! expansion is zero-allocation (a `&dyn HideFn` self-parameter, Y-combinator
//! style); without them it falls back to `Rc + OnceCell + Weak` with fully
//! inferred types. `#[rec_closure(sync)]` switches to the multithreaded
//! expansion (`Arc + OnceLock`, `Send + Sync`).
//!
//! Every generated identifier carries a per-fn sequence number
//! (`__rec_{N}_{role}`), so several closures — including nested ones — never
//! collide.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::{
    Expr, ExprClosure, ItemFn, Pat, Stmt, parse_macro_input,
    visit_mut::{self, VisitMut},
};

mod analysis;
mod expand;
mod replace;
mod store;
mod zero_alloc;

use crate::analysis::body_refers_to;
use crate::expand::expand_closure;
use crate::replace::replace_self_ref;

// ---------------------------------------------------------------------------
// attribute entry point
// ---------------------------------------------------------------------------

/// Converts `let name = <closure>;` bindings whose bodies reference `name`
/// into recursive closures.
///
/// Optional argument `sync` switches to the multithreaded expansion.
#[proc_macro_attribute]
pub fn rec_closure(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut sync = false;
    let parse = syn::meta::parser(|meta: syn::meta::ParseNestedMeta| {
        if meta.path.is_ident("sync") {
            sync = true;
            Ok(())
        } else {
            Err(meta.error("expected `sync`"))
        }
    });
    parse_macro_input!(args with parse);
    let mut item = parse_macro_input!(input as ItemFn);

    let mut ctx = Ctx { counter: 0, sync, errors: Vec::new(), active: Vec::new() };
    expand_stmts(&mut item.block.stmts, &mut ctx);
    if !ctx.errors.is_empty() {
        let mut all = ctx.errors.into_iter();
        let mut combined = all.next().expect("checked non-empty");
        for e in all {
            combined.combine(e);
        }
        return combined.into_compile_error().into();
    }

    quote!(#item).into()
}

// ---------------------------------------------------------------------------
// expansion context
// ---------------------------------------------------------------------------

pub(crate) struct Ctx {
    pub(crate) counter: usize,
    pub(crate) sync: bool,
    // Every failed expansion parks its diagnostic here while the offending
    // statement stays unexpanded, so rustc reports one error per bad closure
    // instead of stopping at the first.
    pub(crate) errors: Vec<syn::Error>,
    // Names of recursive closures currently being expanded (nested-recursion
    // detection).
    pub(crate) active: Vec<Ident>,
}

impl VisitMut for Ctx {
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        expand_stmts(&mut block.stmts, self);
        visit_mut::visit_block_mut(self, block);
    }

    fn visit_item_fn_mut(&mut self, node: &mut ItemFn) {
        // A nested `fn` is an independent scope (it cannot capture the outer
        // bindings): a recursive closure inside it may reuse an enclosing
        // closure's name, so the active-names stack must not leak in.
        let saved = std::mem::take(&mut self.active);
        visit_mut::visit_item_fn_mut(self, node);
        self.active = saved;
    }
}

impl Ctx {
    /// Recursively expand recursive closures inside every block of `expr`
    /// (including nested closure bodies).
    pub(crate) fn expand_expr_blocks(&mut self, expr: &mut Expr) {
        self.visit_expr_mut(expr);
    }
}

// ---------------------------------------------------------------------------
// statement expansion
// ---------------------------------------------------------------------------

fn expand_stmts(stmts: &mut Vec<Stmt>, ctx: &mut Ctx) {
    let mut out = Vec::with_capacity(stmts.len());
    for mut stmt in stmts.drain(..) {
        if let Some((name, closure)) = as_recursive_let(&stmt) {
            match expand_closure(name, closure, ctx) {
                Ok(expanded) => out.extend(expanded),
                Err(e) => {
                    ctx.errors.push(e);
                    out.push(stmt);
                }
            }
            continue;
        }
        ctx.visit_stmt_mut(&mut stmt);
        out.push(stmt);
    }
    *stmts = out;
}

/// A `let name = <closure>;` statement whose body references `name`.
fn as_recursive_let(stmt: &Stmt) -> Option<(&Ident, &ExprClosure)> {
    let Stmt::Local(local) = stmt else { return None };
    let name = pat_ident(&local.pat)?;
    let init = local.init.as_ref()?;
    let Expr::Closure(closure) = strip_parens(&init.expr) else { return None };
    body_refers_to(closure, name).then_some((name, closure))
}

/// Strip transparent parentheses / invisible delimiters so that
/// `let f = (|n| ...);` and `let f = ((|n| ...));` are recognized too.
fn strip_parens(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(p) => strip_parens(&p.expr),
        Expr::Group(g) => strip_parens(&g.expr),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// pattern and body helpers
// ---------------------------------------------------------------------------

/// The identifier bound by a pattern (`n`, `mut n`, `n: i32`); no `ref`, no
/// `@`.
pub(crate) fn pat_ident(pat: &Pat) -> Option<&Ident> {
    match pat {
        Pat::Ident(p) if p.by_ref.is_none() && p.subpat.is_none() => Some(&p.ident),
        Pat::Type(pt) => pat_ident(&pt.pat),
        _ => None,
    }
}

pub(crate) fn role_ident(n: usize, role: &str) -> Ident {
    format_ident!("__rec_{}_{}", n, role)
}

/// The user closure body with self references rewritten and nested recursive
/// closures expanded. Failures inside nested expansions are parked in
/// `ctx.errors` and reported at the end; the body is still returned so the
/// surrounding expansion stays structurally valid.
pub(crate) fn prepare_body(
    closure: &ExprClosure, name: &Ident, self_id: &Ident, ctx: &mut Ctx,
) -> Expr {
    let mut body = *closure.body.clone();
    replace_self_ref(&mut body, name, self_id);
    ctx.expand_expr_blocks(&mut body);
    body
}
