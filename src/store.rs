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
        let sig = quote! { #for_clause Fn(#(#param_tys),*) -> _ #extra };
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

/// The erased parameter types for the inferred store. Annotated reference
/// types become `&'L ...` bound by fresh higher-ranked lifetimes, recursively
/// (so `Vec<&mut T>` and `Option<&T>` work too); everything else is either
/// the annotated type or a `_` hole. Returns the parameter types and the
/// `for<...>` clause (empty when no references were lifted).
pub(crate) fn erased_params(
    inputs: &Punctuated<Pat, Token![,]>,
) -> (Vec<TokenStream2>, TokenStream2) {
    let mut lift = LiftRefs { lifetimes: Vec::new(), counter: 0 };
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

/// Replaces every reference nested in a type with a fresh anonymous
/// higher-ranked lifetime; the collected binders form the `for<...>` clause.
/// Traversing with `VisitMut` covers every type shape uniformly (paths,
/// tuples, slices, arrays, pointers, trait objects, ...), including shapes a
/// hand-rolled match would silently pass through untouched.
struct LiftRefs {
    lifetimes: Vec<Lifetime>,
    counter: usize,
}

impl VisitMut for LiftRefs {
    fn visit_type_reference_mut(&mut self, ty: &mut syn::TypeReference) {
        visit_mut::visit_type_reference_mut(self, ty);
        let lt = Lifetime::new(&format!("'a{}", self.counter), Span::call_site());
        self.counter += 1;
        self.lifetimes.push(lt.clone());
        ty.lifetime = Some(lt);
    }
}
