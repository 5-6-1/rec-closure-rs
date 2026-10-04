//! Reject ambiguous same-name nesting before any path rewrites the body.

use proc_macro2::Ident;
use syn::{
    ExprClosure, Item, Stmt,
    visit::{self, Visit},
};

use crate::as_recursive_let;

pub(crate) fn reject_nested_name(closure: &ExprClosure, name: &Ident) -> syn::Result<()> {
    let mut scan = NestedName { name, conflict: None };
    scan.visit_expr(&closure.body);
    match scan.conflict {
        Some(inner) => Err(syn::Error::new_spanned(
            inner,
            "a nested recursive closure cannot reuse the name of an enclosing \
             recursive closure; rename one of them",
        )),
        None => Ok(()),
    }
}

struct NestedName<'a> {
    name: &'a Ident,
    conflict: Option<&'a ExprClosure>,
}

impl<'a> Visit<'a> for NestedName<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if self.conflict.is_some() {
            return;
        }
        if let Some((name, closure)) = as_recursive_let(stmt)
            && name == self.name
        {
            self.conflict = Some(closure);
            return;
        }
        visit::visit_stmt(self, stmt);
    }

    // Items are independent scopes, so their closures may reuse the name.
    fn visit_item(&mut self, _: &'a Item) {}
}
