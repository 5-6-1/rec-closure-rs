//! Zero-allocation expansion: a `HideFn` trait whose `call` receives the
//! recursive self as a `&dyn HideFn` parameter (Y-combinator style).

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{ExprClosure, Stmt, parse_quote};

use crate::{Ctx, names::type_ident, prepare_body, role_ident};

use super::expand::TypedArgs;

fn sync_bounds(sync: bool) -> (TokenStream2, TokenStream2) {
    if sync {
        (
            quote! { : ::std::marker::Send + ::std::marker::Sync },
            quote! { + ::std::marker::Send + ::std::marker::Sync },
        )
    } else {
        (quote! {}, quote! {})
    }
}

pub(crate) fn expand_zero_alloc(
    name: &Ident, closure: &ExprClosure, ta: &TypedArgs, n: usize, ctx: &mut Ctx,
) -> Vec<Stmt> {
    let mov = closure.capture;
    let TypedArgs { pats, tys, names, ret } = ta;

    let self_param = role_ident(n, "self_param");
    let self_id = role_ident(n, "self");
    let inner = role_ident(n, "inner");
    let trait_name = type_ident(n, "HideFn");
    let impl_name = type_ident(n, "HideFnImpl");
    let callable = type_ident(n, "Callable");

    let body = prepare_body(closure, name, &self_id, ctx);
    let (trait_extra, f_extra) = sync_bounds(ctx.sync);

    let block = quote! {
        {
            trait #trait_name #trait_extra {
                fn call(&self, #(#names: #tys),*) -> #ret;
            }
            struct #impl_name<#callable: ::core::ops::Fn(&dyn #trait_name, #(#tys),*) -> #ret #f_extra>(#callable);
            impl<#callable: ::core::ops::Fn(&dyn #trait_name, #(#tys),*) -> #ret #f_extra>
                #trait_name for #impl_name<#callable> {
                #[inline]
                fn call(&self, #(#names: #tys),*) -> #ret {
                    self.0(self, #(#names),*)
                }
            }
            let #inner = #impl_name(#mov |#self_param, #(#pats: #tys),*| -> #ret {
                let #self_id = |#(#names: #tys),*| #self_param.call(#(#names),*);
                #body
            });
            move |#(#names: #tys),*| -> #ret {
                #inner.call(#(#names),*)
            }
        }
    };

    vec![parse_quote! { let #name = #block; }]
}
