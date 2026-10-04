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
//! rewritten into a recursive closure. Complete signatures expressible by
//! local items enable zero-allocation expansion (a local `fn` or a
//! `&dyn HideFn` self-parameter); other signatures use `Rc + OnceCell + Weak` with
//! inferred types. `#[rec_closure(sync)]` switches to the multithreaded
//! expansion (`Arc + OnceLock`, `Send + Sync`).
//!
//! Generated names use mixed-site hygiene and per-function numbered families,
//! skipping families already present in user tokens, including lifetimes.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{ToTokens, quote};
use std::collections::HashSet;
use syn::{Expr, ExprClosure, ItemFn, Pat, Stmt, parse_macro_input, visit_mut::VisitMut};

mod analysis;
mod attributes;
mod context;
mod expand;
mod names;
mod replace;
mod scope;
mod signature;
mod store;
mod validate;
mod zero_alloc;

use crate::expand::expand_closure;
use crate::names::role_ident;
use crate::replace::{body_refers_to, replace_self_ref};
use crate::scope::{block_items, pat_names};

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

    let mut ctx = Ctx {
        counter: 0,
        reserved: names::reserved_families(item.to_token_stream()),
        sync,
        errors: Vec::new(),
        items: HashSet::new(),
        generics: signature::generic_names(&item.sig.generics),
    };
    expand_stmts(&mut item.block.stmts, &mut ctx);
    let mut all = ctx.errors.into_iter();
    if let Some(mut combined) = all.next() {
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
    pub(crate) reserved: HashSet<usize>,
    pub(crate) sync: bool,
    // Every failed expansion parks its diagnostic here while the offending
    // statement stays unexpanded, so rustc reports one error per bad closure
    // instead of stopping at the first.
    pub(crate) errors: Vec<syn::Error>,
    // Value-namespace item names in scope (`fn`/`const`/`static`): references
    // to them are never captures, so capture analysis can skip them.
    pub(crate) items: HashSet<Ident>,
    // Local generated items cannot inherit these enclosing parameters.
    pub(crate) generics: HashSet<Ident>,
}

// ---------------------------------------------------------------------------
// statement expansion
// ---------------------------------------------------------------------------

fn expand_stmts(stmts: &mut Vec<Stmt>, ctx: &mut Ctx) {
    let saved = ctx.items.clone();
    ctx.items.extend(block_items(stmts));
    let mut out = Vec::with_capacity(stmts.len());
    for mut stmt in stmts.drain(..) {
        let error_start = ctx.errors.len();
        let bindings = match &stmt {
            Stmt::Local(local) => pat_names(&local.pat),
            _ => HashSet::new(),
        };
        if let Some((name, closure)) = as_recursive_let(&stmt) {
            match expand_closure(name, closure, ctx)
                .and_then(|expanded| attributes::inherit(&stmt, expanded))
            {
                Ok(expanded) => out.extend(expanded),
                Err(e) => {
                    ctx.errors.push(e);
                    out.push(stmt);
                }
            }
        } else {
            ctx.visit_stmt_mut(&mut stmt);
            out.push(stmt);
        }
        // The final emitted statement retains the original attributes.
        // Only inspect its prefix when diagnostics need configuration;
        // successful expansions need no extra tokenization or parsing.
        let cfg = if ctx.errors.len() > error_start {
            out.last().map(attributes::statement_configuration).unwrap_or_default()
        } else {
            Vec::new()
        };
        if !cfg.is_empty() {
            // Let rustc evaluate cfg, including features and cfg_attr. A
            // disabled parent statement also suppresses nested diagnostics.
            let errors: Vec<_> = ctx.errors.drain(error_start..).collect();
            for error in errors {
                let error = error.into_compile_error();
                match syn::parse2(quote!(#(#cfg)* { #error })) {
                    Ok(error) => out.push(error),
                    Err(error) => ctx.errors.push(error),
                }
            }
        }
        // Initializers see the previous binding; following statements see
        // the new local, which may shadow a previously visible item.
        ctx.items.retain(|id| !bindings.contains(id));
    }
    *stmts = out;
    ctx.items = saved;
}

/// A `let name = <closure>;` statement whose body references `name`.
fn as_recursive_let(stmt: &Stmt) -> Option<(&Ident, &ExprClosure)> {
    let Stmt::Local(local) = stmt else { return None };
    let name = pat_ident(&local.pat)?;
    let init = local.init.as_ref()?;
    let Expr::Closure(closure) = &*init.expr else { return None };
    body_refers_to(closure, name).then_some((name, closure))
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

/// The user closure body with self references rewritten and nested recursive
/// closures expanded. Failures inside nested expansions are parked in
/// `ctx.errors` and reported at the end; the body is still returned so the
/// surrounding expansion stays structurally valid.
pub(crate) fn prepare_body(
    closure: &ExprClosure, name: &Ident, self_id: &Ident, ctx: &mut Ctx,
) -> Expr {
    let mut body = *closure.body.clone();
    replace_self_ref(&mut body, name, self_id);
    ctx.expand_body(closure, &mut body);
    body
}
