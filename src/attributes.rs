//! Preserve conditional compilation across inline expansion statements.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Attribute, Meta, Stmt, Token, parse::Parser, parse_quote, punctuated::Punctuated};

/// Read only leading attributes, never attributes of child statements.
/// Items and expressions have different AST variants but share this prefix.
pub(crate) fn statement_configuration(stmt: &Stmt) -> Vec<Attribute> {
    let parser = |input: syn::parse::ParseStream<'_>| {
        let attrs = input.call(Attribute::parse_outer)?;
        let _: TokenStream = input.parse()?;
        Ok(attrs)
    };
    let attrs = parser.parse2(stmt.to_token_stream()).unwrap_or_default();
    configuration(&attrs)
}

/// Retain only configuration effects. Duplicating an `expect` lint across
/// implementation statements would create unfulfilled expectations.
pub(crate) fn configuration(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter_map(|attr| configuration_meta(&attr.meta).map(|meta| parse_quote!(#[#meta])))
        .collect()
}

fn configuration_meta(meta: &Meta) -> Option<Meta> {
    if meta.path().is_ident("cfg") {
        return Some(meta.clone());
    }
    let Meta::List(list) = meta else { return None };
    if !list.path.is_ident("cfg_attr") {
        return None;
    }
    let mut args =
        list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).ok()?.into_iter();
    let condition = args.next()?;
    let nested: Vec<_> = args.filter_map(|meta| configuration_meta(&meta)).collect();
    (!nested.is_empty()).then(|| parse_quote!(cfg_attr(#condition, #(#nested),*)))
}

pub(crate) fn inherit(stmt: &Stmt, mut expanded: Vec<Stmt>) -> syn::Result<Vec<Stmt>> {
    let Stmt::Local(original) = stmt else { return Ok(expanded) };
    let cfg = configuration(&original.attrs);
    if !cfg.is_empty() {
        for stmt in &mut expanded {
            *stmt = syn::parse2(quote!(#(#cfg)* #stmt))?;
        }
    }
    if let Some(Stmt::Local(binding)) = expanded.last_mut() {
        binding.pat = original.pat.clone();
        binding.attrs = original.attrs.clone();
        if let (Some(original), Some(generated)) = (&original.init, &mut binding.init) {
            generated.diverge = original.diverge.clone();
        }
    }
    Ok(expanded)
}
