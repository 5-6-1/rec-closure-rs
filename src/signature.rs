//! Decide whether generated local items can express a closure's types.

use proc_macro2::Ident;
use std::collections::HashSet;
use syn::{ExprClosure, GenericParam, Generics, visit::Visit};

pub(crate) fn generic_names(generics: &Generics) -> HashSet<Ident> {
    generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Type(t) => t.ident.clone(),
            GenericParam::Const(c) => c.ident.clone(),
            GenericParam::Lifetime(l) => l.lifetime.ident.clone(),
        })
        .collect()
}

/// A local fn/trait cannot use `_` in its signature or inherit enclosing
/// generic parameters. Keeping these cases in closures preserves inference
/// and the enclosing generic scope. Checking the body too protects `T::new()`.
pub(crate) fn needs_inference(closure: &ExprClosure, generics: &HashSet<Ident>) -> bool {
    let mut scanner = ItemIncompatible { generics, found: false, signature: true };
    for input in &closure.inputs {
        scanner.visit_pat(input);
    }
    scanner.visit_return_type(&closure.output);
    scanner.signature = false;
    scanner.visit_expr(&closure.body);
    scanner.found
}

struct ItemIncompatible<'a> {
    generics: &'a HashSet<Ident>,
    found: bool,
    signature: bool,
}

impl<'a> Visit<'a> for ItemIncompatible<'a> {
    fn visit_ident(&mut self, id: &'a Ident) {
        // `Self` may belong to an enclosing impl that the attribute cannot
        // inspect. It is never safe to move it into a new local item.
        self.found |= id == "Self" || self.generics.contains(id);
    }

    fn visit_type_infer(&mut self, _: &'a syn::TypeInfer) {
        self.found |= self.signature;
    }
}
