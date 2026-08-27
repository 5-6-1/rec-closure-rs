//! Path selection and the `fn` / inferred (Rc) expansions.

use proc_macro2::Ident;
use syn::{
    ExprClosure, Lifetime, Pat, ReturnType, Stmt, Type, parse_quote,
    visit::{self, Visit},
};

use crate::{
    Ctx,
    analysis::captures_external,
    pat_ident, prepare_body, role_ident,
    store::{DynStore, erased_params},
    zero_alloc::expand_zero_alloc,
};

// ---------------------------------------------------------------------------
// typed signature extraction
// ---------------------------------------------------------------------------

/// Extracted, fully-typed closure signature (zero-allocation path) or `None`
/// (fall back to the inferred path).
pub(crate) struct TypedArgs {
    /// User parameter patterns (including `mut`).
    pub(crate) pats: Vec<Pat>,
    /// Parameter types.
    pub(crate) tys: Vec<Type>,
    /// Parameter identifiers.
    pub(crate) names: Vec<Ident>,
    /// Return type.
    pub(crate) ret: Type,
}

fn typed_args(closure: &ExprClosure) -> Option<TypedArgs> {
    let (pats, tys): (Vec<Pat>, Vec<Type>) = closure
        .inputs
        .iter()
        .map(|pat| {
            let Pat::Type(pt) = pat else { return None };
            Some(((*pt.pat).clone(), (*pt.ty).clone()))
        })
        .collect::<Option<Vec<(Pat, Type)>>>()?
        .into_iter()
        .unzip();
    let names: Vec<Ident> = pats.iter().map(|p| pat_ident(p).cloned()).collect::<Option<_>>()?;
    let ret = match &closure.output {
        ReturnType::Type(_, ty) => (**ty).clone(),
        ReturnType::Default => return None,
    };
    Some(TypedArgs { pats, tys, names, ret })
}

// ---------------------------------------------------------------------------
// path selection
// ---------------------------------------------------------------------------

/// How an annotated return type uses elided (anonymous) reference lifetimes:
/// not at all, as a plain top-level reference (`&str`, whose elision the
/// inferred store can resolve), or nested inside a container (`Option<&str>`)
/// where neither expansion path can express it.
enum RetRef {
    None,
    TopLevel,
    Nested,
}

/// A reference lifetime is elided if it is omitted (`&str`) or written as the
/// anonymous `'_` (`&'_ str`) — both resolve through Rust's elision rules and
/// cannot be lifted to an explicit lifetime by the macro.
fn is_elided(lt: &Option<Lifetime>) -> bool {
    match lt {
        None => true,
        Some(l) => l.ident == "_",
    }
}

fn ret_elided_ref(ret: &Type) -> RetRef {
    match ret {
        Type::Reference(r) if is_elided(&r.lifetime) => {
            if type_has_elided_ref(&r.elem) {
                RetRef::Nested
            } else {
                RetRef::TopLevel
            }
        }
        other => {
            if type_has_elided_ref(other) {
                RetRef::Nested
            } else {
                RetRef::None
            }
        }
    }
}

/// Whether any reference with an elided (or `'_`) lifetime occurs anywhere
/// in `ty`.
fn type_has_elided_ref(ty: &Type) -> bool {
    let mut v = ElidedRefFinder { found: false };
    v.visit_type(ty);
    v.found
}

struct ElidedRefFinder {
    found: bool,
}

impl<'a> Visit<'a> for ElidedRefFinder {
    fn visit_type_reference(&mut self, ty: &'a syn::TypeReference) {
        if is_elided(&ty.lifetime) {
            self.found = true;
        }
        visit::visit_type_reference(self, ty);
    }
}

/// Whether any reference occurs anywhere in `ty` (recursively), so container
/// parameters like `Vec<&mut T>` count as reference parameters too.
fn type_has_ref(ty: &Type) -> bool {
    let mut v = AnyRefFinder { found: false };
    v.visit_type(ty);
    v.found
}

struct AnyRefFinder {
    found: bool,
}

impl<'a> Visit<'a> for AnyRefFinder {
    fn visit_type_reference(&mut self, _: &'a syn::TypeReference) {
        self.found = true;
    }
}

pub(crate) fn expand_closure(
    name: &Ident, closure: &ExprClosure, ctx: &mut Ctx,
) -> syn::Result<Vec<Stmt>> {
    let n = ctx.counter;
    ctx.counter += 1;

    // A nested recursive closure with the same name would make the rewrite
    // drift between expansion paths (its body's `name` is either the outer
    // self or the inner fn, depending on the path) — refuse loudly instead
    // of silently changing behavior.
    if ctx.active.iter().any(|a| a == name) {
        return Err(syn::Error::new_spanned(
            closure,
            "a nested recursive closure cannot reuse the name of an enclosing \
             recursive closure; rename one of them",
        ));
    }
    if closure.asyncness.is_some() {
        return Err(syn::Error::new_spanned(closure, "async recursive closures are not supported"));
    }

    ctx.active.push(name.clone());
    let result = match typed_args(closure) {
        Some(ta) => match ret_elided_ref(&ta.ret) {
            // Container-nested elided references (`Option<&str>`) cannot be
            // expressed by any expansion path (zero-alloc: E0106 in the `Fn`
            // bound; inferred: the `_` return is polluted by constructor
            // argument inference). Refuse with a hint.
            RetRef::Nested => Err(syn::Error::new_spanned(
                closure,
                "the return type contains references with elided lifetimes \
                 nested inside a container (e.g. `Option<&str>`); use explicit \
                 lifetimes such as `Option<&'static str>`",
            )),
            // A plain top-level reference return (`&str` / `&'_ str`) cannot
            // go through the zero-alloc `Fn(&dyn HideFn, ...) -> ...` bound
            // (E0106, the elision rule picks `&dyn HideFn`). With exactly one
            // reference parameter:
            // - capture-free: the `fn` path's own elision rule resolves the
            //   return borrowing from that parameter — the best lowering.
            // - capturing: the inferred `_` return only resolves a `'static`
            //   return; a borrow return then fails in rustc (E0623) with no
            //   better option on stable — documented in the README.
            // Any other arity (zero or multiple reference inputs) cannot be
            // expressed by any path; refuse with a hint.
            RetRef::TopLevel => {
                let ref_params = ta.tys.iter().filter(|t| type_has_ref(t)).count();
                if ref_params == 1 {
                    if !captures_external(closure, name) {
                        Ok(expand_fn(name, closure, &ta, ctx))
                    } else {
                        Ok(expand_inferred(name, closure, n, ctx))
                    }
                } else {
                    Err(syn::Error::new_spanned(
                        closure,
                        "an elided reference return (`-> &str`) needs exactly one \
                         reference parameter to borrow from; use explicit \
                         lifetimes such as `-> &'static str`",
                    ))
                }
            }
            // No elided reference in the return: normal path selection.
            RetRef::None => {
                if !captures_external(closure, name) {
                    Ok(expand_fn(name, closure, &ta, ctx))
                } else {
                    Ok(expand_zero_alloc(name, closure, &ta, n, ctx))
                }
            }
        },
        None => Ok(expand_inferred(name, closure, n, ctx)),
    };
    ctx.active.pop();
    result
}

// ---------------------------------------------------------------------------
// fn path
// ---------------------------------------------------------------------------

/// Lower a capture-free, fully annotated recursive closure to a local `fn`.
fn expand_fn(name: &Ident, closure: &ExprClosure, ta: &TypedArgs, ctx: &mut Ctx) -> Vec<Stmt> {
    let TypedArgs { pats, tys, ret, .. } = ta;
    // The body keeps referencing `name`; inside the `fn` that is the
    // recursion. Only nested recursive closures need expansion.
    let mut body = *closure.body.clone();
    ctx.expand_expr_blocks(&mut body);
    // Keep the `let name = ...` shape of every path; the fn lives inside the
    // block (so only the variable occupies the outer scope) and the tail
    // `name` refers to the fn item.
    vec![parse_quote! {
        let #name = {
            fn #name(#(#pats: #tys),*) -> #ret {
                #body
            }
            #name
        };
    }]
}

// ---------------------------------------------------------------------------
// inferred path: Rc + OnceCell + Weak
// ---------------------------------------------------------------------------

fn expand_inferred(name: &Ident, closure: &ExprClosure, n: usize, ctx: &mut Ctx) -> Vec<Stmt> {
    let mov = closure.capture;
    let params: Vec<Pat> = closure.inputs.iter().cloned().collect();
    let (param_tys, for_clause) = erased_params(&closure.inputs);
    let store = DynStore::new(ctx.sync, &param_tys, &for_clause);
    let DynStore { rc, weak_mod, cell_ty, cell_new, self_ty, dyn_ty } = &store;

    let slot = role_ident(n, "slot");
    let slot_ref = role_ident(n, "slot_ref");
    let rec = role_ident(n, "rec");
    let weak = role_ident(n, "weak");
    let strong = role_ident(n, "strong");
    let self_id = role_ident(n, "self");

    let body = prepare_body(closure, name, &self_id, ctx);

    // Bind `name` to a wrapper closure (capturing the `Rc`) whenever every
    // parameter is a simple identifier: `Rc<{closure}>` itself does not
    // implement `Fn`, so a plain binding could not be returned or passed as
    // a trait object. With non-identifier patterns the wrapper cannot be
    // built, so fall back to binding the `Rc` directly (usable in place,
    // not returnable).
    //
    // The wrapper inherits the user's `move`: a non-`move` `rec` borrows the
    // block-local `slot_ref`, so folding the scaffolding into a block or
    // force-moving the wrapper would dangle that borrow. Escaping the
    // enclosing fn therefore requires the user's `move` — same rule as for a
    // native closure. Stable Rust has no per-variable capture modes, so this
    // is not fixable in the macro.
    let args: Vec<Ident> = params.iter().filter_map(pat_ident).cloned().collect();
    let name_stmt = if args.len() == params.len() {
        // Wrapper without the user's `mut` (the body never mutates them).
        parse_quote! { let #name = #mov |#(#args),*| #rec(#(#args),*); }
    } else {
        parse_quote! { let #name = #rec; }
    };

    vec![
        parse_quote! {
            #[allow(clippy::type_complexity)]
            let #slot: #rc<#cell_ty> = #rc::new(#cell_new);
        },
        parse_quote! { let #slot_ref = #rc::clone(&#slot); },
        parse_quote! {
            let #rec = #rc::new(#mov |#(#params),*| {
                let #strong = #slot_ref.get()
                    .expect("rec_closure: recursion slot never initialized")
                    .upgrade()
                    .expect("rec_closure: recursive closure invoked after drop");
                let #self_id: #self_ty = &*#strong;
                #body
            });
        },
        parse_quote! {
            #[allow(clippy::type_complexity)]
            let #weak: #weak_mod<#dyn_ty> =
                #rc::downgrade(&(#rec.clone() as #rc<#dyn_ty>));
        },
        parse_quote! { #slot.set(#weak).ok().expect("rec_closure: recursion slot set twice"); },
        name_stmt,
    ]
}
