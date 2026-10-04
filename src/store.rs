//! Storage tokens for the inferred (Rc) expansion, parameterized by the sync
//! mode, and the higher-ranked lifetime lifting of reference parameters.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Lifetime, Pat, Token,
    punctuated::Punctuated,
    visit_mut::{self, VisitMut},
};

/// Storage tokens for one expansion, parameterized by the sync mode.
pub(crate) struct DynStore {
    pub(crate) rc: TokenStream2,
    pub(crate) weak_mod: TokenStream2,
    pub(crate) cell_ty: TokenStream2,
    pub(crate) cell_new: TokenStream2,
    pub(crate) self_ty: TokenStream2,
    pub(crate) dyn_ty: TokenStream2,
}

impl DynStore {
    /// `param_tys` are the erased parameter types (`_` holes, or annotated
    /// types with reference parameters rewritten to a fresh lifetime);
    /// `for_clause` binds those lifetimes (`for<'a, ...>` or empty).
    pub(crate) fn new(sync: bool, param_tys: &[TokenStream2], for_clause: &TokenStream2) -> Self {
        let extra = if sync {
            quote! { + ::std::marker::Send + ::std::marker::Sync }
        } else {
            quote! {}
        };
        let sig = quote! { #for_clause ::core::ops::Fn(#(#param_tys),*) -> _ #extra };
        let (rc, weak_mod, cell_ty, cell_new) = if sync {
            (
                quote! { ::std::sync::Arc },
                quote! { ::std::sync::Weak },
                quote! { ::std::sync::OnceLock<::std::sync::Weak<dyn #sig> > },
                quote! { ::std::sync::OnceLock::new() },
            )
        } else {
            (
                quote! { ::std::rc::Rc },
                quote! { ::std::rc::Weak },
                quote! { ::std::cell::OnceCell<::std::rc::Weak<dyn #sig> > },
                quote! { ::std::cell::OnceCell::new() },
            )
        };
        Self {
            rc,
            weak_mod,
            cell_ty,
            cell_new,
            self_ty: quote! { &(dyn #sig) },
            dyn_ty: quote! { dyn #sig },
        }
    }
}

/// The erased parameter types for the inferred store. Elided reference
/// lifetimes become `&'L ...` bound by fresh higher-ranked lifetimes, recursively
/// (so `Vec<&mut T>` and `Option<&T>` work too); everything else is either
/// the annotated type or a `_` hole. Explicit lifetimes remain unchanged.
/// Returns the parameter types and the `for<...>` clause (empty when no
/// references were lifted).
pub(crate) fn erased_params(
    inputs: &Punctuated<Pat, Token![,]>, family: usize,
) -> (Vec<TokenStream2>, TokenStream2) {
    let mut lift = LiftRefs { lifetimes: Vec::new(), counter: 0, family };
    let param_tys = inputs
        .iter()
        .map(|pat| match pat {
            Pat::Type(pt) => {
                let mut ty = (*pt.ty).clone();
                lift.visit_type_mut(&mut ty);
                quote! { #ty }
            }
            _ => quote! { _ },
        })
        .collect();
    let for_clause = if lift.lifetimes.is_empty() {
        quote! {}
    } else {
        let lifetimes = &lift.lifetimes;
        quote! { for<#(#lifetimes),*> }
    };
    (param_tys, for_clause)
}

/// Replaces elided references nested in a type with fresh anonymous
/// higher-ranked lifetime; the collected binders form the `for<...>` clause.
/// Traversing with `VisitMut` covers every type shape uniformly (paths,
/// tuples, slices, arrays, pointers, trait objects, ...), including shapes a
/// hand-rolled match would silently pass through untouched.
struct LiftRefs {
    lifetimes: Vec<Lifetime>,
    counter: usize,
    family: usize,
}

impl VisitMut for LiftRefs {
    fn visit_type_reference_mut(&mut self, ty: &mut syn::TypeReference) {
        visit_mut::visit_type_reference_mut(self, ty);
        // Named lifetimes express a user constraint (including `'static`),
        // not a borrow that may vary independently on every recursive call.
        if ty.lifetime.as_ref().is_some_and(|lt| lt.ident != "_") {
            return;
        }
        // `__rec_` prefix keeps generated lifetimes out of the user's
        // namespace, consistent with the `__rec_{N}_{role}` identifiers.
        let lt = Lifetime::new(
            &format!("'__rec_{}_lt{}", self.family, self.counter),
            Span::mixed_site(),
        );
        self.counter += 1;
        self.lifetimes.push(lt.clone());
        ty.lifetime = Some(lt);
    }

    // References inside `fn` pointers and trait bounds are already
    // higher-ranked in the source (`fn(&str)`, `dyn Fn(&str)`); lifting them
    // would nest HRTB and break the signature, so leave them untouched.
    fn visit_type_fn_ptr_mut(&mut self, _: &mut syn::TypeFnPtr) {}

    fn visit_trait_bound_mut(&mut self, _: &mut syn::TraitBound) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;
    use syn::Type;
    use syn::parse_str;

    /// Apply the HRTB lifting to a type and return its output plus the
    /// collected fresh lifetime names.
    fn lifted(input: &str) -> (String, Vec<String>) {
        let ty: Type = parse_str(input).unwrap();
        let mut lift = LiftRefs { lifetimes: Vec::new(), counter: 0, family: 0 };
        let mut t = ty;
        lift.visit_type_mut(&mut t);
        let names = lift.lifetimes.iter().map(|l| l.ident.to_string()).collect();
        (t.to_token_stream().to_string(), names)
    }

    #[test]
    fn lifts_single_ref() {
        let (out, lts) = lifted("&str");
        assert!(out.contains("& '__rec_0_lt0 str"), "out: {out}");
        assert_eq!(lts, ["__rec_0_lt0"]);
    }

    #[test]
    fn lifts_ref_inside_container() {
        let (out, lts) = lifted("Vec<&mut i32>");
        assert!(out.contains("& '__rec_0_lt0 mut i32"), "out: {out}");
        assert_eq!(lts, ["__rec_0_lt0"]);
    }

    #[test]
    fn lifts_double_reference_independently() {
        // Recursion visits the inner reference first, so the outer one gets
        // the later lifetime.
        let (out, lts) = lifted("&(&str)");
        assert!(out.contains("& '__rec_0_lt1 (& '__rec_0_lt0 str)"), "out: {out}");
        assert_eq!(lts, ["__rec_0_lt0", "__rec_0_lt1"]);
    }

    #[test]
    fn lifts_multiple_refs_in_generic_args() {
        let (_, lts) = lifted("Result<&str, &mut [u8]>");
        assert_eq!(lts, ["__rec_0_lt0", "__rec_0_lt1"]);
    }

    #[test]
    fn skips_fn_pointer_refs() {
        let (out, lts) = lifted("fn(&str) -> i32");
        assert!(lts.is_empty(), "fn-pointer refs must not lift: {lts:?}");
        assert!(out.contains("& str"), "out: {out}");
    }

    #[test]
    fn skips_trait_bound_refs() {
        let (out, lts) = lifted("Box<dyn Fn(&str) -> i32>");
        assert!(lts.is_empty(), "trait-bound refs must not lift: {lts:?}");
        assert!(out.contains("& str"), "out: {out}");
    }
}
