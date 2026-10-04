//! Binding predicates shared by capture analysis, self references and expansion.

use proc_macro2::Ident;
use std::collections::HashSet;
use syn::{
    Expr, Item, Pat, Stmt,
    visit::{self, Visit},
};

/// Items bind throughout their block, regardless of declaration order.
pub(crate) fn block_items(stmts: &[Stmt]) -> HashSet<Ident> {
    stmts
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Item(Item::Fn(f)) => Some(f.sig.ident.clone()),
            Stmt::Item(Item::Const(c)) => Some(c.ident.clone()),
            Stmt::Item(Item::Static(s)) => Some(s.ident.clone()),
            Stmt::Item(Item::Struct(s)) if !matches!(s.fields, syn::Fields::Named(_)) => {
                Some(s.ident.clone())
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn pat_names(pat: &Pat) -> HashSet<Ident> {
    let mut names = PatternNames::default();
    names.visit_pat(pat);
    names.0
}

pub(crate) fn pat_binds_name(pat: &Pat, name: &Ident) -> bool {
    pat_names(pat).contains(name)
}

#[derive(Default)]
struct PatternNames(HashSet<Ident>);

impl<'a> Visit<'a> for PatternNames {
    fn visit_pat_ident(&mut self, pat: &'a syn::PatIdent) {
        self.0.insert(pat.ident.clone());
        visit::visit_pat_ident(self, pat);
    }

    // Bindings inside a guard expression belong to that expression, not
    // to the pattern that contains it.
    fn visit_expr(&mut self, _: &'a Expr) {}
}

/// Condition bindings are visible to later `&&` operands and the body.
pub(crate) fn condition_names(expr: &Expr) -> HashSet<Ident> {
    match expr {
        Expr::Let(l) => pat_names(&l.pat),
        Expr::Binary(b) if matches!(b.op, syn::BinOp::And(_)) => {
            let mut names = condition_names(&b.left);
            names.extend(condition_names(&b.right));
            names
        }
        _ => HashSet::new(),
    }
}
