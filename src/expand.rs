//! Path selection and the `fn` / inferred (Rc) expansions.

use proc_macro2::Ident;
use syn::{ExprClosure, Pat, ReturnType, Stmt, Type, parse_quote};

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
        // Fully annotated and captures nothing: a plain local `fn` is the
        // best possible lowering (static call, no dynamic dispatch at all).
        Some(ta) if !captures_external(closure, name) => expand_fn(name, closure, &ta, ctx),
        // An elided reference return (`&str -> &str`) cannot be expressed in
        // the zero-allocation `Fn(&dyn HideFn, ...) -> ...` bound (E0106:
        // the elision rule picks `&dyn HideFn`); the inferred store erases
        // the return as `_`, which resolves correctly. Route there.
        Some(ta) if matches!(ta.ret, Type::Reference(_)) => expand_inferred(name, closure, n, ctx),
        Some(ta) => expand_zero_alloc(name, closure, &ta, n, ctx),
        None => expand_inferred(name, closure, n, ctx),
    };
    ctx.active.pop();
    Ok(result)
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
