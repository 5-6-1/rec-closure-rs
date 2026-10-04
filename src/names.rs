//! Fresh implementation names, including item names without local hygiene.

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::format_ident;
use std::collections::HashSet;

/// Reserve entire families mentioned in the input, including macro tokens
/// and explicit lifetimes. This also protects generated item/type names,
/// for which mixed-site local-variable hygiene alone is insufficient.
pub(crate) fn reserved_families(tokens: TokenStream) -> HashSet<usize> {
    let mut reserved = HashSet::new();
    for token in tokens {
        match token {
            TokenTree::Group(group) => reserved.extend(reserved_families(group.stream())),
            TokenTree::Ident(ident) => {
                let spelling = ident.to_string();
                let name = spelling.strip_prefix("r#").unwrap_or(&spelling);
                let suffix = name.strip_prefix("__rec_").or_else(|| name.strip_prefix("__Rec"));
                if let Some(suffix) = suffix {
                    let number: String = suffix.chars().take_while(char::is_ascii_digit).collect();
                    if let Ok(number) = number.parse() {
                        reserved.insert(number);
                    }
                }
            }
            _ => {}
        }
    }
    reserved
}

pub(crate) fn role_ident(n: usize, role: &str) -> Ident {
    format_ident!("__rec_{}_{}", n, role, span = Span::mixed_site())
}

pub(crate) fn type_ident(n: usize, role: &str) -> Ident {
    format_ident!("__Rec{}{}", n, role, span = Span::mixed_site())
}

pub(crate) fn arguments(n: usize, count: usize) -> Vec<Ident> {
    (0..count).map(|i| role_ident(n, &format!("arg{i}"))).collect()
}
